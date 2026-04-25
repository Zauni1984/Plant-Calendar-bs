//! Spider Farmer LC Light Controller BLE parser.
//!
//! The LC controller sends plain-JSON frames over BLE notifications (no AES).
//! A single BLE read typically returns a concatenation of several frames:
//! - Frame 1 is often raw JSON with no header.
//! - Frames 2+ are preceded by a Tuya-style header starting with `aa aa`
//!   followed by metadata and a 2-byte length field; the payload itself is
//!   still plain JSON.
//!
//! Observed methods: `getDevSta`, `getSysSta`.

use serde::{Deserialize, Serialize};

/// One parsed JSON frame from the controller stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LcFrame {
    pub method: String,
    #[serde(default)]
    pub pid: Option<String>,
    #[serde(default)]
    pub pcode: Option<u32>,
    #[serde(default)]
    pub uid: Option<String>,
    #[serde(rename = "UTC", default)]
    pub utc: Option<i64>,
    #[serde(default)]
    pub code: Option<u32>,
    #[serde(default)]
    pub msg: Option<String>,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// High-level device status derived from `getDevSta` + `getSysSta` frames.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LcStatus {
    // From getDevSta.data
    pub mode: Option<u32>,
    pub brightness: Option<u32>,
    pub pwm: Option<u32>,
    pub is_screen_on: Option<bool>,
    pub adc_led_dim: Option<u32>,
    pub adc_up_dim: Option<u32>,
    pub adc_down_dim: Option<u32>,
    pub is_up: Option<bool>,
    pub is_force_slave: Option<bool>,
    pub auto_mode_check: Option<bool>,
    pub last_on_brightness: Option<u32>,
    pub last_manual_brightness: Option<u32>,
    pub light_model: Option<u32>,

    // From getSysSta.data.sys
    pub fw_version: Option<String>,
    pub hw_version: Option<String>,
    pub build_time: Option<String>,
    pub localtime: Option<String>,
    pub timezone: Option<String>,
    pub up_count: Option<u32>,
    pub up_time: Option<u64>,
    pub mem_free: Option<u32>,

    pub wifi_connected: Option<bool>,
    pub wifi_ssid: Option<String>,
    pub mqtt_connected: Option<bool>,
    pub bt_connected: Option<bool>,
    pub bt_connect_time: Option<u32>,

    pub pid: Option<String>,
    pub pcode: Option<u32>,
    pub uid: Option<String>,
    pub utc: Option<i64>,
}

/// Parse a raw BLE byte stream and extract all JSON frames.
///
/// Strategy: scan for the ASCII pattern `{"method":` and brace-count
/// forward to extract each JSON object. Binary framing headers between
/// frames are simply skipped.
pub fn parse_stream(raw: &[u8]) -> Vec<LcFrame> {
    let mut frames = Vec::new();
    let needle = b"{\"method\":";
    let mut i = 0;
    while i + needle.len() <= raw.len() {
        if &raw[i..i + needle.len()] == needle {
            if let Some((frame_bytes, consumed)) = extract_json_object(&raw[i..]) {
                if let Ok(parsed) = serde_json::from_slice::<LcFrame>(frame_bytes) {
                    frames.push(parsed);
                }
                i += consumed;
                continue;
            }
        }
        i += 1;
    }
    frames
}

/// Given a slice that starts with `{`, walk forward counting braces
/// (while respecting string literals) and return (&json_bytes, len_consumed).
fn extract_json_object(s: &[u8]) -> Option<(&[u8], usize)> {
    if s.first() != Some(&b'{') {
        return None;
    }
    let mut depth: i32 = 0;
    let mut in_str = false;
    let mut escape = false;
    for (i, &b) in s.iter().enumerate() {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&s[..=i], i + 1));
                }
            }
            _ => {}
        }
    }
    None
}

/// Fold a list of parsed frames into a single consolidated status snapshot.
pub fn build_status(frames: &[LcFrame]) -> LcStatus {
    let mut status = LcStatus::default();
    for frame in frames {
        // Common fields (take first non-empty)
        if status.pid.is_none() {
            status.pid = frame.pid.clone();
        }
        if status.pcode.is_none() {
            status.pcode = frame.pcode;
        }
        if status.uid.is_none() {
            status.uid = frame.uid.clone();
        }
        if status.utc.is_none() {
            status.utc = frame.utc;
        }

        let Some(data) = frame.data.as_ref() else { continue };

        match frame.method.as_str() {
            "getDevSta" => apply_dev_sta(&mut status, data),
            "getSysSta" => apply_sys_sta(&mut status, data),
            _ => {}
        }
    }
    status
}

fn apply_dev_sta(status: &mut LcStatus, data: &serde_json::Value) {
    status.mode = data.get("mode").and_then(|v| v.as_u64()).map(|v| v as u32);
    status.brightness = data.get("brightness").and_then(|v| v.as_u64()).map(|v| v as u32);
    status.pwm = data.get("pwm").and_then(|v| v.as_u64()).map(|v| v as u32);
    status.is_screen_on = data.get("isScreenOn").and_then(|v| v.as_u64()).map(|v| v != 0);
    status.adc_led_dim = data.get("adcLedDim").and_then(|v| v.as_u64()).map(|v| v as u32);
    status.adc_up_dim = data.get("adcUpDim").and_then(|v| v.as_u64()).map(|v| v as u32);
    status.adc_down_dim = data.get("adcDownDim").and_then(|v| v.as_u64()).map(|v| v as u32);
    status.is_up = data.get("isUp").and_then(|v| v.as_u64()).map(|v| v != 0);
    status.is_force_slave = data.get("isForceSlaveMode").and_then(|v| v.as_u64()).map(|v| v != 0);
    status.auto_mode_check = data.get("AutoModeCheck").and_then(|v| v.as_u64()).map(|v| v != 0);
    status.last_on_brightness = data.get("lastOnBrightness").and_then(|v| v.as_u64()).map(|v| v as u32);
    status.last_manual_brightness = data.get("lastManualBrightness").and_then(|v| v.as_u64()).map(|v| v as u32);
    status.light_model = data.get("lightModel").and_then(|v| v.as_u64()).map(|v| v as u32);
}

