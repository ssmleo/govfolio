//! The `(canada_ciec, interest)` and `(canada_ciec, change_notification)`
//! details contracts (regime doc §5, invariant 5). Doc comments are contract
//! surface: schemars embeds them as `description`, so edits here must be
//! re-snapshotted deliberately (schema-contracts skill). Optional fields
//! serialize as explicit nulls so the full contract surface is visible on
//! every candidate (fixtures `MANIFEST.json` `details_nulls`).

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `details` payload of one CIEC declaration record that is NOT a Material
/// Change (regime doc §5): every in-scope type produces this shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CanadaCiecInterestDetailsV1 {
    /// Source declaration GUID (lowercase) — threaded from the fetch URL;
    /// families A/B never print it in-page (regime doc §4).
    pub declaration_id: String,
    /// 1-based row position in document order (always 1 for families A/A′/B;
    /// one per `ciec-declaration-disclosureitem` for family C).
    #[schemars(range(min = 1))]
    pub row_ordinal: u32,
    /// Family-C per-item GUID verbatim (UPPERCASE as printed); null for the
    /// flat families A/A′/B.
    pub item_id: Option<String>,
    /// Family-C section label verbatim (case + any `[…]` statute-ref suffix
    /// intact); null otherwise.
    pub section_label: Option<String>,
    /// `Declaration type` cell verbatim (`Gifts (Code)`, `Disclosure
    /// Summaries (Code)`, …) — the §3.1 census key.
    pub declaration_type_raw: String,
    /// Legal instrument, derived from the `Regime` cell: `act` for the
    /// Conflict of Interest Act, `code` for the Members' Code (regime doc
    /// §3.10 check 2 guarantees the binary).
    pub law: String,
    /// `<h1>` declaration-type title verbatim (regime doc §3.3 vocabulary).
    pub h1_title: String,
    /// Source person GUID from the client link — the §2.4 resolution audit
    /// trail (real politician id resolves from the roster in pool-backed runs).
    pub client_id: String,
    /// Card-header `· {title}` suffix, middot-stripped; null when absent.
    pub client_title: Option<String>,
    /// `No Longer Applicable` (`bg-warning`) badge present at fetch time
    /// (regime doc §3.8 — status flips mutate the same URL's bytes).
    pub no_longer_applicable: bool,
    /// `OCIEC Translation` (`bg-info`) badge present — the displayed text is
    /// office-translated (regime doc §3.9 provenance).
    pub ociec_translation: bool,
    /// Ingestion language: always `en` in v1 (regime doc §3.9).
    pub language: String,
    /// Family-A `Gift received date` verbatim (`YYYY-MM-DD`); null when absent
    /// (nullable, regime doc §3.4/§3.7). Never promoted to `transaction_date`.
    pub gift_received_date: Option<String>,
    /// Family-A `Source` cell verbatim (query-hot copy; also in `fields`).
    pub gift_source: Option<String>,
    /// Family-A `Circumstance` cell verbatim.
    pub gift_circumstance: Option<String>,
    /// Family-A′ `Destination` cell verbatim (Sponsored Travel).
    pub travel_destination: Option<String>,
    /// Family-A′ `Sponsor` cell verbatim.
    pub travel_sponsor: Option<String>,
    /// Family-A′ `Dates` cell verbatim (range + day count).
    pub travel_dates_raw: Option<String>,
    /// Travel range start parsed from `Dates` (`YYYY-MM-DD`); null when the
    /// range does not parse (fail-soft — raw survives in `travel_dates_raw`).
    pub travel_start: Option<String>,
    /// Travel range end parsed from `Dates`; null on parse failure.
    pub travel_end: Option<String>,
    /// Family-A/A′ payload `<dl>` pairs verbatim (`{dt: dd-text}`), an open
    /// vocabulary; `{}` for families B/C (regime doc §4).
    pub fields: BTreeMap<String, String>,
}

