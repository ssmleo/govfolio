//! Alert payload shapes and formatting — design §6.3 honesty rule: every
//! payload carries `verification_state` + extraction confidence + a
//! provenance link. Money is [`ValueInterval`] (decimal strings, invariant 7).

use anyhow::Context as _;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;

use govfolio_core::domain::enums::Currency;
use govfolio_core::domain::value::ValueInterval;

use crate::alerts::email::EmailMessage;

/// The record fields an alert carries (a summary, not the full row — the
/// provenance link leads to everything else).
#[derive(Debug, Clone, Serialize)]
pub struct RecordSummary {
    /// `disclosure_record.id`.
    pub record_id: String,
    /// Source filing (provenance anchor).
    pub filing_id: String,
    /// The politician's id.
    pub politician_id: String,
    /// The politician's canonical name.
    pub politician_name: String,
    /// Record type wire token.
    pub record_type: String,
    /// Asset class wire token.
    pub asset_class: String,
    /// Transaction side wire token, when present.
    pub side: Option<String>,
    /// Asset description exactly as filed (invariant 2).
    pub asset_description_raw: String,
    /// Best event date.
    pub event_date: Option<NaiveDate>,
    /// Declared value band — decimal strings on the wire (invariant 7).
    pub value: Option<ValueInterval>,
    /// Verification state — honesty travels with the fast path (§6.3).
    pub verification_state: String,
    /// Extractor confidence in `[0, 1]`.
    pub extraction_confidence: Option<f32>,
}

#[derive(sqlx::FromRow)]
struct SummaryRow {
    record_id: String,
    filing_id: String,
    politician_id: String,
    politician_name: String,
    record_type: String,
    asset_class: String,
    side: Option<String>,
    asset_description_raw: String,
    event_date: Option<NaiveDate>,
    value_low: Option<Decimal>,
    value_high: Option<Decimal>,
    currency: Option<String>,
    verification_state: String,
    extraction_confidence: Option<f32>,
}

/// Loads one record's alert summary.
///
/// # Errors
/// Unknown record id, database failure, or stored values outside the core
/// vocabularies (data corruption).
pub async fn load_summary(pool: &PgPool, record_id: &str) -> anyhow::Result<RecordSummary> {
    let row: SummaryRow = sqlx::query_as(
        "select r.id as record_id, r.filing_id, r.politician_id, \
                p.canonical_name as politician_name, r.record_type, r.asset_class, \
                r.side, r.asset_description_raw, r.event_date, r.value_low, \
                r.value_high, r.currency, r.verification_state, r.extraction_confidence \
         from disclosure_record r \
         join politician p on p.id = r.politician_id \
         where r.id = $1",
    )
    .bind(record_id)
    .fetch_one(pool)
    .await
    .with_context(|| format!("loading alert summary for record {record_id}"))?;
    let value = match row.value_low {
        None => None,
        Some(low) => {
            let token = row
                .currency
                .context("value_low without currency")?
                .trim()
                .to_owned();
            let currency: Currency = serde_json::from_value(serde_json::Value::String(token))
                .context("stored currency is outside the core vocabulary")?;
            Some(
                ValueInterval::new(low, row.value_high, currency)
                    .context("stored value interval is inverted")?,
            )
        }
    };
    Ok(RecordSummary {
        record_id: row.record_id,
        filing_id: row.filing_id,
        politician_id: row.politician_id,
        politician_name: row.politician_name,
        record_type: row.record_type,
        asset_class: row.asset_class,
        side: row.side,
        asset_description_raw: row.asset_description_raw,
        event_date: row.event_date,
        value,
        verification_state: row.verification_state,
        extraction_confidence: row.extraction_confidence,
    })
}

/// The provenance link an alert carries (design §6.3).
#[must_use]
pub fn provenance_url(base: &str, filing_id: &str) -> String {
    format!("{}/filings/{filing_id}", base.trim_end_matches('/'))
}

/// Instant webhook body: one record.
#[derive(Debug, Serialize)]
pub struct AlertPayload<'a> {
    /// Event kind (`disclosure_record.published`).
    pub kind: &'a str,
    /// The matching rule — receivers dedup on (rule, event).
    pub alert_rule_id: &'a str,
    /// The outbox event.
    pub outbox_event_id: &'a str,
    /// The record summary (carries `verification_state` + confidence).
    pub record: &'a RecordSummary,
    /// Link back to the source filing.
    pub provenance_url: String,
}

/// One digest entry.
#[derive(Debug, Serialize)]
pub struct DigestEntry<'a> {
    /// The outbox event this entry settles.
    pub outbox_event_id: &'a str,
    /// The record summary.
    pub record: &'a RecordSummary,
    /// Link back to the source filing.
    pub provenance_url: String,
}

