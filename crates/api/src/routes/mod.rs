//! `/v1` route handlers (design §6.1: resources mirror Gold ~1:1).

pub mod admin;
pub mod alert_rules;
pub mod filings;
pub mod jurisdictions;
pub mod keys;
pub mod ops;
pub mod politicians;
pub mod records;
pub mod regimes;
pub mod review;
pub mod search;
pub mod stripe;

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

/// Turns user text into a substring ILIKE pattern with `%`/`_`/`\` escaped —
/// metacharacters in a query are LITERALS, never wildcards.
pub(crate) fn like_pattern(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len() + 2);
    escaped.push('%');
    for c in query.chars() {
        if matches!(c, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped.push('%');
    escaped
}

#[cfg(test)]
mod tests {
    use super::like_pattern;

    #[test]
    fn like_pattern_escapes_metacharacters_and_wraps_in_wildcards() {
        assert_eq!(like_pattern("smucker"), "%smucker%");
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
        assert_eq!(like_pattern(r"back\slash"), "%back\\\\slash%");
    }
}
