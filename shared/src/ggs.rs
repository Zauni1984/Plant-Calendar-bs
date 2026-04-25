use serde::{Deserialize, Serialize};

/// Raw JSON response from GGS Controller BLE notifications (FF01)
#[derive(Debug, Clone, Deserialize)]
pub struct GgsResponse {
    pub method: String,
    pub code: u16,
    pub data: Option<GgsData>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GgsData {
    pub sensor: Option<GgsSensor>,
    pub fan: Option<GgsFan>,
    pub light: Option<GgsLight>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GgsSensor {
    pub temp: Option<f32>,
    pub humi: Option<f32>,
    pub vpd: Option<f32>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GgsFan {
    pub on: Option<u8>,
    pub level: Option<u8>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GgsLight {
    pub level: Option<u8>,
}

/// Parsed telemetry snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GgsTelemetry {
    pub temp: Option<f32>,
    pub humidity: Option<f32>,
    pub vpd: Option<f32>,
    pub fan_on: bool,
    pub fan_level: u8,
    pub light_level: u8,
}

/// Parse raw BLE notification bytes into telemetry.
/// GGS sends fragmented messages with possible binary garbage interspersed.
/// This parser is deliberately lenient: it strips all non-ASCII bytes,
/// then finds the outermost JSON object in the cleaned string.
pub fn parse_ggs_notification(raw: &[u8]) -> Result<GgsTelemetry, String> {
    // Step 1: Strip all non-printable-ASCII bytes, keep only 0x20..=0x7E
    // This removes binary garbage while preserving JSON characters.
    let cleaned: Vec<u8> = raw.iter()
        .copied()
        .filter(|&b| (0x20..=0x7E).contains(&b))
        .collect();

    let text = std::str::from_utf8(&cleaned)
        .map_err(|e| format!("UTF-8 conversion failed after cleanup: {e}"))?;

    // Step 2: Find outermost { ... } with brace matching
    let json_str = extract_json_object(text)
        .ok_or_else(|| format!(
            "no complete JSON object found in BLE data ({} raw bytes, {} cleaned chars)",
            raw.len(), cleaned.len()
        ))?;

    // Step 3: Parse
    let response: GgsResponse = serde_json::from_str(json_str)
        .map_err(|e| format!("JSON parse error: {e} — cleaned: {}", truncate(json_str, 200)))?;

    let data = response.data.ok_or_else(|| "no data in GGS response".to_string())?;
    let sensor = data.sensor.unwrap_or(GgsSensor { temp: None, humi: None, vpd: None });
    let fan = data.fan.unwrap_or(GgsFan { on: None, level: None });
    let light = data.light.unwrap_or(GgsLight { level: None });

    Ok(GgsTelemetry {
        temp: sensor.temp,
        humidity: sensor.humi,
        vpd: sensor.vpd,
        fan_on: fan.on.unwrap_or(0) != 0,
        fan_level: fan.level.unwrap_or(0),
        light_level: light.level.unwrap_or(0),
    })
}

/// Find the first complete top-level JSON object in a string via brace counting.
fn extract_json_object(s: &str) -> Option<&str> {
    let bytes = s.as_bytes();
    let start = bytes.iter().position(|&b| b == b'{')?;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escape_next = false;

    for (i, &b) in bytes[start..].iter().enumerate() {
        if escape_next {
            escape_next = false;
            continue;
        }
        match b {
            b'\\' if in_string => { escape_next = true; }
            b'"' => { in_string = !in_string; }
            b'{' if !in_string => { depth += 1; }
            b'}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&s[start..start + i + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_clean_json() {
        let raw = br#"{"method":"getDevSta","code":200,"data":{"sensor":{"temp":23.3,"humi":37.7,"vpd":1.78},"fan":{"on":1,"level":5},"light":{"level":26}}}"#;
        let t = parse_ggs_notification(raw).unwrap();
        assert!((t.temp.unwrap() - 23.3).abs() < 0.01);
        assert!((t.humidity.unwrap() - 37.7).abs() < 0.01);
        assert!((t.vpd.unwrap() - 1.78).abs() < 0.01);
        assert!(t.fan_on);
        assert_eq!(t.fan_level, 5);
        assert_eq!(t.light_level, 26);
    }

    #[test]
    fn parse_with_binary_prefix() {
        let mut data = vec![0x00, 0xFF, 0xAB, 0x12]; // garbage bytes
        data.extend_from_slice(br#"{"method":"getDevSta","code":200,"data":{"sensor":{"temp":25.0,"humi":50.0,"vpd":1.2},"fan":{"on":0,"level":0},"light":{"level":0}}}"#);
        let t = parse_ggs_notification(&data).unwrap();
        assert!((t.temp.unwrap() - 25.0).abs() < 0.01);
        assert!(!t.fan_on);
    }

    #[test]
    fn parse_partial_data() {
        let raw = br#"{"method":"getDevSta","code":200,"data":{"sensor":{"temp":20.0}}}"#;
        let t = parse_ggs_notification(raw).unwrap();
        assert!((t.temp.unwrap() - 20.0).abs() < 0.01);
        assert!(t.humidity.is_none());
        assert!(!t.fan_on);
    }

    #[test]
    fn reject_no_json() {
        let raw = &[0x00, 0xFF, 0xAB];
        assert!(parse_ggs_notification(raw).is_err());
    }

    #[test]
    fn parse_with_interspersed_binary() {
        // Simulate fragmented BLE: binary garbage mixed into JSON
        let mut data = vec![0x00, 0xFF, 0xFE, 0x80]; // garbage prefix
        data.extend_from_slice(b"{\"method\":\"getDevSta\",\"code\":");
        data.extend_from_slice(&[0x00, 0xFF]); // garbage mid-stream (will be stripped)
        data.extend_from_slice(b"200,\"data\":{\"sensor\":{\"temp\":22.5,\"humi\":45.0,\"vpd\":1.1},\"fan\":{\"on\":1,\"level\":3},\"light\":{\"level\":50}}}");
        data.extend_from_slice(&[0xAB, 0xCD]); // garbage suffix
        let t = parse_ggs_notification(&data).unwrap();
        assert!((t.temp.unwrap() - 22.5).abs() < 0.01);
        assert!((t.humidity.unwrap() - 45.0).abs() < 0.01);
        assert!(t.fan_on);
        assert_eq!(t.fan_level, 3);
        assert_eq!(t.light_level, 50);
    }
}
