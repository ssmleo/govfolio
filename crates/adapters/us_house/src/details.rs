//! The `(us_house, transaction)` details contract (regime doc §5, invariant 5).
//! Doc comments are contract surface: schemars embeds them as `description`,
//! so edits here must be re-snapshotted deliberately (schema-contracts skill).

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Provenance of the Gold `owner` mapping (regime doc §3.3 auditability).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OwnerSource {
    /// Owner code printed in the row's own Owner column.
    Row,
    /// Inherited from the matching Investment Vehicle bullet's `(Owner: XX)`.
    Vehicle,
    /// Blank owner column and no vehicle owner: filer's own asset (flagged
    /// assumption, regime doc §3.3 — raw stays null in Silver).
    DefaultSelf,
}

/// `details` payload of one US House PTR transaction record (regime doc §5).
/// Optional fields serialize as explicit nulls so the contract surface is
/// fully visible in every candidate (fixtures `MANIFEST.json` convention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UsHousePtrTransactionDetailsV1 {
    /// Clerk `DocID` of the filing (opaque string; also on the PDF as `Filing ID #`).
    pub doc_id: String,
    /// 1-based row position across the whole document.
    #[schemars(range(min = 1))]
    pub row_ordinal: u32,
    /// eFD transaction id — printed only on amended rows (amendment linkage key).
    pub row_id: Option<String>,
    /// Trailing `[XX]` asset-type code from the asset cell (legend E2).
    pub asset_type_code: Option<String>,
    /// Amount band string verbatim as filed.
    pub amount_band_raw: String,
    /// Transaction type token verbatim (`P`/`S`/`S (partial)`/`E`).
    pub transaction_type_raw: String,
    /// True only for the `S (partial)` token (regime doc §3.4).
    pub partial_sale: bool,
    /// Cap. Gains > $200 checkbox; null = indeterminate from the text layer.
    pub cap_gains_over_200: Option<bool>,
    /// Per-row filing status verbatim (`New`/`Amended`, regime doc §3.7).
    pub filing_status_raw: String,
    /// Provenance of the Gold `owner` mapping.
    pub owner_source: Option<OwnerSource>,
    /// `SUBHOLDING OF:` sub-line verbatim.
    pub subholding_of: Option<String>,
    /// `(Owner: XX)` code of the matching Investment Vehicle bullet.
    pub vehicle_owner_code: Option<String>,
    /// `LOCATION:` sub-line of the matching Investment Vehicle bullet.
    pub vehicle_location: Option<String>,
    /// `DESCRIPTION:` sub-line verbatim.
    pub description: Option<String>,
    /// `COMMENTS:` sub-line verbatim.
    pub comments: Option<String>,
    /// Date from the `Digitally Signed:` line (ISO).
    pub signed_date: NaiveDate,
}

/// JSON Schema for the `(us_house, transaction)` details contract.
#[must_use]
pub fn transaction_details_schema() -> schemars::Schema {
    schemars::schema_for!(UsHousePtrTransactionDetailsV1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn optional_fields_serialize_as_explicit_nulls() {
        let details = UsHousePtrTransactionDetailsV1 {
            doc_id: "20020055".to_owned(),
            row_ordinal: 1,
            row_id: None,
            asset_type_code: Some("HN".to_owned()),
            amount_band_raw: "$250,001 - $500,000".to_owned(),
            transaction_type_raw: "P".to_owned(),
            partial_sale: false,
            cap_gains_over_200: None,
            filing_status_raw: "New".to_owned(),
            owner_source: Some(OwnerSource::DefaultSelf),
            subholding_of: None,
            vehicle_owner_code: None,
            vehicle_location: None,
            description: None,
            comments: None,
            signed_date: NaiveDate::from_ymd_opt(2026, 6, 12).unwrap(),
        };
        let value = serde_json::to_value(&details).unwrap();
        assert_eq!(value["row_id"], json!(null), "explicit null, never omitted");
        assert_eq!(value["owner_source"], json!("default_self"));
        assert_eq!(value["signed_date"], json!("2026-06-12"));
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 16, "every contract field present: {value}");
    }

    #[test]
    fn schema_requires_the_mandatory_fields() {
        let schema = serde_json::to_value(transaction_details_schema()).unwrap();
        let required = schema["required"].as_array().unwrap();
        for field in [
            "doc_id",
            "row_ordinal",
            "amount_band_raw",
            "transaction_type_raw",
            "partial_sale",
            "filing_status_raw",
            "signed_date",
        ] {
            assert!(
                required.contains(&json!(field)),
                "{field} must be required: {schema}"
            );
        }
    }
}
