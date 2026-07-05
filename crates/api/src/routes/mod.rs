//! `/v1` route handlers (design §6.1: resources mirror Gold ~1:1).

pub mod alert_rules;
pub mod politicians;
pub mod records;

/// One shared `disclosure_record` projection behind every records endpoint —
/// a `const` so every composed statement stays a compile-time `&'static str`
/// (via `const_format::concatcp!`) and sqlx's `SqlSafeStr` guarantee holds
/// without escape hatches.
pub(crate) const RECORD_COLUMNS: &str = "select id, filing_id, politician_id, regime_id, instrument_id, \
     asset_description_raw, record_type, asset_class, side, transaction_date, \
     as_of_date, notified_date, event_date, value_low, value_high, currency, owner, \
     verification_state, extraction_confidence, extracted_by, fingerprint, \
     supersedes_record_id, details, created_at \
     from disclosure_record ";
