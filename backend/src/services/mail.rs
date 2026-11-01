//! Outgoing e-mail over SMTP (Hostinger: smtp.hostinger.com).
//!
//!   SMTP_HOST      e.g. smtp.hostinger.com; unset (or no password) = no e-mail
//!   SMTP_PORT      465 (TLS from the start, default) or 587 (STARTTLS)
//!   SMTP_USER      e.g. service@sowling.com
//!   SMTP_PASSWORD
//!   MAIL_FROM      e.g. Sowling <service@sowling.com>; default SMTP_USER
//!
//! Only transactional mail: license keys and voucher codes. No newsletter,
//! no tracking. Texts are German with an English part below.

use lettre::{
    message::{header::ContentType, Attachment, Mailbox, MultiPart, SinglePart},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};

pub struct Mailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl Mailer {
    /// None when SMTP_HOST is not set (e-mail off).
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let Some(host) = std::env::var("SMTP_HOST").ok().filter(|h| !h.trim().is_empty()) else {
            return Ok(None);
        };
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        // Half-filled settings must not keep the server from starting.
        let (Some(user), Some(password)) = (var("SMTP_USER"), var("SMTP_PASSWORD")) else {
            tracing::warn!("SMTP_HOST is set but SMTP_USER or SMTP_PASSWORD is empty: no e-mail is sent");
            return Ok(None);
        };
        let port: u16 = match std::env::var("SMTP_PORT").ok().filter(|p| !p.trim().is_empty()) {
            Some(p) => p.trim().parse().map_err(|_| anyhow::anyhow!("SMTP_PORT must be a number"))?,
            None => 465,
        };
        let builder = if port == 465 {
            AsyncSmtpTransport::<Tokio1Executor>::relay(host.trim())?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(host.trim())?
        };
        let transport = builder
            .port(port)
            .credentials(Credentials::new(user.clone(), password))
            .timeout(Some(std::time::Duration::from_secs(20)))
            .build();
        let from = std::env::var("MAIL_FROM").ok().filter(|v| !v.trim().is_empty()).unwrap_or(user);
        let from: Mailbox = from.parse().map_err(|e| anyhow::anyhow!("MAIL_FROM is not a valid sender: {e}"))?;
        Ok(Some(Self { transport, from }))
    }

    pub async fn send(&self, to: &str, mail: &Mail) -> anyhow::Result<()> {
        let to: Mailbox = to.parse().map_err(|e| anyhow::anyhow!("invalid recipient: {e}"))?;
        let message = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(&mail.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(mail.body.clone())?;
        self.transport.send(message).await?;
        Ok(())
    }
}

impl Mailer {
    /// Text plus one attachment (an invoice PDF).
    pub async fn send_with_attachment(&self, to: &str, mail: &Mail, filename: &str, content_type: &str, data: Vec<u8>) -> anyhow::Result<()> {
        let to: Mailbox = to.parse().map_err(|e| anyhow::anyhow!("invalid recipient: {e}"))?;
        let content_type = ContentType::parse(content_type).map_err(|e| anyhow::anyhow!("content type: {e}"))?;
        let message = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(&mail.subject)
            .multipart(
                MultiPart::mixed()
                    .singlepart(SinglePart::plain(mail.body.clone()))
                    .singlepart(Attachment::new(filename.to_string()).body(data, content_type)),
            )?;
        self.transport.send(message).await?;
        Ok(())
    }
}

pub struct Mail {
    pub subject: String,
    pub body: String,
}

const FOOTER: &str = "--\nSowling · app.sowling.com\nBlockSocial UG (haftungsbeschränkt) · Kratzmühlstr. 14 · 92339 Beilngries\nservice@sowling.com";

fn web_url() -> String {
    std::env::var("CHECKOUT_BASE_URL").ok().filter(|u| !u.trim().is_empty()).unwrap_or_else(|| "https://app.sowling.com".into())
}

/// The desktop license key, after a purchase or from the operator.
pub fn license_key_mail(key: &str, purchase: bool, max_devices: i32, download_url: Option<&str>) -> Mail {
    let web = web_url();
    let (intro_de, intro_en) = if purchase {
        ("vielen Dank für deinen Kauf der Sowling-Desktop-App.", "thank you for buying the Sowling desktop app.")
    } else {
        ("hier ist dein Lizenzschlüssel für die Sowling-Desktop-App.", "here is your license key for the Sowling desktop app.")
    };
    let download_de = download_url.map(|u| format!("Download: {u}\n\n")).unwrap_or_default();
    let download_en = download_url.map(|u| format!("Download: {u}\n\n")).unwrap_or_default();
    Mail {
        subject: "Dein Sowling-Lizenzschlüssel / Your Sowling license key".into(),
        body: format!(
            "Hallo,\n\n{intro_de}\n\nDein Lizenzschlüssel:\n\n    {key}\n\n\
             So schaltest du die App frei: Desktop-App öffnen und den Schlüssel im Startfenster eingeben. \
             Er gilt für bis zu {max_devices} Geräte. In der Web-App ({web}) findest du ihn jederzeit im Bereich „Desktop-App“.\n\n\
             {download_de}Bitte gib den Schlüssel nicht weiter.\n\nViele Grüße\ndein Sowling-Team\n\n\
             ----------------------------------------\n\n\
             Hello,\n\n{intro_en}\n\nYour license key:\n\n    {key}\n\n\
             To unlock the app, open the desktop app and enter the key in the start window. \
             It works on up to {max_devices} devices. You can always find it in the web app ({web}) under “Desktop app”.\n\n\
             {download_en}Please do not share the key.\n\nBest regards\nThe Sowling team\n\n{FOOTER}\n"
        ),
    }
}