fn apply_sys_sta(status: &mut LcStatus, data: &serde_json::Value) {
    if let Some(sys) = data.get("sys") {
        status.fw_version = sys.get("ver").and_then(|v| v.as_str()).map(String::from);
        status.hw_version = sys.get("hwver").and_then(|v| v.as_str()).map(String::from);
        status.build_time = sys.get("buildTime").and_then(|v| v.as_str()).map(String::from);
        status.localtime = sys.get("localtime").and_then(|v| v.as_str()).map(String::from);
        status.timezone = sys.get("timezone").and_then(|v| v.as_str()).map(String::from);
        status.up_count = sys.get("upCount").and_then(|v| v.as_u64()).map(|v| v as u32);
        status.up_time = sys.get("upTime").and_then(|v| v.as_u64());
        status.mem_free = sys.get("mem").and_then(|v| v.as_u64()).map(|v| v as u32);

        if let Some(wifi) = sys.get("wifi") {
            status.wifi_connected = wifi.get("isConnect").and_then(|v| v.as_u64()).map(|v| v != 0);
            status.wifi_ssid = wifi.get("ssid").and_then(|v| v.as_str()).map(String::from);
        }
        if let Some(mqtt) = sys.get("mqtt") {
            status.mqtt_connected = mqtt.get("isConnect").and_then(|v| v.as_u64()).map(|v| v != 0);
        }
        if let Some(bt) = sys.get("bluetooth") {
            status.bt_connected = bt.get("isConnect").and_then(|v| v.as_u64()).map(|v| v != 0);
            status.bt_connect_time = bt.get("connectTime").and_then(|v| v.as_u64()).map(|v| v as u32);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real-world dump from SF-LC-CB controller (917 bytes, hex truncated for brevity).
    fn sample_bytes() -> Vec<u8> {
        let part1 = br#"{"method":"getDevSta","pid":"90E5B1B7D37C","pcode":1005,"uid":"90331","UTC":1776612063,"code":200,"msg":"ok","data":{"mode":1,"brightness":60,"isScreenOn":1,"isIoTx":1,"isIoRx":0,"adcLedDim":7100,"adcUpDim":0,"adcDownDim":7100,"pwm":613,"isUp":0,"isForceSlaveMode":0,"AutoModeCheck":0,"lastOnBrightness":11,"lastManualBrightness":11,"lightModel":2}}"#;
        let separator: &[u8] = &[0xaa, 0xaa, 0x00, 0x03, 0x01, 0x9e, 0x00, 0x01, 0xbe, 0x97];
        let part2 = br#"{"method":"getSysSta","pid":"90E5B1B7D37C","pcode":1005,"uid":"90331","UTC":1776612064,"code":200,"msg":"ok","data":{"sys":{"ver":"1.5","hwcode":2,"hwver":"S-TG-A-PCB-V3.2","buildTime":"Jul 11 2024 15:34:59","localtime":"2026-04-19 17:21:04 Sunday CEST","timezone":"Europe/Berlin","TZ":"CET-1CEST,M3.5.0,M10.5.0/3","gmtoff":3600,"upCount":7,"upTime":1337603,"mem":1703,"wifi":{"isConnect":0,"ssid":"Haus","pass":"Haus_2022"},"mqtt":{"isConnect":0},"bluetooth":{"isConnect":1,"connectTime":7}}}}"#;
        let mut out = Vec::new();
        out.extend_from_slice(part1);
        out.extend_from_slice(separator);
        out.extend_from_slice(part2);
        out
    }

    #[test]
    fn parses_two_frames_from_mixed_stream() {
        let frames = parse_stream(&sample_bytes());
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].method, "getDevSta");
        assert_eq!(frames[1].method, "getSysSta");
        assert_eq!(frames[0].pid.as_deref(), Some("90E5B1B7D37C"));
        assert_eq!(frames[0].code, Some(200));
    }

    #[test]
    fn builds_consolidated_status() {
        let frames = parse_stream(&sample_bytes());
        let status = build_status(&frames);
        assert_eq!(status.brightness, Some(60));
        assert_eq!(status.pwm, Some(613));
        assert_eq!(status.mode, Some(1));
        assert_eq!(status.is_screen_on, Some(true));
        assert_eq!(status.fw_version.as_deref(), Some("1.5"));
        assert_eq!(status.hw_version.as_deref(), Some("S-TG-A-PCB-V3.2"));
        assert_eq!(status.wifi_ssid.as_deref(), Some("Haus"));
        assert_eq!(status.bt_connected, Some(true));
    }

    #[test]
    fn handles_empty_and_garbage_input() {
        assert!(parse_stream(&[]).is_empty());
        assert!(parse_stream(b"not json at all").is_empty());
        assert!(parse_stream(&[0xaa, 0xaa, 0x00, 0x03]).is_empty());
    }
}
