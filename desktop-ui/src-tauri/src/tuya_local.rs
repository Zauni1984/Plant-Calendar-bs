//! Tuya Local Protocol — communicate with Spider Farmer GGS controllers
//! via the local network (TCP port 6668, AES-128-ECB encrypted).
//!
//! Requires: device_id, local_key, and IP address of the device.

use serde::{Deserialize, Serialize};
use shared::GgsTelemetry;
use tuya_rs::connection::{DeviceConfig, TuyaCommand, TuyaConnection, Transport};

/// Configuration for a Tuya device on the local network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuyaDeviceConfig {
    pub device_id: String,
    pub local_key: String,
    pub ip_address: String,
    pub port: u16,
}

impl Default for TuyaDeviceConfig {
    fn default() -> Self {
        Self {
            device_id: String::new(),
            local_key: String::new(),
            ip_address: String::new(),
            port: 6668,
        }
    }
}

/// Query a Tuya device for its current status (DPS values)
pub async fn query_device_status(config: &TuyaDeviceConfig) -> anyhow::Result<serde_json::Value> {
    let cfg = DeviceConfig {
        dev_id: config.device_id.clone(),
        address: config.ip_address.clone(),
        local_key: config.local_key.clone(),
        port: config.port,
        ..Default::default()
    };

    // tuya-rs uses blocking TCP — run in spawn_blocking
    let result = tokio::task::spawn_blocking(move || -> anyhow::Result<serde_json::Value> {
        let mut conn = TuyaConnection::connect(&cfg)
            .map_err(|e| anyhow::anyhow!("Tuya connection failed: {e}"))?;

        let response = conn.send(TuyaCommand::DpQuery, b"{}".to_vec())
            .map_err(|e| anyhow::anyhow!("Tuya query failed: {e}"))?;

        let payload_str = String::from_utf8_lossy(&response.payload);
        eprintln!("[Tuya] Device responded, {} bytes", response.payload.len());

        if response.payload.is_empty() {
            anyhow::bail!("Tuya device returned empty payload");
        }

        let value: serde_json::Value = serde_json::from_str(&payload_str)
            .map_err(|e| anyhow::anyhow!("JSON parse error: {e} — raw: {}", payload_str))?;

        Ok(value)
    }).await
    .map_err(|e| anyhow::anyhow!("task join error: {e}"))??;

    Ok(result)
}

/// Parse Tuya DPS (Data Point Status) into GGS telemetry.
/// GGS controllers typically use these DPS IDs:
/// - DPS 1: temperature (°C * 10, integer)
/// - DPS 2: humidity (% * 10, integer)
/// - DPS 18 or similar: VPD
/// - DPS 101+: fan/light levels
///
/// The exact mapping depends on the device firmware. This function
/// tries common patterns and falls back gracefully.
pub fn parse_dps_to_telemetry(dps: &serde_json::Value) -> GgsTelemetry {
    let dps_obj = dps.get("dps").unwrap_or(dps);

    // Try to extract values — Tuya often sends scaled integers
    let temp = extract_scaled_float(dps_obj, &["1", "18", "101"], 10.0);
    let humidity = extract_scaled_float(dps_obj, &["2", "19", "102"], 10.0);
    let vpd = extract_scaled_float(dps_obj, &["12", "20", "103"], 100.0);

    // Fan and light — try common DPS IDs
    let fan_on = extract_bool(dps_obj, &["4", "104"]);
    let fan_level = extract_int(dps_obj, &["5", "3", "105"]).unwrap_or(0) as u8;
    let light_level = extract_int(dps_obj, &["6", "106"]).unwrap_or(0) as u8;

    GgsTelemetry {
        temp,
        humidity,
        vpd,
        fan_on: fan_on.unwrap_or(fan_level > 0),
        fan_level,
        light_level,
    }
}

fn extract_scaled_float(dps: &serde_json::Value, keys: &[&str], scale: f32) -> Option<f32> {
    for key in keys {
        if let Some(val) = dps.get(key) {
            if let Some(n) = val.as_f64() {
                return Some(n as f32 / scale);
            }
            if let Some(n) = val.as_i64() {
                return Some(n as f32 / scale);
            }
        }
    }
    None
}

fn extract_bool(dps: &serde_json::Value, keys: &[&str]) -> Option<bool> {
    for key in keys {
        if let Some(val) = dps.get(key) {
            if let Some(b) = val.as_bool() { return Some(b); }
            if let Some(n) = val.as_i64() { return Some(n != 0); }
        }
    }
    None
}

fn extract_int(dps: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    for key in keys {
        if let Some(val) = dps.get(key) {
            if let Some(n) = val.as_i64() { return Some(n); }
        }
    }
    None
}
