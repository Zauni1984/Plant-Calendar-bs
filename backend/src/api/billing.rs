//! Billing data and invoices.
//!
//!   GET/PUT/DELETE /me/billing     invoice recipient (private or company)
//!   GET  /invoices                 the account's invoices
//!   GET  /invoices/{id}/pdf        PDF from Lexware Office
//!
//! Every payment becomes an invoice: the Stripe webhook records it here
//! (`record_*`), a background job creates it in Lexware Office (finalized, so
//! Lexware assigns the number), fetches the PDF and mails it to the customer.
//! Failures are retried with growing pauses.
//!
//! Tax: consumers and German companies pay 19 % German VAT (gross prices).
//! Companies in another EU country with a VAT ID confirmed by VIES are
//! invoiced without VAT (reverse charge, Lexware "intraCommunitySupply");
//! companies outside the EU without VAT ("thirdPartyCountryService"). Both
//! need a Lexware contact, created on first use.

use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::{DateTime, Duration, SecondsFormat, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::api_error;
use crate::{
    db::{repos, AppState},
    services::{auth::SessionUser, mail, vies},
};

/// Re-check a VAT ID with VIES when the last check is older than this.
const VAT_RECHECK_DAYS: i64 = 30;
const MAX_ATTEMPTS: i32 = 12;
const VAT_RATE: f64 = 19.0;

// --- billing profile ---------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Recipient {
    pub is_company: bool,
    pub name: String,
    #[serde(default)]
    pub street: String,
    #[serde(default)]
    pub zip: String,
    #[serde(default)]
    pub city: String,
    pub country_code: String,
    #[serde(default)]
    pub vat_id: Option<String>,
    #[serde(default)]
    pub vat_valid: bool,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TaxTreatment {
    /// 19 % German VAT, gross prices.
    Gross,
    /// EU company with confirmed VAT ID: no VAT, reverse charge.
    ReverseCharge,
    /// Company outside the EU: no German VAT.
    ThirdCountry,
}

impl TaxTreatment {
    fn lexoffice(self) -> &'static str {
        match self {
            TaxTreatment::Gross => "gross",
            TaxTreatment::ReverseCharge => "intraCommunitySupply",
            TaxTreatment::ThirdCountry => "thirdPartyCountryService",
        }
    }
    fn api_name(self) -> &'static str {
        match self {
            TaxTreatment::Gross => "vat_19",
            TaxTreatment::ReverseCharge => "reverse_charge",
            TaxTreatment::ThirdCountry => "third_country",
        }
    }
}

pub fn tax_treatment(r: &Recipient) -> TaxTreatment {
    if !r.is_company || r.country_code == "DE" {
        TaxTreatment::Gross
    } else if vies::is_eu(&r.country_code) {
        if r.vat_valid { TaxTreatment::ReverseCharge } else { TaxTreatment::Gross }
    } else {
        TaxTreatment::ThirdCountry
    }
}

#[derive(sqlx::FromRow)]
struct ProfileRow {
    is_company: bool,
    name: String,
    street: String,
    zip: String,
    city: String,
    country_code: String,
    vat_id: Option<String>,
    vat_valid: bool,
    vat_checked_at: Option<DateTime<Utc>>,
    lexoffice_contact_id: Option<String>,
}

impl ProfileRow {
    fn recipient(&self) -> Recipient {
        Recipient {
            is_company: self.is_company,
            name: self.name.clone(),
            street: self.street.clone(),
            zip: self.zip.clone(),
            city: self.city.clone(),
            country_code: self.country_code.clone(),
            vat_id: self.vat_id.clone(),
            vat_valid: self.vat_valid,
        }
    }
}

