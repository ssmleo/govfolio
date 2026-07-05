//! `/v1` route handlers (design §6.1: resources mirror Gold ~1:1).

pub mod politicians;
pub mod records;

/// One shared `disclosure_record` projection behind every records endpoint —
/// a compile-time string literal so sqlx's `SqlSafeStr` guarantee holds
/// without escape hatches.
macro_rules! record_select {
    ($tail:literal) => {
        concat!(
            "select id, filing_id, politician_id, regime_id, instrument_id, \
             asset_description_raw, record_type, asset_class, side, transaction_date, \
             as_of_date, notified_date, event_date, value_low, value_high, currency, owner, \
             verification_state, extraction_confidence, extracted_by, fingerprint, \
             supersedes_record_id, details, created_at \
             from disclosure_record ",
            $tail
        )
    };
}
pub(crate) use record_select;
