//! Per-regime fingerprint-content selector (design invariant 4 / plan Task 6).
//!
//! `publish.rs` (and `promote.rs`'s correction path) fingerprint `(filing_id,
//! ordinal, canonical content)` to keep Gold writes idempotent. By default,
//! "content" is the WHOLE bound [`GoldCandidate`], `details` included. A
//! regime's `details` payload can carry raw source fields kept for forensic
//! visibility (invariant 2) that must NOT feed the hash, because the source
//! mutates them out-of-band from the record's actual substance.
//!
//! `br` is the first such case (`docs/regimes/br/AUTHORITY.md` Quirks log,
//! 2026-07-06 finding): TSE's `DT_ULT_ATUAL_BEM_CANDIDATO`/
//! `HH_ULT_ATUAL_BEM_CANDIDATO` ("last updated") timestamps were independently
//! confirmed, three times this session, to reflect a bulk backend
//! re-timestamp event — 85-99%+ of a whole state's candidate population
//! sharing one timestamp, zero per-candidate mixing — rather than genuine
//! per-item rectification. Hashing them would spuriously re-fingerprint
//! nearly a whole state's holdings as "new" Gold rows on every routine TSE
//! backend touch (duplicated/inflated public data, not just wasted work).
//!
//! Follows the same `regime_code: &str` dispatch idiom as
//! [`crate::redaction::redact`] and [`crate::conformance::check_details`].
//! The candidate's ACTUAL stored `details` (DB row, API response) is never
//! touched here — only the throwaway `serde_json::Value` copy handed to
//! `fingerprint()` is adjusted.

use anyhow::Context as _;

use govfolio_core::domain::gold::GoldCandidate;

/// `(regime_code, excluded details keys)` — keys removed from the fingerprint
/// content only, kept verbatim in the stored `details`. Empty for every
/// regime not listed here (the default arm below).
const EXCLUDED_DETAIL_KEYS: &[(&str, &[&str])] =
    &[("br", &["last_updated_date_raw", "last_updated_time_raw"])];