async fn load_profile(state: &AppState, user_id: Uuid) -> anyhow::Result<Option<ProfileRow>> {
    Ok(sqlx::query_as(
        "SELECT is_company, name, street, zip, city, country_code, vat_id, vat_valid, vat_checked_at, lexoffice_contact_id \
         FROM billing_profiles WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.pg)
    .await?)
}

fn profile_json(p: &ProfileRow) -> serde_json::Value {
    let r = p.recipient();
    serde_json::json!({
        "is_company": r.is_company, "name": r.name, "street": r.street, "zip": r.zip, "city": r.city,
        "country_code": r.country_code, "vat_id": r.vat_id, "vat_valid": r.vat_valid,
        "vat_checked_at": p.vat_checked_at, "tax_treatment": tax_treatment(&r).api_name(),
    })
}

pub async fn get_billing(State(state): State<AppState>, session: SessionUser) -> Response {
    match load_profile(&state, session.user_id).await {
        Ok(Some(p)) => Json(profile_json(&p)).into_response(),
        Ok(None) => Json(serde_json::Value::Null).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

#[derive(Deserialize)]
pub struct BillingPayload {
    #[serde(default)]
    pub is_company: bool,
    pub name: String,
    #[serde(default)]
    pub street: String,
    #[serde(default)]
    pub zip: String,
    #[serde(default)]
    pub city: String,
    pub country_code: String,
    #[serde(default)]
    pub vat_id: Option<String>,
}

fn billing_error(status: StatusCode, code: &str) -> Response {
    (status, Json(serde_json::json!({ "code": code, "message": code }))).into_response()
}

pub async fn put_billing(State(state): State<AppState>, session: SessionUser, Json(p): Json<BillingPayload>) -> Response {
    let clean = |s: &str, max: usize| s.trim().chars().take(max).collect::<String>();
    let name = clean(&p.name, 200);
    let street = clean(&p.street, 200);
    let zip = clean(&p.zip, 20);
    let city = clean(&p.city, 100);
    let country = p.country_code.trim().to_ascii_uppercase();
    if name.is_empty() {
        return billing_error(StatusCode::BAD_REQUEST, "name_required");
    }
    if country.len() != 2 || !country.bytes().all(|b| b.is_ascii_uppercase()) {
        return billing_error(StatusCode::BAD_REQUEST, "country_invalid");
    }
    if p.is_company && (street.is_empty() || zip.is_empty() || city.is_empty()) {
        return billing_error(StatusCode::BAD_REQUEST, "address_required");
    }
    let vat_input = p.vat_id.as_deref().map(str::trim).filter(|v| !v.is_empty() && p.is_company);

    // EU company outside Germany: the VAT ID decides about reverse charge.
    let (mut vat_id, mut vat_valid, mut checked_at, mut request_id) = (vat_input.map(|v| v.to_ascii_uppercase()), false, None, None);
    let mut warning = None;
    if let Some(input) = vat_input {
        if vies::is_eu(&country) {
            let Some((prefix, number)) = vies::split_vat_id(input, &country) else {
                return billing_error(StatusCode::UNPROCESSABLE_ENTITY, "vat_country_mismatch");
            };
            vat_id = Some(format!("{prefix}{number}"));
            if country != "DE" {
                match vies::check(&prefix, &number).await {
                    vies::ViesResult::Valid { request_id: rid } => {
                        vat_valid = true;
                        checked_at = Some(Utc::now());
                        request_id = rid;
                    }
                    vies::ViesResult::Invalid => return billing_error(StatusCode::UNPROCESSABLE_ENTITY, "vat_invalid"),
                    vies::ViesResult::Unavailable(reason) => {
                        // Saved unconfirmed; the invoice job checks again.
                        tracing::warn!("VIES unavailable: {reason}");
                        warning = Some("vat_unavailable");
                    }
                }
            }
        } else {
            vat_id = Some(clean(input, 30));
        }
    }

    let saved = sqlx::query(
        "INSERT INTO billing_profiles (user_id, is_company, name, street, zip, city, country_code, vat_id, vat_valid, vat_checked_at, vat_request_id, lexoffice_contact_id, updated_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,NULL,NOW()) \
         ON CONFLICT (user_id) DO UPDATE SET is_company = $2, name = $3, street = $4, zip = $5, city = $6, country_code = $7, \
            vat_id = $8, vat_valid = $9, vat_checked_at = $10, vat_request_id = $11, lexoffice_contact_id = NULL, updated_at = NOW()",
    )
    .bind(session.user_id)
    .bind(p.is_company)
    .bind(&name)
    .bind(&street)
    .bind(&zip)
    .bind(&city)
    .bind(&country)
    .bind(&vat_id)
    .bind(vat_valid)
    .bind(checked_at)
    .bind(&request_id)
    .execute(&state.pg)
    .await;
    if let Err(err) = saved {
        return api_error(StatusCode::INTERNAL_SERVER_ERROR, err);
    }
    match load_profile(&state, session.user_id).await {
        Ok(Some(row)) => {
            let mut body = profile_json(&row);
            body["warning"] = warning.into();
            Json(body).into_response()
        }
        Ok(None) => api_error(StatusCode::INTERNAL_SERVER_ERROR, "profile vanished"),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn delete_billing(State(state): State<AppState>, session: SessionUser) -> Response {
    match sqlx::query("DELETE FROM billing_profiles WHERE user_id = $1").bind(session.user_id).execute(&state.pg).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

// --- invoices for the account --------------------------------------------------------

#[derive(sqlx::FromRow, Serialize)]
pub struct InvoiceView {
    invoice_id: Uuid,
    kind: String,
    description: String,
    amount_cents: i64,
    currency: String,
    paid_at: DateTime<Utc>,
    voucher_number: Option<String>,
    tax_type: Option<String>,
    status: String,
}

pub async fn list_for_user(state: &AppState, user_id: Uuid) -> anyhow::Result<Vec<InvoiceView>> {
    Ok(sqlx::query_as(
        "SELECT invoice_id, kind, description, amount_cents, currency, paid_at, voucher_number, tax_type, status \
         FROM invoices WHERE user_id = $1 ORDER BY paid_at DESC LIMIT 500",
    )
    .bind(user_id)
    .fetch_all(&state.pg)
    .await?)
}

pub async fn list_invoices(State(state): State<AppState>, session: SessionUser) -> Response {
    match list_for_user(&state, session.user_id).await {
        Ok(rows) => Json(rows).into_response(),
        Err(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    }
}

pub async fn invoice_pdf(State(state): State<AppState>, session: SessionUser, Path(id): Path<Uuid>) -> Response {
    let row: Option<(Option<String>, Option<String>)> = match sqlx::query_as(
        "SELECT lexoffice_id, voucher_number FROM invoices WHERE invoice_id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(session.user_id)
    .fetch_optional(&state.pg)
    .await
    {
        Ok(r) => r,
        Err(err) => return api_error(StatusCode::INTERNAL_SERVER_ERROR, err),
    };
    let Some((Some(lexoffice_id), number)) = row else {
        return api_error(StatusCode::NOT_FOUND, "invoice not available yet");
    };
    let Some(lex) = state.lexoffice.clone() else {
        return api_error(StatusCode::SERVICE_UNAVAILABLE, "invoices are not configured");
    };
    match lex.invoice_pdf(&lexoffice_id).await {
        Ok(pdf) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/pdf"));
            let filename = format!("Sowling-Rechnung-{}.pdf", number.unwrap_or_else(|| id.to_string()));
            if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", filename.replace('"', ""))) {
                headers.insert(header::CONTENT_DISPOSITION, v);
            }
            (headers, pdf).into_response()
        }
        Err(err) => {
            tracing::warn!("invoice pdf: {err}");
            api_error(StatusCode::BAD_GATEWAY, "invoice service temporarily unavailable")
        }
    }
}

// --- recording payments (Stripe webhook) ------------------------------------------------

fn stripe_time(v: &serde_json::Value) -> Option<DateTime<Utc>> {
    v.as_i64().and_then(|t| Utc.timestamp_opt(t, 0).single())
}

/// Recipient from Stripe's customer details, for accounts without a profile.
fn recipient_from_stripe(name: Option<&str>, email: Option<&str>, address: &serde_json::Value) -> Recipient {
    let line1 = address["line1"].as_str().unwrap_or_default();
    let line2 = address["line2"].as_str().unwrap_or_default();
    Recipient {
        is_company: false,
        name: name.filter(|n| !n.trim().is_empty()).or(email).unwrap_or("Kunde").trim().to_string(),
        street: [line1, line2].iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>().join(", "),
        zip: address["postal_code"].as_str().unwrap_or_default().to_string(),
        city: address["city"].as_str().unwrap_or_default().to_string(),
        country_code: address["country"].as_str().filter(|c| c.len() == 2).unwrap_or("DE").to_string(),
        vat_id: None,
        vat_valid: false,
    }
}

struct Payment<'a> {
    stripe_ref: &'a str,
    user_id: Option<Uuid>,
    kind: &'a str,
    description: String,
    amount_cents: i64,
    currency: String,
    paid_at: DateTime<Utc>,
    period: Option<(DateTime<Utc>, DateTime<Utc>)>,
    email: Option<String>,
    stripe_recipient: Recipient,
}

async fn record(state: &AppState, p: Payment<'_>) -> anyhow::Result<()> {
    if p.amount_cents <= 0 {
        return Ok(()); // nothing paid, nothing to invoice (e.g. 100 % coupon)
    }
    let recipient = match p.user_id {
        Some(uid) => load_profile(state, uid).await?.map(|row| row.recipient()).unwrap_or(p.stripe_recipient),
        None => p.stripe_recipient,
    };
    sqlx::query(
        "INSERT INTO invoices (invoice_id, user_id, stripe_ref, kind, description, amount_cents, currency, paid_at, period_start, period_end, email, recipient) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12) ON CONFLICT (stripe_ref) DO NOTHING",
    )
    .bind(Uuid::new_v4())
    .bind(p.user_id)
    .bind(p.stripe_ref)
    .bind(p.kind)
    .bind(&p.description)
    .bind(p.amount_cents)
    .bind(p.currency.to_ascii_uppercase())
    .bind(p.paid_at)
    .bind(p.period.map(|x| x.0))
    .bind(p.period.map(|x| x.1))
    .bind(&p.email)
    .bind(serde_json::to_value(&recipient)?)
    .execute(&state.pg)
    .await?;
    Ok(())
}

/// One-time desktop purchase (`checkout.session.completed`, mode payment).
pub async fn record_desktop_purchase(state: &AppState, session: &serde_json::Value) {
    let Some(stripe_ref) = session["id"].as_str() else { return };
    let details = &session["customer_details"];
    let email = details["email"].as_str().or(session["customer_email"].as_str());
    let payment = Payment {
        stripe_ref,
        user_id: session["client_reference_id"].as_str().and_then(|s| Uuid::parse_str(s).ok()),
        kind: "desktop",
        description: "Sowling Desktop-App (Lizenz für bis zu 3 Geräte)".into(),
        amount_cents: session["amount_total"].as_i64().unwrap_or(0),
        currency: session["currency"].as_str().unwrap_or("eur").into(),
        paid_at: stripe_time(&session["created"]).unwrap_or_else(Utc::now),
        period: None,
        email: email.map(String::from),
        stripe_recipient: recipient_from_stripe(details["name"].as_str(), email, &details["address"]),
    };
    if let Err(err) = record(state, payment).await {
        tracing::error!("could not record desktop purchase {stripe_ref} for invoicing: {err}");
    }
}

/// The price id of a Stripe invoice line (field moved in newer API versions).
fn line_price_id(line: &serde_json::Value) -> Option<&str> {
    line["price"]["id"].as_str().or_else(|| line["pricing"]["price_details"]["price"].as_str())
}

/// A paid subscription invoice (`invoice.paid`): first payment and every
/// renewal. Also keeps the account's plan in line with what was paid for
/// (upgrades and downgrades through the customer portal).
pub async fn record_subscription_payment(state: &AppState, invoice: &serde_json::Value) {
    let Some(stripe_ref) = invoice["id"].as_str() else { return };
    let customer = invoice["customer"].as_str().unwrap_or_default();
    let email = invoice["customer_email"].as_str();
    let mut user_id: Option<Uuid> = None;
    if !customer.is_empty() {
        user_id = sqlx::query_scalar("SELECT user_id FROM users WHERE stripe_customer_id = $1")
            .bind(customer)
            .fetch_optional(&state.pg)
            .await
            .ok()
            .flatten();
    }
    if user_id.is_none() {
        if let Some(e) = email {
            user_id = repos::get_user_by_email(state, e).await.ok().flatten().map(|u| u.user_id);
            if let (Some(uid), false) = (user_id, customer.is_empty()) {
                let _ = repos::set_stripe_customer_id(state, uid, customer).await;
            }
        }
    }
    let line = &invoice["lines"]["data"][0];
    let plan = line_price_id(line).and_then(super::plan_for_price);
    if let (Some(uid), Some(plan)) = (user_id, plan) {
        if let Err(err) = repos::set_stripe_plan(state, uid, Some(plan)).await {
            tracing::error!(%uid, "invoice.paid: could not apply plan: {err}");
        }
    }
    let plan_name = match plan {
        Some(shared::SubscriptionPlan::Nursery) => "Gärtnerei",
        Some(shared::SubscriptionPlan::Pro) => "Pro",
        Some(shared::SubscriptionPlan::Basic) => "Basic",
        _ => "",
    };
    let description = if plan_name.is_empty() {
        line["description"].as_str().unwrap_or("Sowling Abo").to_string()
    } else {
        format!("Sowling {plan_name} (Abo)")
    };
    let period = match (stripe_time(&line["period"]["start"]), stripe_time(&line["period"]["end"])) {
        (Some(a), Some(b)) if b > a => Some((a, b)),
        _ => None,
    };
    let payment = Payment {
        stripe_ref,
        user_id,
        kind: "subscription",
        description,
        amount_cents: invoice["amount_paid"].as_i64().unwrap_or(0),
        currency: invoice["currency"].as_str().unwrap_or("eur").into(),
        paid_at: stripe_time(&invoice["status_transitions"]["paid_at"]).or_else(|| stripe_time(&invoice["created"])).unwrap_or_else(Utc::now),
        period,
        email: email.map(String::from),
        stripe_recipient: recipient_from_stripe(invoice["customer_name"].as_str(), email, &invoice["customer_address"]),
    };
    if let Err(err) = record(state, payment).await {
        tracing::error!("could not record subscription payment {stripe_ref} for invoicing: {err}");
    }
}

// --- the invoice job ------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct InvoiceRow {
    invoice_id: Uuid,
    user_id: Option<Uuid>,
    kind: String,
    description: String,
    amount_cents: i64,
    currency: String,
    paid_at: DateTime<Utc>,
    period_start: Option<DateTime<Utc>>,
    period_end: Option<DateTime<Utc>>,
    email: Option<String>,
    recipient: serde_json::Value,
    tax_type: Option<String>,
    status: String,
    attempts: i32,
    lexoffice_id: Option<String>,
}

fn lex_date(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(SecondsFormat::Millis, false)
}

fn money(cents: i64) -> f64 {
    cents as f64 / 100.0
}

/// The Lexware invoice body.
pub fn invoice_body(
    description: &str,
    amount_cents: i64,
    currency: &str,
    paid_at: DateTime<Utc>,
    period: Option<(DateTime<Utc>, DateTime<Utc>)>,
    recipient: &Recipient,
    treatment: TaxTreatment,
    contact_id: Option<&str>,
) -> serde_json::Value {
    let address = match contact_id {
        Some(id) => serde_json::json!({ "contactId": id }),
        None => serde_json::json!({
            "name": recipient.name, "street": recipient.street, "zip": recipient.zip,
            "city": recipient.city, "countryCode": recipient.country_code,
        }),
    };
    let unit_price = match treatment {
        TaxTreatment::Gross => serde_json::json!({ "currency": currency, "grossAmount": money(amount_cents), "taxRatePercentage": VAT_RATE }),
        _ => serde_json::json!({ "currency": currency, "netAmount": money(amount_cents), "taxRatePercentage": 0 }),
    };
    let (shipping, period_text) = match period {
        // Stripe periods end at the start of the next one.
        Some((start, end)) => {
            let last_day = end - Duration::days(1);
            (
                serde_json::json!({ "shippingType": "serviceperiod", "shippingDate": lex_date(start), "shippingEndDate": lex_date(last_day.max(start)) }),
                Some(format!("Leistungszeitraum {} – {}", start.format("%d.%m.%Y"), last_day.max(start).format("%d.%m.%Y"))),
            )
        }
        None => (serde_json::json!({ "shippingType": "service", "shippingDate": lex_date(paid_at) }), None),
    };
    let mut line = serde_json::json!({
        "type": "custom", "name": description, "quantity": 1, "unitName": "Stück", "unitPrice": unit_price,
    });
    if let Some(text) = period_text {
        line["description"] = text.into();
    }
    serde_json::json!({
        "voucherDate": lex_date(paid_at),
        "address": address,
        "lineItems": [line],
        "totalPrice": { "currency": currency },
        "taxConditions": { "taxType": treatment.lexoffice() },
        "paymentConditions": {
            "paymentTermLabel": format!("Bereits bezahlt am {} (Stripe).", paid_at.format("%d.%m.%Y")),
            "paymentTermDuration": 0,
        },
        "shippingConditions": shipping,
        "title": "Rechnung",
        "introduction": "Vielen Dank für deinen Einkauf bei Sowling.",
        "remark": "Der Rechnungsbetrag wurde bereits bezahlt.",
    })
}

fn contact_body(r: &Recipient, email: Option<&str>) -> serde_json::Value {
    let mut contact = serde_json::json!({
        "version": 0,
        "roles": { "customer": {} },
        "company": { "name": r.name, "allowTaxFreeInvoices": true },
        "addresses": { "billing": [{ "street": r.street, "zip": r.zip, "city": r.city, "countryCode": r.country_code }] },
    });
    if let Some(v) = &r.vat_id {
        contact["company"]["vatRegistrationId"] = v.clone().into();
    }
    if let Some(e) = email {
        contact["emailAddresses"] = serde_json::json!({ "business": [e] });
    }
    contact
}

/// Every minute: create, fetch and send due invoices.
pub async fn run_due(state: &AppState) -> anyhow::Result<usize> {
    let Some(lex) = state.lexoffice.clone() else { return Ok(0) };
    let rows: Vec<InvoiceRow> = sqlx::query_as(
        "SELECT invoice_id, user_id, kind, description, amount_cents, currency, paid_at, period_start, period_end, email, \
                recipient, tax_type, status, attempts, lexoffice_id \
         FROM invoices WHERE status IN ('pending', 'created') AND next_attempt_at <= NOW() ORDER BY paid_at LIMIT 20",
    )
    .fetch_all(&state.pg)
    .await?;
    let mut done = 0;
    for row in rows {
        let id = row.invoice_id;
        let attempts = row.attempts;
        match process(state, &lex, row).await {
            Ok(()) => done += 1,
            Err(err) => {
                let attempts = attempts + 1;
                let failed = attempts >= MAX_ATTEMPTS;
                // 2, 4, 8 … minutes, at most 6 hours.
                let pause = Duration::minutes((1i64 << attempts.min(9)).min(360));
                tracing::warn!(invoice = %id, attempts, "invoice not done yet: {err}");
                if failed {
                    tracing::error!(invoice = %id, "invoice failed after {attempts} attempts: {err}");
                }
                sqlx::query(
                    "UPDATE invoices SET attempts = $2, last_error = $3, next_attempt_at = NOW() + $4, \
                        status = CASE WHEN $5 THEN 'failed' ELSE status END WHERE invoice_id = $1",
                )
                .bind(id)
                .bind(attempts)
                .bind(err.to_string().chars().take(1000).collect::<String>())
                .bind(pause)
                .bind(failed)
                .execute(&state.pg)
                .await?;
            }
        }
    }
    Ok(done)
}

/// Re-check a stored VAT ID with VIES when it is unconfirmed or old.
async fn fresh_vat_status(state: &AppState, user_id: Option<Uuid>, r: &mut Recipient) -> anyhow::Result<()> {
    if !r.is_company || r.country_code == "DE" || !vies::is_eu(&r.country_code) {
        return Ok(());
    }
    let Some((prefix, number)) = r.vat_id.as_deref().and_then(|v| vies::split_vat_id(v, &r.country_code)) else {
        r.vat_valid = false;
        return Ok(());
    };
    let profile = match user_id {
        Some(uid) => load_profile(state, uid).await?,
        None => None,
    };
    let checked = profile.as_ref().and_then(|p| p.vat_checked_at.filter(|_| p.vat_valid && p.vat_id == r.vat_id));
    if checked.is_some_and(|t| Utc::now() - t < Duration::days(VAT_RECHECK_DAYS)) {
        r.vat_valid = true;
        return Ok(());
    }
    match vies::check(&prefix, &number).await {
        vies::ViesResult::Valid { request_id } => {
            r.vat_valid = true;
            if let Some(uid) = user_id {
                sqlx::query("UPDATE billing_profiles SET vat_valid = TRUE, vat_checked_at = NOW(), vat_request_id = $2 WHERE user_id = $1 AND vat_id = $3")
                    .bind(uid)
                    .bind(request_id)
                    .bind(&r.vat_id)
                    .execute(&state.pg)
                    .await?;
            }
            Ok(())
        }
        vies::ViesResult::Invalid => {
            r.vat_valid = false;
            if let Some(uid) = user_id {
                sqlx::query("UPDATE billing_profiles SET vat_valid = FALSE, vat_checked_at = NOW() WHERE user_id = $1 AND vat_id = $2")
                    .bind(uid)
                    .bind(&r.vat_id)
                    .execute(&state.pg)
                    .await?;
            }
            Ok(())
        }
        // Do not fall back to VAT because VIES is down; try again later.
        vies::ViesResult::Unavailable(reason) => anyhow::bail!("VIES unavailable: {reason}"),
    }
}

async fn process(state: &AppState, lex: &crate::services::lexoffice::Lexoffice, row: InvoiceRow) -> anyhow::Result<()> {
    let mut recipient: Recipient = serde_json::from_value(row.recipient.clone())?;
    let mut lexoffice_id = row.lexoffice_id.clone();
    let mut tax_type = row.tax_type.clone();

    if row.status == "pending" {
        fresh_vat_status(state, row.user_id, &mut recipient).await?;
        let treatment = tax_treatment(&recipient);
        // Tax-free invoices need a Lexware contact (with the VAT ID).
        let contact_id = if treatment == TaxTreatment::Gross {
            None
        } else {
            let stored = match row.user_id {
                Some(uid) => load_profile(state, uid).await?.filter(|p| p.recipient() == recipient).and_then(|p| p.lexoffice_contact_id),
                None => None,
            };
            match stored {
                Some(id) => Some(id),
                None => {
                    let id = lex.create_contact(&contact_body(&recipient, row.email.as_deref())).await?;
                    if let Some(uid) = row.user_id {
                        sqlx::query("UPDATE billing_profiles SET lexoffice_contact_id = $2 WHERE user_id = $1")
                            .bind(uid)
                            .bind(&id)
                            .execute(&state.pg)
                            .await?;
                    }
                    Some(id)
                }
            }
        };
        let period = row.period_start.zip(row.period_end);
        let body = invoice_body(&row.description, row.amount_cents, &row.currency, row.paid_at, period, &recipient, treatment, contact_id.as_deref());
        let id = lex.create_invoice(&body).await?;
        // Record right away: a retry must never create the invoice twice.
        sqlx::query("UPDATE invoices SET status = 'created', lexoffice_id = $2, tax_type = $3, recipient = $4, last_error = NULL WHERE invoice_id = $1")
            .bind(row.invoice_id)
            .bind(&id)
            .bind(treatment.lexoffice())
            .bind(serde_json::to_value(&recipient)?)
            .execute(&state.pg)
            .await?;
        lexoffice_id = Some(id);
        tax_type = Some(treatment.lexoffice().to_string());
    }

    let lexoffice_id = lexoffice_id.ok_or_else(|| anyhow::anyhow!("created invoice without Lexware id"))?;
    let info = lex.invoice_info(&lexoffice_id).await?;
    let number = info.voucher_number.clone().unwrap_or_else(|| row.invoice_id.to_string());
    sqlx::query("UPDATE invoices SET voucher_number = $2 WHERE invoice_id = $1")
        .bind(row.invoice_id)
        .bind(&info.voucher_number)
        .execute(&state.pg)
        .await?;

    let (Some(mailer), Some(to)) = (state.mail.clone(), row.email.clone()) else {
        sqlx::query("UPDATE invoices SET status = 'issued', last_error = NULL WHERE invoice_id = $1").bind(row.invoice_id).execute(&state.pg).await?;
        return Ok(());
    };
    let pdf = lex.invoice_pdf(&lexoffice_id).await?;
    let e_invoice = info.electronic_profile.as_deref() == Some("EN16931");
    let reverse_charge = tax_type.as_deref() == Some(TaxTreatment::ReverseCharge.lexoffice());
    let mail = mail::invoice_mail(&number, &row.description, e_invoice, reverse_charge);
    mailer.send_with_attachment(&to, &mail, &format!("Sowling-Rechnung-{number}.pdf"), "application/pdf", pdf).await?;
    sqlx::query("UPDATE invoices SET status = 'sent', emailed_at = NOW(), last_error = NULL WHERE invoice_id = $1")
        .bind(row.invoice_id)
        .execute(&state.pg)
        .await?;
    tracing::info!(invoice = %row.invoice_id, kind = %row.kind, "invoice {number} sent");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn company(country: &str, vat_valid: bool) -> Recipient {
        Recipient { is_company: true, name: "Firma".into(), country_code: country.into(), vat_id: Some(format!("{country}123")), vat_valid, ..Default::default() }
    }

    #[test]
    fn tax_follows_customer_type_and_country() {
        let private = Recipient { name: "Max".into(), country_code: "FR".into(), ..Default::default() };
        assert_eq!(tax_treatment(&private), TaxTreatment::Gross, "consumers always pay German VAT");
        assert_eq!(tax_treatment(&company("DE", true)), TaxTreatment::Gross);
        assert_eq!(tax_treatment(&company("AT", true)), TaxTreatment::ReverseCharge);
        assert_eq!(tax_treatment(&company("AT", false)), TaxTreatment::Gross, "unconfirmed VAT ID");
        assert_eq!(tax_treatment(&company("CH", false)), TaxTreatment::ThirdCountry);
    }

    #[test]
    fn invoice_bodies_match_the_tax_type() {
        let paid = Utc.with_ymd_and_hms(2026, 11, 1, 10, 0, 0).unwrap();
        let end = Utc.with_ymd_and_hms(2026, 12, 1, 10, 0, 0).unwrap();
        let private = Recipient { name: "Max".into(), country_code: "DE".into(), ..Default::default() };
        let b = invoice_body("Sowling Pro (Abo)", 599, "EUR", paid, Some((paid, end)), &private, TaxTreatment::Gross, None);
        assert_eq!(b["taxConditions"]["taxType"], "gross");
        assert_eq!(b["lineItems"][0]["unitPrice"]["grossAmount"], 5.99);
        assert_eq!(b["lineItems"][0]["unitPrice"]["taxRatePercentage"], 19.0);
        assert_eq!(b["address"]["name"], "Max");
        assert_eq!(b["shippingConditions"]["shippingType"], "serviceperiod");
        assert_eq!(b["lineItems"][0]["description"], "Leistungszeitraum 01.11.2026 – 30.11.2026");
        assert_eq!(b["voucherDate"], "2026-11-01T10:00:00.000+00:00");

        let rc = invoice_body("Sowling Desktop-App", 2999, "EUR", paid, None, &company("AT", true), TaxTreatment::ReverseCharge, Some("c-1"));
        assert_eq!(rc["taxConditions"]["taxType"], "intraCommunitySupply");
        assert_eq!(rc["lineItems"][0]["unitPrice"]["netAmount"], 29.99);
        assert_eq!(rc["lineItems"][0]["unitPrice"]["taxRatePercentage"], 0);
        assert_eq!(rc["address"], serde_json::json!({ "contactId": "c-1" }));
        assert_eq!(rc["shippingConditions"]["shippingType"], "service");
    }

    #[test]
    fn stripe_details_fill_a_recipient() {
        let address = serde_json::json!({ "line1": "Rue 1", "line2": "", "postal_code": "75001", "city": "Paris", "country": "FR" });
        let r = recipient_from_stripe(Some("Anne"), Some("a@example.com"), &address);
        assert_eq!((r.name.as_str(), r.street.as_str(), r.country_code.as_str()), ("Anne", "Rue 1", "FR"));
        let r = recipient_from_stripe(None, Some("a@example.com"), &serde_json::Value::Null);
        assert_eq!((r.name.as_str(), r.country_code.as_str()), ("a@example.com", "DE"));
    }
}
