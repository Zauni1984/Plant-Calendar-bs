//! VAT ID check against the EU's VIES service. A confirmed ID of a company in
//! another EU country lets us invoice without VAT (reverse charge).
//!
//!   SELLER_VAT_ID   our own VAT ID (e.g. DE354082556); with it VIES returns a
//!                   consultation number, kept as proof of the check

use serde::Deserialize;

const VIES_URL: &str = "https://ec.europa.eu/taxation_customs/vies/rest-api/check-vat-number";

/// EU member states (ISO codes; VIES uses EL for Greece).
pub const EU_COUNTRIES: [&str; 27] = [
    "AT", "BE", "BG", "CY", "CZ", "DE", "DK", "EE", "ES", "FI", "FR", "GR", "HR", "HU", "IE", "IT",
    "LT", "LU", "LV", "MT", "NL", "PL", "PT", "RO", "SE", "SI", "SK",
];

pub fn is_eu(country: &str) -> bool {
    EU_COUNTRIES.contains(&country)
}

/// "de 354 082.556" → ("DE", "354082556"). The country prefix of the ID is
/// required and must match the company's country (EL for Greece).
pub fn split_vat_id(input: &str, country: &str) -> Option<(String, String)> {
    let compact: String = input.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase();
    let prefix = if country == "GR" { "EL" } else { country };
    let number = compact.strip_prefix(prefix)?;
    ((2..=14).contains(&number.len())).then(|| (prefix.to_string(), number.to_string()))
}

#[derive(Debug, PartialEq)]
pub enum ViesResult {
    Valid { request_id: Option<String> },
    Invalid,
    /// VIES or the member state's service is down; try again later.
    Unavailable(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CheckResponse {
    valid: Option<bool>,
    request_identifier: Option<String>,
}

pub async fn check(vies_country: &str, number: &str) -> ViesResult {
    let mut body = serde_json::json!({ "countryCode": vies_country, "vatNumber": number });
    if let Some(own) = std::env::var("SELLER_VAT_ID").ok().filter(|v| v.len() > 2) {
        let own = own.trim().to_ascii_uppercase();
        body["requesterMemberStateCode"] = own[..2].into();
        body["requesterNumber"] = own[2..].into();
    }
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build();
    let Ok(client) = client else { return ViesResult::Unavailable("client".into()) };
    // VIES_URL: only for tests against a stand-in service.
    let url = std::env::var("VIES_URL").ok().filter(|u| !u.is_empty()).unwrap_or_else(|| VIES_URL.to_string());
    let response = match client.post(url).json(&body).send().await {
        Ok(r) => r,
        Err(err) => return ViesResult::Unavailable(err.to_string()),
    };
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    if !status.is_success() {
        // Errors such as MS_UNAVAILABLE, TIMEOUT, MS_MAX_CONCURRENT_REQ; an
        // INVALID_INPUT means the number cannot exist.
        if text.contains("INVALID_INPUT") {
            return ViesResult::Invalid;
        }
        return ViesResult::Unavailable(format!("{status}: {}", text.chars().take(200).collect::<String>()));
    }
    match serde_json::from_str::<CheckResponse>(&text) {
        Ok(CheckResponse { valid: Some(true), request_identifier }) => ViesResult::Valid { request_id: request_identifier.filter(|r| !r.is_empty()) },
        Ok(CheckResponse { valid: Some(false), .. }) => ViesResult::Invalid,
        _ => ViesResult::Unavailable(format!("unexpected answer: {}", text.chars().take(200).collect::<String>())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vat_ids_are_split_by_country() {
        assert_eq!(split_vat_id("atu 123.456-78", "AT"), Some(("AT".into(), "U12345678".into())));
        assert_eq!(split_vat_id("EL123456789", "GR"), Some(("EL".into(), "123456789".into())));
        // Prefix must match the country.
        assert_eq!(split_vat_id("FR12345678901", "AT"), None);
        assert_eq!(split_vat_id("AT", "AT"), None);
        assert!(is_eu("FR") && !is_eu("CH") && !is_eu("GB"));
    }
}
