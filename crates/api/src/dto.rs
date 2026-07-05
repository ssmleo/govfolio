//! Wire shapes of `/v1` (design §6.1): resources mirror Gold ~1:1. Closed
//! vocabularies and money REUSE the core wire types — one serialization
//! contract, everywhere (invariant 7: decimal strings, never floats).

use anyhow::Context as _;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde::de::DeserializeOwned;
use utoipa::ToSchema;

use govfolio_core::domain::enums::{
    AssetClass, Currency, Owner, RecordType, Side, VerificationState,
};
use govfolio_core::domain::value::ValueInterval;

use crate::error::ApiError;

/// One canonical disclosure record (Gold `disclosure_record`, design §4.2).
/// `verification_state` is present on EVERY record — honesty travels with
/// the data.
#[derive(Debug, Serialize, ToSchema)]
pub struct DisclosureRecord {
    /// Record ULID (time-sortable; the pagination cursor).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Source filing this record came from.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub filing_id: String,
    /// The politician the record concerns.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub politician_id: String,
    /// Disclosure regime the record was filed under.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub regime_id: String,
    /// Resolved instrument — `null` below threshold, never guessed (invariant 3).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub instrument_id: Option<String>,
    /// Asset description exactly as filed, always kept (invariant 2).
    pub asset_description_raw: String,
    /// One of the four observation types.
    pub record_type: RecordType,
    /// Asset class vocabulary.
    pub asset_class: AssetClass,
    /// Transaction direction; present when `record_type == transaction`.
    pub side: Option<Side>,
    /// Date the transaction happened.
    pub transaction_date: Option<NaiveDate>,
    /// Snapshot date for holdings.
    pub as_of_date: Option<NaiveDate>,
    /// Date the interest/change was notified.
    pub notified_date: Option<NaiveDate>,
    /// Best event date: `coalesce(transaction, notified, as_of)` (generated).
    pub event_date: Option<NaiveDate>,
    /// Declared value band; bounds are decimal STRINGS (invariant 7).
    pub value: Option<ValueInterval>,
    /// Whose asset (self/spouse/...).
    pub owner: Option<Owner>,
    /// Two-stage publication state (design §7.1) — on every record.
    pub verification_state: VerificationState,
    /// Extractor confidence in `[0, 1]`.
    pub extraction_confidence: Option<f32>,
    /// Parser id / model+prompt version that produced this record.
    pub extracted_by: String,
    /// Idempotency fingerprint (audit trail).
    pub fingerprint: String,
    /// Superseded record in a correction chain (supersede, never update).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub supersedes_record_id: Option<String>,
    /// Contract-typed regime payload (validated per (regime, `record_type`)
    /// JSON Schema at promotion, invariant 5).
    #[schema(value_type = Object)]
    pub details: serde_json::Value,
    /// When the record row was created.
    pub created_at: DateTime<Utc>,
}

/// One page of records; `next_cursor` is the last item's ULID, `null` when
/// the listing is exhausted (design §6.1: cursor pagination on ULIDs).
#[derive(Debug, Serialize, ToSchema)]
pub struct RecordPage {
    /// Records in ascending ULID (= insertion-time) order.
    pub items: Vec<DisclosureRecord>,
    /// Pass back as `cursor` to fetch the page after this one; `null` at the end.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub next_cursor: Option<String>,
}

/// Raw `disclosure_record` row as selected; conversion into the wire shape
/// funnels every closed vocabulary back through the core types.
#[derive(Debug, sqlx::FromRow)]
pub struct RecordRow {
    pub id: String,
    pub filing_id: String,
    pub politician_id: String,
    pub regime_id: String,
    pub instrument_id: Option<String>,
    pub asset_description_raw: String,
    pub record_type: String,
    pub asset_class: String,
    pub side: Option<String>,
    pub transaction_date: Option<NaiveDate>,
    pub as_of_date: Option<NaiveDate>,
    pub notified_date: Option<NaiveDate>,
    pub event_date: Option<NaiveDate>,
    pub value_low: Option<Decimal>,
    pub value_high: Option<Decimal>,
    pub currency: Option<String>,
    pub owner: Option<String>,
    pub verification_state: String,
    pub extraction_confidence: Option<f32>,
    pub extracted_by: String,
    pub fingerprint: String,
    pub supersedes_record_id: Option<String>,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

/// Parses a SQL CHECK wire token (e.g. `"buy"`) back into its core enum —
/// the same one rule the writer enforced (one rule, two enforcers).
fn from_token<T: DeserializeOwned>(token: String, what: &str) -> Result<T, ApiError> {
    serde_json::from_value(serde_json::Value::String(token))
        .with_context(|| format!("stored {what} is outside the core vocabulary"))
        .map_err(ApiError::from)
}

impl TryFrom<RecordRow> for DisclosureRecord {
    type Error = ApiError;

