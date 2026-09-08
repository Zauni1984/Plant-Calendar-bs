use reqwest::Client;
use sha2::Sha256;
use hmac::{Hmac, Mac};

type HmacSha256 = Hmac<Sha256>;

pub struct StripeClient {
    secret_key: String,
    client: Client,
}

impl StripeClient {
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("STRIPE_SECRET_KEY").ok()?;
        if key.trim().is_empty() {
            return None;
        }
        Some(Self {
            secret_key: key,
            client: Client::new(),
        })
    }

    pub async fn create_checkout_session(
        &self,
        customer_email: &str,
        price_id: &str,
        success_url: &str,
        cancel_url: &str,
    ) -> Result<String, String> {
        let resp = self
            .client
            .post("https://api.stripe.com/v1/checkout/sessions")
            .basic_auth(&self.secret_key, None::<&str>)
            .form(&[
                ("mode", "subscription"),
                ("customer_email", customer_email),
                ("line_items[0][price]", price_id),
                ("line_items[0][quantity]", "1"),
                ("success_url", success_url),
                ("cancel_url", cancel_url),
            ])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!(
                "Stripe error: {}",
                resp.text().await.unwrap_or_default()
            ));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        body["url"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| "no checkout URL in response".into())
    }

    pub fn verify_webhook_signature(
        payload: &[u8],
        sig_header: &str,
        webhook_secret: &str,
    ) -> Result<(), String> {
        let mut timestamp = "";
        let mut signature = "";
        for part in sig_header.split(',') {
            if let Some(t) = part.strip_prefix("t=") {
                timestamp = t;
            }
            if let Some(s) = part.strip_prefix("v1=") {
                signature = s;
            }
        }
        if timestamp.is_empty() || signature.is_empty() {
            return Err("invalid signature format".into());
        }

        // Check timestamp is within 5 minutes
        let ts: i64 = timestamp.parse().map_err(|_| "invalid timestamp".to_string())?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| "system time error".to_string())?
            .as_secs() as i64;
        // saturating_sub-based comparison: a hostile/rolled-over timestamp must
        // not overflow the subtraction before .abs() sees it.
        let skew = if now >= ts { now.saturating_sub(ts) } else { ts.saturating_sub(now) };
        if skew > 300 {
            return Err("webhook timestamp expired (>5min)".to_string());
        }

        let signed_payload = format!(
            "{}.{}",
            timestamp,
            std::str::from_utf8(payload).map_err(|e| e.to_string())?
        );
        let mut mac = HmacSha256::new_from_slice(webhook_secret.as_bytes())
            .map_err(|e| e.to_string())?;
        mac.update(signed_payload.as_bytes());

        // Constant-time compare: a plain `!=` on String leaks how many leading
        // bytes matched, which is enough to forge a signature byte by byte.
        let expected = mac.finalize().into_bytes();
        let provided = match hex::decode(signature) {
            Ok(bytes) => bytes,
            Err(_) => return Err("webhook signature mismatch".into()),
        };
        if provided.len() != expected.len() {
            return Err("webhook signature mismatch".into());
        }
        let mut diff = 0u8;
        for (a, b) in expected.iter().zip(provided.iter()) {
            diff |= a ^ b;
        }
        if diff != 0 {
            return Err("webhook signature mismatch".into());
        }
        Ok(())
    }

    /// Delete a Stripe customer. Used on account erasure (Art. 17 GDPR).
    /// Returns Ok(()) on 200 and also on 404 (customer already gone).
    pub async fn delete_customer(&self, customer_id: &str) -> anyhow::Result<()> {
        let url = format!("https://api.stripe.com/v1/customers/{customer_id}");
        let resp = self
            .client
            .delete(&url)
            .basic_auth(&self.secret_key, None::<&str>)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() || status.as_u16() == 404 {
            return Ok(());
        }
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("Stripe delete_customer failed ({status}): {body}");
    }

    pub async fn create_portal_session(
        &self,
        customer_id: &str,
        return_url: &str,
    ) -> Result<String, String> {
        let resp = self
            .client
            .post("https://api.stripe.com/v1/billing_portal/sessions")
            .basic_auth(&self.secret_key, None::<&str>)
            .form(&[("customer", customer_id), ("return_url", return_url)])
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !resp.status().is_success() {
            return Err(format!(
                "Stripe portal error: {}",
                resp.text().await.unwrap_or_default()
            ));
        }

        let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
        body["url"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| "no portal URL in response".into())
    }
}