/// A voucher code for one person (influencer, loyal customer).
pub fn voucher_mail(code: &str, plan_de: &str, plan_en: &str, duration_de: &str, duration_en: &str) -> Mail {
    let web = web_url();
    Mail {
        subject: "Dein Sowling-Gutschein / Your Sowling voucher".into(),
        body: format!(
            "Hallo,\n\nhier ist dein Gutschein für Sowling {plan_de} ({duration_de}):\n\n    {code}\n\n\
             So löst du ihn ein: {web} öffnen, anmelden oder ein Konto anlegen, im Bereich „Abo“ \
             auf „Gutschein einlösen“ gehen und den Code eingeben. Die Desktop-App ist inklusive.\n\n\
             Viele Grüße\ndein Sowling-Team\n\n\
             ----------------------------------------\n\n\
             Hello,\n\nhere is your voucher for Sowling {plan_en} ({duration_en}):\n\n    {code}\n\n\
             To redeem it, open {web}, sign in or create an account, go to “Plan” → “Redeem voucher” \
             and enter the code. The desktop app is included.\n\nBest regards\nThe Sowling team\n\n{FOOTER}\n"
        ),
    }
}

/// The invoice as PDF attachment. `e_invoice`: the PDF is a ZUGFeRD e-invoice
/// (EN 16931). `reverse_charge`: invoiced without VAT.
pub fn invoice_mail(number: &str, description: &str, e_invoice: bool, reverse_charge: bool) -> Mail {
    let web = web_url();
    let (e_de, e_en) = if e_invoice {
        (
            "Die Rechnung ist eine elektronische Rechnung nach der EU-Norm EN 16931 (ZUGFeRD): Die Rechnungsdaten sind maschinenlesbar im PDF eingebettet.\n\n",
            "The invoice is an electronic invoice under the EU standard EN 16931 (ZUGFeRD): the invoice data is embedded machine-readably in the PDF.\n\n",
        )
    } else {
        ("", "")
    };
    let (rc_de, rc_en) = if reverse_charge {
        (
            "Steuerschuldnerschaft des Leistungsempfängers (Reverse Charge): Die Rechnung enthält keine Umsatzsteuer.\n\n",
            "Reverse charge: the invoice contains no VAT; VAT is due from the recipient.\n\n",
        )
    } else {
        ("", "")
    };
    Mail {
        subject: format!("Deine Rechnung {number} / Your invoice {number} – Sowling"),
        body: format!(
            "Hallo,\n\nim Anhang findest du die Rechnung {number} für: {description}.\n\
             Der Betrag wurde bereits bezahlt.\n\n{e_de}{rc_de}\
             Alle Rechnungen findest du auch in der Web-App ({web}) unter „Abo“ → „Rechnungen“.\n\n\
             Viele Grüße\ndein Sowling-Team\n\n\
             ----------------------------------------\n\n\
             Hello,\n\nattached is invoice {number} for: {description}.\nThe amount has already been paid.\n\n{e_en}{rc_en}\
             You can also find all invoices in the web app ({web}) under “Plan” → “Invoices”.\n\n\
             Best regards\nThe Sowling team\n\n{FOOTER}\n"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mails_contain_the_code_and_the_company() {
        let m = license_key_mail("SOWL-AAAA-BBBB-CCCC-DDDD", true, 3, Some("https://example.com/setup.exe"));
        assert!(m.body.contains("SOWL-AAAA-BBBB-CCCC-DDDD"));
        assert!(m.body.contains("3 Geräte") && m.body.contains("3 devices"));
        assert!(m.body.contains("https://example.com/setup.exe"));
        assert!(m.body.contains("BlockSocial UG"));
        let v = voucher_mail("7K2M-QX9D-4HRT-B6WN", "Pro", "Pro", "3 Monate", "3 months");
        assert!(v.body.contains("7K2M-QX9D-4HRT-B6WN") && v.body.contains("3 Monate"));
    }

    #[test]
    fn mail_is_off_without_smtp_host() {
        // SMTP_HOST is not set in the test environment.
        if std::env::var("SMTP_HOST").is_err() {
            assert!(Mailer::from_env().unwrap().is_none());
        }
    }
}