/// Builds the JSON value fingerprinted for `candidate` under `regime_code`.
/// Default (every regime not listed in [`EXCLUDED_DETAIL_KEYS`]): identical
/// to the previous bare `serde_json::to_value(candidate)` call site — no
/// behavior change. A listed regime's excluded `details` keys are stripped
/// from the serialized COPY used only for hashing.
///
/// # Errors
/// Candidate serialization failure (should not happen for a valid
/// `GoldCandidate` — `details` is already a `serde_json::Value`).
pub fn fingerprint_content(
    regime_code: &str,
    candidate: &GoldCandidate,
) -> anyhow::Result<serde_json::Value> {
    let mut content =
        serde_json::to_value(candidate).context("serializing candidate for fingerprint content")?;
    if let Some((_, keys)) = EXCLUDED_DETAIL_KEYS
        .iter()
        .find(|(code, _)| *code == regime_code)
        && let Some(serde_json::Value::Object(details)) = content.get_mut("details")
    {
        for key in *keys {
            details.remove(*key);
        }
    }
    Ok(content)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use govfolio_core::domain::enums::{AssetClass, RecordType};
    use govfolio_core::domain::fingerprint::fingerprint;

    use super::*;

    /// A `br` holding candidate with a realistic `details` payload (shape
    /// matches the committed `crates/pipeline/schemas/details/br.holding.json`
    /// snapshot), parameterized on the two disputed timestamp fields.
    fn br_holding(last_date: &str, last_time: &str) -> GoldCandidate {
        GoldCandidate {
            filing_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse().unwrap(),
            politician_id: "01BX5ZZKBKACTAV9WEVGEMMVRZ".parse().unwrap(),
            regime_id: "01BX5ZZKBKACTAV9WEVGEMMVS0".parse().unwrap(),
            instrument_id: None,
            asset_description_raw: "Casa residencial".to_owned(),
            record_type: RecordType::Holding,
            asset_class: AssetClass::RealEstate,
            side: None,
            transaction_date: None,
            as_of_date: Some(NaiveDate::from_ymd_opt(2022, 10, 2).unwrap()),
            notified_date: None,
            value: None,
            owner: None,
            extraction_confidence: Some(1.0),
            extracted_by: "br_bem_candidato/csv@0".to_owned(),
            fingerprint: None,
            details: json!({
                "asset_type_code_raw": "12",
                "asset_type_label_raw": "Casa",
                "asset_class": "real_estate",
                "asset_description_raw": "Casa residencial",
                "value_raw": "150000,00",
                "election_year": 2022,
                "line_item_ordinal": 1,
                "last_updated_date_raw": last_date,
                "last_updated_time_raw": last_time,
            }),
        }
    }

    /// The real `us_house` `typical_single_row` fixture's committed
    /// `expected.gold.json` row (`crates/adapters/us_house/fixtures/
    /// typical_single_row/expected.gold.json`), reproduced as JSON (confidence
    /// simplified to `0.98` to dodge an f32-roundtrip clippy lint — the exact
    /// float value is irrelevant to what this test proves) so this test
    /// exercises the same shape the conformance harness fingerprints against
    /// in a real publish.
    fn us_house_typical_single_row() -> GoldCandidate {
        serde_json::from_value(json!({
            "filing_id": "0HSEFNG0000000000020020055",
            "politician_id": "0HSEMBR0000000000000000001",
            "regime_id": "0HSEREG0000000000000000001",
            "instrument_id": null,
            "asset_description_raw": "Listen Ventures IV, LP [HN]",
            "record_type": "transaction",
            "asset_class": "fund",
            "side": "buy",
            "transaction_date": "2026-05-13",
            "as_of_date": null,
            "notified_date": "2026-05-13",
            "value": { "low": "250001.00", "high": "500000.00", "currency": "USD" },
            "owner": "self",
            "extraction_confidence": 0.98,
            "extracted_by": "us_house_ptr/text@1",
            "fingerprint": null,
            "details": {
                "doc_id": "20020055",
                "row_ordinal": 1,
                "row_id": null,
                "asset_type_code": "HN",
                "amount_band_raw": "$250,001 - $500,000",
                "transaction_type_raw": "P",
                "partial_sale": false,
                "cap_gains_over_200": null,
                "filing_status_raw": "New",
                "owner_source": "default_self",
                "subholding_of": null,
                "vehicle_owner_code": null,
                "vehicle_location": null,
                "description": null,
                "comments": null,
                "signed_date": "2026-06-12"
            }
        }))
        .unwrap()
    }

    #[test]
    fn br_excludes_only_the_two_retimestamp_fields_from_the_hash() {
        // Two candidates differing ONLY in the disputed timestamp fields
        // (everything else, including every other `details` key, identical)
        // — a bulk TSE backend re-timestamp event, per AUTHORITY.md. The
        // whole point of this fix: they must fingerprint the SAME.
        let original = br_holding("13/05/2022", "09:00:00");
        let retouched = br_holding("13/05/2026", "23:59:59");
        assert_ne!(
            original.details, retouched.details,
            "sanity: the two candidates really do differ in details"
        );
        let original_content = fingerprint_content("br", &original).unwrap();
        let retouched_content = fingerprint_content("br", &retouched).unwrap();
        assert_eq!(
            original_content, retouched_content,
            "br fingerprint content must ignore last_updated_date_raw/last_updated_time_raw"
        );
        let original_fp = fingerprint("filing-1", 0, &original_content);
        let retouched_fp = fingerprint("filing-1", 0, &retouched_content);
        assert_eq!(
            original_fp, retouched_fp,
            "a bulk re-timestamp must not manufacture a new Gold fingerprint"
        );
    }

    #[test]
    fn br_content_omits_the_two_keys_but_stored_details_is_untouched() {
        let candidate = br_holding("13/05/2022", "09:00:00");
        let content = fingerprint_content("br", &candidate).unwrap();
        assert!(content["details"].get("last_updated_date_raw").is_none());
        assert!(content["details"].get("last_updated_time_raw").is_none());
        // every other details key still feeds the hash
        assert_eq!(content["details"]["value_raw"], json!("150000,00"));
        assert_eq!(content["details"]["line_item_ordinal"], json!(1));
        // the candidate handed in is untouched — caller's `details` still
        // carries both raw fields verbatim (satisfies the committed schema).
        assert_eq!(
            candidate.details["last_updated_date_raw"],
            json!("13/05/2022")
        );
        assert_eq!(
            candidate.details["last_updated_time_raw"],
            json!("09:00:00")
        );
    }

    #[test]
    fn us_house_fingerprint_content_is_byte_identical_to_the_old_bare_serialization() {
        // Zero blast radius (requirement 1): the default arm must produce
        // EXACTLY what the old call site did — `serde_json::to_value(&bound)`
        // — for every regime other than `br`. Same candidate in, same JSON
        // value out, so the resulting hash is provably unchanged.
        let candidate = us_house_typical_single_row();
        let old_behavior = serde_json::to_value(&candidate).unwrap();
        let new_behavior = fingerprint_content("us_house", &candidate).unwrap();
        assert_eq!(
            new_behavior, old_behavior,
            "us_house must still fingerprint the whole candidate, details included"
        );
        let old_fp = fingerprint("0HSEFNG0000000000020020055", 0, &old_behavior);
        let new_fp = fingerprint("0HSEFNG0000000000020020055", 0, &new_behavior);
        assert_eq!(old_fp, new_fp);
    }

    #[test]
    fn every_non_br_regime_falls_through_to_the_unchanged_default_arm() {
        // Broader zero-blast-radius sweep across every other launch regime's
        // adapter/regime code (crates/adapters/<x>): none of them is in
        // `EXCLUDED_DETAIL_KEYS`, so all must hash the whole candidate,
        // details included, exactly as before this change.
        let candidate = us_house_typical_single_row();
        let old_behavior = serde_json::to_value(&candidate).unwrap();
        for regime_code in [
            "us_house",
            "us_senate",
            "uk_commons_register",
            "canada_ciec",
            "australia_register",
            "eu_parliament_dpi",
            "fr_hatvp_dia",
            "de_bundestag",
        ] {
            assert_eq!(
                fingerprint_content(regime_code, &candidate).unwrap(),
                old_behavior,
                "{regime_code} must fall through to the unchanged default arm"
            );
        }
    }
}
