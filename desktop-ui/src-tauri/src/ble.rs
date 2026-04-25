use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, CharPropFlags, WriteType};
use btleplug::platform::{Manager, Peripheral};
use uuid::Uuid as BleUuid;
use shared::GgsTelemetry;

/// GGS BLE Service UUID
const GGS_SERVICE_UUID: &str = "0000ff00-0000-1000-8000-00805f9b34fb";
/// GGS Status Notification Characteristic (FF01) — device → client
const GGS_STATUS_UUID: &str = "0000ff01-0000-1000-8000-00805f9b34fb";
/// GGS Command Write Characteristic (FF02) — client → device
const GGS_COMMAND_UUID: &str = "0000ff02-0000-1000-8000-00805f9b34fb";

#[derive(Debug, Clone, serde::Serialize)]
pub struct BleDeviceInfo {
    pub name: String,
    pub address: String,
    pub rssi: Option<i16>,
}

pub struct BleManager {
    manager: Manager,
}

impl BleManager {
    pub async fn new() -> anyhow::Result<Self> {
        let manager = Manager::new().await?;
        Ok(Self { manager })
    }

    /// Scan for GGS devices (scan for N seconds, return found devices)
    pub async fn scan_ggs_devices(&self, duration_secs: u64) -> anyhow::Result<Vec<BleDeviceInfo>> {
        let adapters = self.manager.adapters().await?;
        let adapter = adapters.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("no Bluetooth adapter found"))?;

        adapter.start_scan(ScanFilter::default()).await?;
        tokio::time::sleep(std::time::Duration::from_secs(duration_secs)).await;
        adapter.stop_scan().await?;

        let peripherals = adapter.peripherals().await?;
        let mut devices = Vec::new();

        for p in peripherals {
            let props = p.properties().await?.unwrap_or_default();
            let name = props.local_name.unwrap_or_default();
            let address = props.address.to_string();

            let has_ggs = props.services.iter().any(|s|
                s.to_string().to_lowercase() == GGS_SERVICE_UUID
            );
            let name_match = name.to_lowercase().contains("ggs")
                || name.to_lowercase().contains("spider")
                || name.to_lowercase().contains("sf_");

            if has_ggs || name_match {
                devices.push(BleDeviceInfo {
                    name: if name.is_empty() { format!("GGS Device ({})", &address[..std::cmp::min(8, address.len())]) } else { name },
                    address,
                    rssi: props.rssi,
                });
            }
        }