    fn try_from(row: RecordRow) -> Result<Self, Self::Error> {
        let value = match row.value_low {
            None => None,
            Some(low) => {
                let currency: Currency = from_token(
                    row.currency
                        .ok_or_else(|| {
                            ApiError::from(anyhow::anyhow!("value_low without currency"))
                        })?
                        .trim()
                        .to_owned(),
                    "currency",
                )?;
                Some(
                    ValueInterval::new(low, row.value_high, currency)
                        .context("stored value interval is inverted")?,
                )
            }
        };
        Ok(Self {
            id: row.id,
            filing_id: row.filing_id,
            politician_id: row.politician_id,
            regime_id: row.regime_id,
            instrument_id: row.instrument_id,
            asset_description_raw: row.asset_description_raw,
            record_type: from_token(row.record_type, "record_type")?,
            asset_class: from_token(row.asset_class, "asset_class")?,
            side: row.side.map(|s| from_token(s, "side")).transpose()?,
            transaction_date: row.transaction_date,
            as_of_date: row.as_of_date,
            notified_date: row.notified_date,
            event_date: row.event_date,
            value,
            owner: row.owner.map(|o| from_token(o, "owner")).transpose()?,
            verification_state: from_token(row.verification_state, "verification_state")?,
            extraction_confidence: row.extraction_confidence,
            extracted_by: row.extracted_by,
            fingerprint: row.fingerprint,
            supersedes_record_id: row.supersedes_record_id,
            details: row.details,
            created_at: row.created_at,
        })
    }
}

/// Page size bounds (design §6.1 keeps pages small and cacheable).
pub const DEFAULT_LIMIT: u32 = 50;
/// Hard ceiling on `limit`.
pub const MAX_LIMIT: u32 = 200;

/// Validates `cursor` (must be a ULID — it is a record id) and `limit`
/// (1..=[`MAX_LIMIT`], default [`DEFAULT_LIMIT`]).
///
/// # Errors
/// [`ApiError::BadRequest`] with code `invalid_cursor` or `invalid_limit`.
pub fn validate_page_params(
    cursor: Option<&str>,
    limit: Option<u32>,
) -> Result<(Option<String>, i64), ApiError> {
    let cursor = cursor
        .map(|c| {
            ulid::Ulid::from_string(c)
                .map(|_| c.to_owned())
                .map_err(|e| ApiError::bad_request("invalid_cursor", format!("cursor: {e}")))
        })
        .transpose()?;
    let limit = limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(ApiError::bad_request(
            "invalid_limit",
            format!("limit must be within 1..={MAX_LIMIT}, got {limit}"),
        ));
    }
    Ok((cursor, i64::from(limit)))
}

/// Builds one page from `limit + 1` fetched rows: the sentinel row (if any)
/// only proves more data exists; `next_cursor` = last RETURNED id.
///
/// # Errors
/// A stored row outside the core vocabularies (data corruption — internal).
pub fn build_page(mut rows: Vec<RecordRow>, limit: i64) -> Result<RecordPage, ApiError> {
    let limit = usize::try_from(limit).context("limit fits usize")?;
    let has_more = rows.len() > limit;
    rows.truncate(limit);
    let items = rows
        .into_iter()
        .map(DisclosureRecord::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = if has_more {
        items.last().map(|record| record.id.clone())
    } else {
        None
    };
    Ok(RecordPage { items, next_cursor })
}