/// Digest webhook body: every record accumulated since the last digest pass.
#[derive(Debug, Serialize)]
pub struct DigestPayload<'a> {
    /// Payload kind (`digest`).
    pub kind: &'a str,
    /// The rule this digest belongs to.
    pub alert_rule_id: &'a str,
    /// The accumulated records.
    pub records: Vec<DigestEntry<'a>>,
}

/// The one text block both email bodies are built from — §6.3 honesty rule
/// enforced in a single place.
fn record_block(summary: &RecordSummary, provenance: &str) -> String {
    let side = summary
        .side
        .as_deref()
        .map(|s| format!(", {s}"))
        .unwrap_or_default();
    let date = summary
        .event_date
        .map_or_else(|| "unknown".to_owned(), |d| d.to_string());
    let value = summary.value.map_or_else(
        || "undisclosed".to_owned(),
        |v| {
            let currency = format!("{:?}", v.currency());
            match v.high() {
                Some(high) if high == v.low() => format!("{} {currency}", v.low()),
                Some(high) => format!("{}-{high} {currency}", v.low()),
                None => format!("over {} {currency}", v.low()),
            }
        },
    );
    let confidence = summary
        .extraction_confidence
        .map_or_else(|| "n/a".to_owned(), |c| c.to_string());
    format!(
        "{}: {}{side} ({})\n\
         asset: {}\n\
         date: {date}\n\
         value: {value}\n\
         verification: {} (extraction confidence {confidence})\n\
         provenance: {provenance}",
        summary.politician_name,
        summary.record_type,
        summary.asset_class,
        summary.asset_description_raw,
        summary.verification_state,
    )
}

/// Formats the instant alert email.
#[must_use]
pub fn instant_email(to: &str, summary: &RecordSummary, provenance: &str) -> EmailMessage {
    EmailMessage {
        to: to.to_owned(),
        subject: format!(
            "govfolio alert: {} — {}",
            summary.record_type, summary.politician_name
        ),
        text: record_block(summary, provenance),
    }
}

/// Formats the digest email: one block per record, one message per rule.
#[must_use]
pub fn digest_email(to: &str, entries: &[(RecordSummary, String)]) -> EmailMessage {
    let blocks: Vec<String> = entries
        .iter()
        .map(|(summary, provenance)| record_block(summary, provenance))
        .collect();
    EmailMessage {
        to: to.to_owned(),
        subject: format!("govfolio digest: {} new record(s)", entries.len()),
        text: blocks.join("\n\n"),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rust_decimal::Decimal;

    use super::*;

    fn summary() -> RecordSummary {
        RecordSummary {
            record_id: "0RECORD0000000000000000001".to_owned(),
            filing_id: "0FILING0000000000000000001".to_owned(),
            politician_id: "0HSEMBR0000000000000000001".to_owned(),
            politician_name: "Hon. Ada Lovelace".to_owned(),
            record_type: "transaction".to_owned(),
            asset_class: "equity".to_owned(),
            side: Some("buy".to_owned()),
            asset_description_raw: "Analytical Engines Inc (AE)".to_owned(),
            event_date: chrono::NaiveDate::from_ymd_opt(2026, 6, 1),
            value: Some(
                ValueInterval::new(
                    Decimal::new(100_100, 2),
                    Some(Decimal::new(1_500_000, 2)),
                    Currency::USD,
                )
                .unwrap(),
            ),
            verification_state: "unverified".to_owned(),
            extraction_confidence: Some(0.97),
        }
    }

    #[test]
    fn alerts_email_carries_the_honesty_fields() {
        let msg = instant_email(
            "a@example.org",
            &summary(),
            "https://govfolio.io/filings/0FILING0000000000000000001",
        );
        for needle in [
            "unverified",
            "0.97",
            "https://govfolio.io/filings/0FILING0000000000000000001",
            "Analytical Engines Inc (AE)",
            "1001.00-15000.00 USD",
        ] {
            assert!(
                msg.text.contains(needle),
                "missing {needle:?} in:\n{}",
                msg.text
            );
        }
    }

    #[test]
    fn alerts_payload_serializes_money_as_decimal_strings() {
        let s = summary();
        let payload = AlertPayload {
            kind: "disclosure_record.published",
            alert_rule_id: "r",
            outbox_event_id: "e",
            record: &s,
            provenance_url: provenance_url("https://govfolio.io/", &s.filing_id),
        };
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["record"]["value"]["low"], serde_json::json!("1001.00"));
        assert_eq!(
            json["provenance_url"],
            serde_json::json!("https://govfolio.io/filings/0FILING0000000000000000001"),
            "trailing base-url slash is normalized"
        );
        assert_eq!(
            json["record"]["verification_state"],
            serde_json::json!("unverified")
        );
    }
}