        Ok(devices)
    }

    /// Connect to a GGS device by address and read one telemetry snapshot
    pub async fn read_telemetry(&self, ble_address: &str) -> anyhow::Result<GgsTelemetry> {
        let adapters = self.manager.adapters().await?;
        let adapter = adapters.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("no Bluetooth adapter found"))?;

        // Scan briefly to discover the device
        adapter.start_scan(ScanFilter::default()).await?;
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        adapter.stop_scan().await?;

        // Find peripheral by address
        let peripherals = adapter.peripherals().await?;
        let mut target_peripheral = None;
        for p in peripherals {
            if let Ok(Some(props)) = p.properties().await {
                if props.address.to_string() == ble_address {
                    target_peripheral = Some(p);
                    break;
                }
            }
        }
        let target = target_peripheral
            .ok_or_else(|| anyhow::anyhow!("device not found: {}", ble_address))?;

        // Connect
        if !target.is_connected().await? {
            target.connect().await?;
        }
        target.discover_services().await?;

        // Find FF01 (read) and FF02 (write) characteristics
        let ff01_uuid = BleUuid::parse_str(GGS_STATUS_UUID)?;
        let ff02_uuid = BleUuid::parse_str(GGS_COMMAND_UUID)?;
        let characteristics = target.characteristics();

        #[cfg(debug_assertions)]
        {
            eprintln!("[BLE] Discovered {} characteristics", characteristics.len());
            for c in &characteristics {
                eprintln!("[BLE]   UUID={} props={:?}", c.uuid, c.properties);
            }
        }

        let status_char = characteristics.iter()
            .find(|c| c.uuid == ff01_uuid)
            .ok_or_else(|| anyhow::anyhow!("GGS status characteristic (FF01) not found. Available: {:?}",
                characteristics.iter().map(|c| c.uuid.to_string()).collect::<Vec<_>>()))?
            .clone();

        let command_char = characteristics.iter()
            .find(|c| c.uuid == ff02_uuid)
            .cloned();

        // Send getDevSta command to FF02 to request telemetry
        if let Some(ref cmd_char) = command_char {
            let cmd = br#"{"method":"getDevSta"}"#;
            let write_type = if cmd_char.properties.contains(CharPropFlags::WRITE) {
                WriteType::WithResponse
            } else {
                WriteType::WithoutResponse
            };
            #[cfg(debug_assertions)]
            eprintln!("[BLE] Sending getDevSta command to FF02...");
            target.write(cmd_char, cmd, write_type).await
                .map_err(|e| anyhow::anyhow!("failed to write command to FF02: {e}"))?;
            // Give device time to process command
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        } else {
            #[cfg(debug_assertions)]
            eprintln!("[BLE] FF02 command characteristic not found, trying to read FF01 directly...");
        }

        // Try to read or subscribe for notifications.
        // GGS sends fragmented JSON across multiple BLE packets — we must
        // buffer until we see a complete JSON object ({...}).
        let raw_data = if status_char.properties.contains(CharPropFlags::NOTIFY)
            || status_char.properties.contains(CharPropFlags::READ)
        {
            if status_char.properties.contains(CharPropFlags::NOTIFY) {
                use tokio_stream::StreamExt;
                target.subscribe(&status_char).await?;
                let mut stream = target.notifications().await?;
                let buffer = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
                let buf_clone = buffer.clone();

                let result = tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    async move {
                        while let Some(notification) = stream.next().await {
                            if notification.uuid == ff01_uuid {
                                let mut buf = buf_clone.lock().unwrap();
                                buf.extend_from_slice(&notification.value);
                                // Check if buffer contains a complete JSON object
                                if let (Some(start), Some(end)) = (
                                    buf.iter().position(|&b| b == b'{'),
                                    buf.iter().rposition(|&b| b == b'}'),
                                ) {
                                    if end > start {
                                        let slice = &buf[start..=end];
                                        if std::str::from_utf8(slice).is_ok()
                                            && serde_json::from_slice::<serde_json::Value>(slice).is_ok()
                                        {
                                            return true;
                                        }
                                    }
                                }
                            }
                        }
                        false
                    }
                ).await;

                target.unsubscribe(&status_char).await.ok();

                let final_buf = buffer.lock().unwrap().clone();
                match result {
                    Ok(true) => final_buf,
                    Ok(false) => anyhow::bail!("BLE stream ended before complete JSON received"),
                    Err(_) => {
                        if final_buf.is_empty() {
                            anyhow::bail!("timeout: no BLE data received in 15s");
                        } else {
                            final_buf
                        }
                    }
                }
            } else {
                // READ mode — may also be fragmented, try reading multiple times
                let mut buffer = target.read(&status_char).await?;
                // If first read doesn't contain complete JSON, try a few more
                for _ in 0..5 {
                    if let (Some(s), Some(e)) = (
                        buffer.iter().position(|&b| b == b'{'),
                        buffer.iter().rposition(|&b| b == b'}'),
                    ) {
                        if e > s {
                            let slice = &buffer[s..=e];
                            if std::str::from_utf8(slice).is_ok()
                                && serde_json::from_slice::<serde_json::Value>(slice).is_ok()
                            {
                                break;
                            }
                        }
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    if let Ok(more) = target.read(&status_char).await {
                        buffer.extend_from_slice(&more);
                    }
                }
                buffer
            }
        } else {
            anyhow::bail!("FF01 characteristic is neither readable nor notifiable");
        };

        // Disconnect
        target.disconnect().await.ok();

        eprintln!("[BLE] Received {} bytes from device", raw_data.len());

        // Parse
        shared::parse_ggs_notification(&raw_data)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Read raw BLE data without parsing — for diagnostics
    pub async fn read_raw_data(&self, ble_address: &str) -> anyhow::Result<Vec<u8>> {
        let adapters = self.manager.adapters().await?;
        let adapter = adapters.into_iter().next()
            .ok_or_else(|| anyhow::anyhow!("no Bluetooth adapter found"))?;

        // Try already-known peripherals first (no scan needed if cached)
        let mut target_peripheral: Option<_> = None;
        for p in adapter.peripherals().await.unwrap_or_default() {
            if let Ok(Some(props)) = p.properties().await {
                if props.address.to_string() == ble_address {
                    target_peripheral = Some(p);
                    break;
                }
            }
        }

        // If not in cache, scan with retry (up to 3 attempts, 5s each)
        if target_peripheral.is_none() {
            for attempt in 1..=3 {
                let _ = adapter.start_scan(ScanFilter::default()).await;
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                let _ = adapter.stop_scan().await;

                for p in adapter.peripherals().await.unwrap_or_default() {
                    if let Ok(Some(props)) = p.properties().await {
                        if props.address.to_string() == ble_address {
                            target_peripheral = Some(p);
                            break;
                        }
                    }
                }
                if target_peripheral.is_some() {
                    break;
                }
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                }
            }
        }

        let target = target_peripheral
            .ok_or_else(|| anyhow::anyhow!("device not found after 3 scan attempts: {}", ble_address))?;

        if !target.is_connected().await? {
            target.connect().await?;
        }
        target.discover_services().await?;

        let ff01_uuid = BleUuid::parse_str(GGS_STATUS_UUID)?;
        let ff02_uuid = BleUuid::parse_str(GGS_COMMAND_UUID)?;
        let characteristics = target.characteristics();

        // Log characteristics
        let char_list: Vec<String> = characteristics.iter()
            .map(|c| format!("{} {:?}", c.uuid, c.properties))
            .collect();

        let status_char = characteristics.iter()
            .find(|c| c.uuid == ff01_uuid)
            .cloned();
        let command_char = characteristics.iter()
            .find(|c| c.uuid == ff02_uuid)
            .cloned();

        // Try sending command first
        if let Some(ref cmd) = command_char {
            let write_type = if cmd.properties.contains(CharPropFlags::WRITE) {
                WriteType::WithResponse
            } else {
                WriteType::WithoutResponse
            };
            target.write(cmd, br#"{"method":"getDevSta"}"#, write_type).await.ok();
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
        }

        let mut all_data = Vec::new();

        if let Some(ref sc) = status_char {
            // Try subscribe + collect for a few seconds
            if sc.properties.contains(CharPropFlags::NOTIFY) {
                use tokio_stream::StreamExt;
                target.subscribe(sc).await?;
                let mut stream = target.notifications().await?;
                let buf = std::sync::Arc::new(std::sync::Mutex::new(Vec::<u8>::new()));
                let buf_c = buf.clone();
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(8),
                    async move {
                        while let Some(n) = stream.next().await {
                            buf_c.lock().unwrap().extend_from_slice(&n.value);
                        }
                    }
                ).await;
                target.unsubscribe(sc).await.ok();
                all_data = buf.lock().unwrap().clone();
            } else if sc.properties.contains(CharPropFlags::READ) {
                all_data = target.read(sc).await?;
            }
        }

        target.disconnect().await.ok();

        // Return raw data + metadata
        let mut result = Vec::new();
        // Prepend metadata as UTF-8 text, then separator, then raw
        let meta = format!("CHARS:{}\nDATA_START:", char_list.join("; "));
        result.extend_from_slice(meta.as_bytes());
        result.extend_from_slice(&all_data);
        Ok(all_data) // Return just raw data
    }
}
