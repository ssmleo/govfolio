//! The `(uk_commons_register, interest)` details contract (regime doc §5,
//! invariant 5). Doc comments are contract surface: schemars embeds them as
//! `description`, so edits here must be re-snapshotted deliberately
//! (schema-contracts skill).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// §3.4 value-rule provenance: which rule produced the Gold `value`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValueSource {
    /// R1: top-level typed `Value` field (`Decimal` + `currencyCode`).
    ValueField,
    /// R2a: category 7 `ShareholdingThreshold` monetary threshold string —
    /// open-ended interval, low = stated threshold.
    ShareholdingThreshold,
    /// R3: category 4 visits — deterministic sum of the declared exact
    /// per-donor amounts (single donor = that exact value).
    SumOfDonors,
    /// R2b/R4: no monetary value in the record — Gold `value` is null,
    /// never inferred.
    None,
}

/// One `fields[]` entry, flattened per regime doc §5: `typeInfo.currencyCode`
/// is hoisted to `currency_code` (money ⇔ `currency_code` present — never the
/// field name or type alone, §3.2); `value` is deliberately schema-`any`
/// (the payload is source-shaped and category-specific); `values` carries the
/// nested rows of complex fields (`Donor[]`, `VisitLocation[]`) and is null
/// on flat fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DetailsField {
    /// Source field name verbatim (open vocabulary, §3.2).
    pub name: String,
    /// Source field description verbatim; null when the source omits it.
    pub description: Option<String>,
    /// Source type token verbatim (`String`, `Decimal`, `Boolean`, `Int`,
    /// `DateOnly`, `Donor[]`, …).
    #[serde(rename = "type")]
    pub field_type: String,
    /// `typeInfo.currencyCode` verbatim; null when `typeInfo` is null.
    pub currency_code: Option<String>,
    /// Field value verbatim in its source JSON type (§3.2 table).
    pub value: serde_json::Value,
    /// Nested rows (array of array-of-field) for complex fields; null when
    /// the source carries no `values` key.
    pub values: Option<Vec<Vec<DetailsField>>>,
}

/// `details` payload of one UK Commons Register interest record (regime doc
/// §5). Optional fields serialize as explicit nulls so the contract surface
/// is fully visible in every candidate (fixtures `MANIFEST.json` convention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UkCommonsRegisterInterestDetailsV1 {
    /// Source-native interest id, stable across in-place updates (§2.5).
    #[schemars(range(min = 1))]
    pub interest_id: u32,
    /// `updatedDates.length` at fetch time (0 = original) — the filing
    /// version key (§2.5).
    pub version: u32,
    /// Payment (child) → payer (parent) link, category 1.x; null otherwise.
    pub parent_interest_id: Option<u32>,
    /// API category id (§3.1 — NOT the printed category number).
    pub category_id: u32,
    /// `category.number` verbatim (a STRING, e.g. `"1.1"`).
    pub category_number: String,
    /// `category.name` verbatim.
    pub category_name: String,
    /// Numeric MNIS member id — the §2.4 resolution audit trail.
    pub member_id: u32,
    /// `registrationDate` verbatim (`YYYY-MM-DD`); null on migrated legacy
    /// rows (§3.6).
    pub registration_date: Option<String>,
    /// `publishedDate` verbatim (`YYYY-MM-DD`); a null is rejected at
    /// promotion (unobserved).
    pub published_date: String,
    /// `updatedDates` verbatim (may be empty).
    pub updated_dates: Vec<String>,
    /// Rectification flag (e.g. late registration); `true` unobserved.
    pub rectified: bool,
    /// Rectification reason verbatim; null unless rectified.
    pub rectified_details: Option<String>,
    /// Category 7 only: the `ShareholdingThreshold` value verbatim
    /// (query-hot copy; also inside `fields`).
    pub shareholding_threshold_raw: Option<String>,
    /// §3.4 provenance of the Gold `value`.
    pub value_source: ValueSource,
    /// The source `fields` array, flattened per [`DetailsField`].
    pub fields: Vec<DetailsField>,
}

/// JSON Schema for the `(uk_commons_register, interest)` details contract.
#[must_use]
pub fn interest_details_schema() -> schemars::Schema {
    schemars::schema_for!(UkCommonsRegisterInterestDetailsV1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn optional_fields_serialize_as_explicit_nulls() {
        let details = UkCommonsRegisterInterestDetailsV1 {
            interest_id: 15475,
            version: 0,
            parent_interest_id: None,
            category_id: 8,
            category_number: "7".to_owned(),
            category_name: "Shareholdings".to_owned(),
            member_id: 4051,
            registration_date: Some("2026-06-17".to_owned()),
            published_date: "2026-06-17".to_owned(),
            updated_dates: vec![],
            rectified: false,
            rectified_details: None,
            shareholding_threshold_raw: Some(
                "(ii) Other shareholdings, valued at more than £70,000".to_owned(),
            ),
            value_source: ValueSource::ShareholdingThreshold,
            fields: vec![DetailsField {
                name: "HeldOnBehalfOf".to_owned(),
                description: Some("Held jointly or on behalf of ...".to_owned()),
                field_type: "String".to_owned(),
                currency_code: None,
                value: serde_json::Value::Null,
                values: None,
            }],
        };
        let value = serde_json::to_value(&details).unwrap();
        assert_eq!(value["parent_interest_id"], json!(null), "explicit null");
        assert_eq!(value["rectified_details"], json!(null));
        assert_eq!(value["fields"][0]["currency_code"], json!(null));
        assert_eq!(value["fields"][0]["values"], json!(null));
        assert_eq!(value["fields"][0]["type"], json!("String"), "wire rename");
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 15, "every contract field present: {value}");
    }

    #[test]
    fn value_source_wire_tokens_are_snake_case() {
        assert_eq!(
            serde_json::to_value(ValueSource::ValueField).unwrap(),
            json!("value_field")
        );
        assert_eq!(
            serde_json::to_value(ValueSource::ShareholdingThreshold).unwrap(),
            json!("shareholding_threshold")
        );
        assert_eq!(
            serde_json::to_value(ValueSource::SumOfDonors).unwrap(),
            json!("sum_of_donors")
        );
        assert_eq!(
            serde_json::to_value(ValueSource::None).unwrap(),
            json!("none")
        );
    }

    #[test]
    fn schema_requires_the_mandatory_fields() {
        let schema = serde_json::to_value(interest_details_schema()).unwrap();
        let required = schema["required"].as_array().unwrap();
        for field in [
            "interest_id",
            "version",
            "category_id",
            "category_number",
            "category_name",
            "member_id",
            "published_date",
            "updated_dates",
            "rectified",
            "value_source",
            "fields",
        ] {
            assert!(
                required.contains(&json!(field)),
                "{field} must be required: {schema}"
            );
        }
        for field in [
            "parent_interest_id",
            "registration_date",
            "rectified_details",
            "shareholding_threshold_raw",
        ] {
            assert!(
                !required.contains(&json!(field)),
                "{field} must stay optional: {schema}"
            );
        }
    }
}
