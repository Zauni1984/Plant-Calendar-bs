//! Lexware Office (Lexoffice) public API: contacts, invoices, invoice PDFs.
//!
//!   LEXOFFICE_API_KEY   from app.lexware.de/addons/public-api; unset = no invoices
//!   LEXOFFICE_API_URL   default https://api.lexware.io
//!
//! The API allows 2 requests per second; calls are spaced and a 429 is
//! retried after a pause.

use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration, Instant};

const MIN_SPACING: Duration = Duration::from_millis(600);

pub struct Lexoffice {
    client: reqwest::Client,
    base: String,
    key: String,
    last_call: Arc<Mutex<Instant>>,
}

pub struct InvoiceInfo {
    pub voucher_number: Option<String>,
    /// NONE, EN16931 (ZUGFeRD) or XRechnung.
    pub electronic_profile: Option<String>,
}

impl Lexoffice {
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("LEXOFFICE_API_KEY").ok().filter(|k| !k.trim().is_empty())?;
        let base = std::env::var("LEXOFFICE_API_URL")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| "https://api.lexware.io".into());
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30)).build().ok()?;
        Some(Self {
            client,
            base: base.trim_end_matches('/').to_string(),
            key: key.trim().to_string(),
            last_call: Arc::new(Mutex::new(Instant::now() - MIN_SPACING)),
        })
    }

    async fn send(&self, build: impl Fn() -> reqwest::RequestBuilder) -> anyhow::Result<reqwest::Response> {
        for attempt in 0..4 {
            {
                let mut last = self.last_call.lock().await;
                let wait = (*last + MIN_SPACING).saturating_duration_since(Instant::now());
                sleep(wait).await;
                *last = Instant::now();
            }
            let response = build().bearer_auth(&self.key).send().await?;
            if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
                sleep(Duration::from_secs(2 << attempt)).await;
                continue;
            }
            if !response.status().is_success() {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                anyhow::bail!("Lexoffice {status}: {}", body.chars().take(400).collect::<String>());
            }
            return Ok(response);
        }
        anyhow::bail!("Lexoffice: rate limit")
    }

    async fn post_json(&self, path: &str, body: &serde_json::Value) -> anyhow::Result<serde_json::Value> {
        let url = format!("{}{path}", self.base);
        let response = self.send(|| self.client.post(&url).json(body).header("Accept", "application/json")).await?;
        Ok(response.json().await?)
    }

    /// A customer contact; returns its id.
    pub async fn create_contact(&self, contact: &serde_json::Value) -> anyhow::Result<String> {
        let reply = self.post_json("/v1/contacts", contact).await?;
        reply["id"].as_str().map(String::from).ok_or_else(|| anyhow::anyhow!("Lexoffice: contact without id"))
    }

    /// A finalized (open) invoice; returns its id.
    pub async fn create_invoice(&self, invoice: &serde_json::Value) -> anyhow::Result<String> {
        let reply = self.post_json("/v1/invoices?finalize=true", invoice).await?;
        reply["id"].as_str().map(String::from).ok_or_else(|| anyhow::anyhow!("Lexoffice: invoice without id"))
    }

    pub async fn invoice_info(&self, id: &str) -> anyhow::Result<InvoiceInfo> {
        let url = format!("{}/v1/invoices/{id}", self.base);
        let reply: serde_json::Value = self.send(|| self.client.get(&url).header("Accept", "application/json")).await?.json().await?;
        Ok(InvoiceInfo {
            voucher_number: reply["voucherNumber"].as_str().map(String::from),
            electronic_profile: reply["electronicDocumentProfile"].as_str().map(String::from),
        })
    }

    /// The invoice PDF (for ZUGFeRD with the embedded e-invoice data).
    pub async fn invoice_pdf(&self, id: &str) -> anyhow::Result<Vec<u8>> {
        let url = format!("{}/v1/invoices/{id}/file", self.base);
        let response = self.send(|| self.client.get(&url).header("Accept", "application/pdf")).await?;
        Ok(response.bytes().await?.to_vec())
    }
}
