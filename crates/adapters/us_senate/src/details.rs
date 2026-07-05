//! The `(us_senate, transaction)` details contract (regime doc §5, invariant 5).
//! Doc comments are contract surface: schemars embeds them as `description`,
//! so edits here must be re-snapshotted deliberately (schema-contracts skill).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `details` payload of one US Senate eFD PTR transaction record (regime doc §5).
/// Optional fields serialize as explicit nulls so the contract surface is
/// fully visible in every candidate (fixtures `MANIFEST.json` convention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UsSenatePtrTransactionDetailsV1 {
    /// eFD report UUID from the view URL — the page never prints it; the
    /// pipeline threads it (regime doc §4).
    pub report_uuid: String,
    /// 1-based row position in document order (top-to-bottom).
    #[schemars(range(min = 1))]
    pub row_ordinal: u32,
    /// Printed `#` cell verbatim (descends `N..1` in document order, regime doc §3.1).
    pub row_number: String,
    /// Ticker anchor text; null when the cell is the `--` sentinel. The
    /// instrument-resolution waterfall starts here (design §5.4).
    pub ticker: Option<String>,
    /// Asset Type cell verbatim (`Stock`, `Corporate Bond`, …) — kept so
    /// reclassification never needs a reparse (regime doc §3.5).
    pub asset_type_raw: String,
    /// Asset Name sub-line (`div.text-muted`), whitespace-collapsed
    /// (e.g. `Rate/Coupon: 5.5% Matures: 2036-04-15`); null when absent.
    pub asset_detail: Option<String>,
    /// Amount band string verbatim as filed.
    pub amount_band_raw: String,
    /// Type cell verbatim (`Purchase`/`Sale (Partial)`/`Sale (Full)`/`Exchange`).
    pub transaction_type_raw: String,
    /// True only for the `Sale (Partial)` token (regime doc §3.3).
    pub partial_sale: bool,
    /// Comment cell text; null when the cell is the `--` sentinel.
    pub comment: Option<String>,
    /// `N` from the title suffix `(Amendment N)`; null on originals. No
    /// supersession linkage exists in the document (regime doc §3.6).
    pub amendment_number: Option<u32>,
    /// Filed stamp minus the `Filed` label, collapsed
    /// (`06/12/2026 @ 1:23 PM`; timezone unresolved — raw survives).
    pub filed_at_raw: String,
}

/// JSON Schema for the `(us_senate, transaction)` details contract.
#[must_use]
pub fn transaction_details_schema() -> schemars::Schema {
    schemars::schema_for!(UsSenatePtrTransactionDetailsV1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn optional_fields_serialize_as_explicit_nulls() {
        let details = UsSenatePtrTransactionDetailsV1 {
            report_uuid: "4b69867f-0376-4526-93f2-cd556b1155c9".to_owned(),
            row_ordinal: 1,
            row_number: "1".to_owned(),
            ticker: None,
            asset_type_raw: "Corporate Bond".to_owned(),
            asset_detail: Some("Rate/Coupon: 5.5% Matures: 2036-04-15".to_owned()),
            amount_band_raw: "$1,001 - $15,000".to_owned(),
            transaction_type_raw: "Purchase".to_owned(),
            partial_sale: false,
            comment: None,
            amendment_number: None,
            filed_at_raw: "06/12/2026 @ 1:23 PM".to_owned(),
        };
        let value = serde_json::to_value(&details).unwrap();
        assert_eq!(value["ticker"], json!(null), "explicit null, never omitted");
        assert_eq!(value["amendment_number"], json!(null));
        assert_eq!(value["partial_sale"], json!(false));
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 12, "every contract field present: {value}");
    }

    #[test]
    fn schema_requires_the_mandatory_fields() {
        let schema = serde_json::to_value(transaction_details_schema()).unwrap();
        let required = schema["required"].as_array().unwrap();
        for field in [
            "report_uuid",
            "row_ordinal",
            "row_number",
            "asset_type_raw",
            "amount_band_raw",
            "transaction_type_raw",
            "partial_sale",
            "filed_at_raw",
        ] {
            assert!(
                required.contains(&json!(field)),
                "{field} must be required: {schema}"
            );
        }
        for field in ["ticker", "asset_detail", "comment", "amendment_number"] {
            assert!(
                !required.contains(&json!(field)),
                "{field} must stay optional: {schema}"
            );
        }
    }
}