/// `details` payload of one Material Change record (regime doc §5): every
/// interest field PLUS the notice's change date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CanadaCiecChangeNotificationDetailsV1 {
    /// The shared interest surface (flattened to the same top-level keys).
    #[serde(flatten)]
    pub interest: CanadaCiecInterestDetailsV1,
    /// `Date of change: YYYY/MM/DD` from the item text, normalized to ISO
    /// `YYYY-MM-DD`; null when the line is missing/unparseable (fail-soft —
    /// the raw line survives in `asset_description_raw`, regime doc §3.7).
    pub date_of_change: Option<String>,
}

/// JSON Schema for the `(canada_ciec, interest)` details contract.
#[must_use]
pub fn interest_details_schema() -> schemars::Schema {
    schemars::schema_for!(CanadaCiecInterestDetailsV1)
}

/// JSON Schema for the `(canada_ciec, change_notification)` details contract.
#[must_use]
pub fn change_notification_details_schema() -> schemars::Schema {
    schemars::schema_for!(CanadaCiecChangeNotificationDetailsV1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sample_interest() -> CanadaCiecInterestDetailsV1 {
        CanadaCiecInterestDetailsV1 {
            declaration_id: "30c94327-3108-f111-81a2-001dd8b72449".to_owned(),
            row_ordinal: 1,
            item_id: None,
            section_label: None,
            declaration_type_raw: "Declarable Assets".to_owned(),
            law: "act".to_owned(),
            h1_title: "Public Declaration of Assets".to_owned(),
            client_id: "5b99c2bd-7b2a-f011-8195-001dd8b72449".to_owned(),
            client_title: Some("Minister".to_owned()),
            no_longer_applicable: false,
            ociec_translation: false,
            language: "en".to_owned(),
            gift_received_date: None,
            gift_source: None,
            gift_circumstance: None,
            travel_destination: None,
            travel_sponsor: None,
            travel_dates_raw: None,
            travel_start: None,
            travel_end: None,
            fields: BTreeMap::new(),
        }
    }

    #[test]
    fn optional_fields_serialize_as_explicit_nulls() {
        let value = serde_json::to_value(sample_interest()).unwrap();
        assert_eq!(
            value["item_id"],
            json!(null),
            "explicit null, never omitted"
        );
        assert_eq!(value["gift_received_date"], json!(null));
        assert_eq!(value["travel_start"], json!(null));
        assert_eq!(value["fields"], json!({}), "empty object, not null");
        assert_eq!(value["language"], json!("en"));
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 21, "every contract field present: {value}");
    }

    #[test]
    fn change_notification_flattens_interest_and_adds_date_of_change() {
        let cn = CanadaCiecChangeNotificationDetailsV1 {
            interest: sample_interest(),
            date_of_change: Some("2025-07-22".to_owned()),
        };
        let value = serde_json::to_value(&cn).unwrap();
        // Interest fields sit at the top level (flatten), plus date_of_change.
        assert_eq!(
            value["declaration_id"],
            json!("30c94327-3108-f111-81a2-001dd8b72449")
        );
        assert_eq!(value["date_of_change"], json!("2025-07-22"));
        assert_eq!(value.as_object().unwrap().len(), 22);
    }

    #[test]
    fn interest_schema_requires_the_mandatory_fields() {
        let schema = serde_json::to_value(interest_details_schema()).unwrap();
        let required = schema["required"].as_array().unwrap();
        for field in [
            "declaration_id",
            "row_ordinal",
            "declaration_type_raw",
            "law",
            "h1_title",
            "client_id",
            "no_longer_applicable",
            "ociec_translation",
            "language",
            "fields",
        ] {
            assert!(
                required.contains(&json!(field)),
                "{field} must be required: {schema}"
            );
        }
        for field in [
            "item_id",
            "section_label",
            "client_title",
            "gift_received_date",
        ] {
            assert!(
                !required.contains(&json!(field)),
                "{field} must stay optional: {schema}"
            );
        }
    }
}
