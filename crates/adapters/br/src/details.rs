//! The `(br, holding)` details contract (`docs/regimes/br/AUTHORITY.md` field
//! mapping + Quirks log, invariant 5). Doc comments are contract surface:
//! schemars embeds them as `description`, so edits here must be
//! re-snapshotted deliberately (schema-contracts skill).
//!
//! First `holding`-record-type details contract in this project (every prior
//! regime is `transaction`/`interest`/`change_notification`) — there is no rolling
//! feed and no band table here: one instance per declared asset line item
//! (`bem_candidato` row), filed once per candidacy at TSE candidacy-
//! registration time (AUTHORITY.md `cadence_and_lag`). See
//! `crates/adapters/br/plan.md` for the full field-mapping table, parse
//! strategy, and open items — most importantly the `CD_TIPO_BEM_CANDIDATO ->
//! AssetClass` table and the `DT_ULT_ATUAL_BEM_CANDIDATO` amendment-timestamp
//! ambiguity, neither of which this file resolves on its own.

use govfolio_core::domain::enums::AssetClass;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `details` payload of one Brazilian TSE `declaração de bens` asset-line-item
/// (regime `br`, `record_type = holding`). One instance per `bem_candidato`
/// row (plan.md "Parse strategy"). Every field is required: the 3 sampled
/// fixtures (typical/amendment/zero-asset cases) show no partial rows for any
/// of these columns — a source row missing one of them should fail closed
/// (invariant 6) rather than silently relaxing this contract to `Option`
/// (plan.md open item on TSE sentinel values, e.g. `#NULO`/`-1`/`-4`,
/// observed in adjacent `consulta_cand` columns but not yet observed in
/// `bem_candidato` itself).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BrHoldingDetailsV1 {
    /// Verbatim `CD_TIPO_BEM_CANDIDATO` source code (e.g. `"12"`, `"97"`).
    /// TSE publishes no complete code table (AUTHORITY.md field-mapping
    /// table) — see plan.md's code -> `AssetClass` table and its flagged
    /// low-confidence entries.
    pub asset_type_code_raw: String,
    /// Verbatim `DS_TIPO_BEM_CANDIDATO` human-readable label, always paired
    /// with the code above in the source row (self-describing pair,
    /// AUTHORITY.md field-mapping table).
    pub asset_type_label_raw: String,
    /// Derived asset-class bucket (plan.md's code -> `AssetClass` table).
    /// Deliberately duplicates the Gold `asset_class` column so the details
    /// payload is self-contained for audit without a join back to
    /// `disclosure_record` — a departure from every prior regime's details
    /// contract (none echoes `asset_class`); flagged in plan.md for auditor
    /// confirmation rather than assumed silently.
    pub asset_class: AssetClass,
    /// Verbatim `DS_BEM_CANDIDATO` free-text description. Duplicates the
    /// Gold `asset_description_raw` column for the same self-containment
    /// reason as `asset_class` above (invariant 2: raw is sacred).
    pub asset_description_raw: String,
    /// Verbatim `VR_BEM_CANDIDATO` comma-decimal string exactly as filed
    /// (e.g. `"15000,00"`), before comma->dot / `rust_decimal` parsing into
    /// the Gold `value` interval (invariant 7). Plan.md blocker: Gold
    /// `value.currency` needs `Currency::BRL`, which does not yet exist in
    /// `govfolio_core::domain::enums::Currency` (`EUR`/`GBP`/`USD` only).
    pub value_raw: String,
    /// `ANO_ELEICAO` — the quadrennial federal-election cycle year this
    /// declaration was filed for (e.g. `2022`).
    pub election_year: u16,
    /// `NR_ORDEM_BEM_CANDIDATO` — 1-based ordinal position of this asset
    /// within the candidate's own declaration (stable ordering/fingerprint
    /// key; not a durable cross-cycle id — `SQ_CANDIDATO` itself is minted
    /// fresh per election cycle, AUTHORITY.md `identifiers_available`).
    #[schemars(range(min = 1))]
    pub line_item_ordinal: u32,
    /// Verbatim `DT_ULT_ATUAL_BEM_CANDIDATO` (`DD/MM/YYYY`). See plan.md's
    /// amendment-timestamp-ambiguity edge case before wiring this into any
    /// supersession trigger (invariant 1) — the sampler found evidence this
    /// field may reflect a bulk backend re-timestamp event rather than a
    /// genuine per-candidate rectification.
    pub last_updated_date_raw: String,
    /// Verbatim `HH_ULT_ATUAL_BEM_CANDIDATO` (`HH:MM:SS`), paired with the
    /// date above.
    pub last_updated_time_raw: String,
}

/// JSON Schema for the `(br, holding)` details contract.
#[must_use]
pub fn holding_details_schema() -> schemars::Schema {
    schemars::schema_for!(BrHoldingDetailsV1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Case `typical_house_vehicle_land`, the "Casa" item
    /// (`crates/adapters/br/fixtures/typical_house_vehicle_land/input.json`).
    fn sample() -> BrHoldingDetailsV1 {
        BrHoldingDetailsV1 {
            asset_type_code_raw: "12".to_owned(),
            asset_type_label_raw: "Casa".to_owned(),
            asset_class: AssetClass::RealEstate,
            asset_description_raw: "Casa na zona rural de xapuri".to_owned(),
            value_raw: "10000,00".to_owned(),
            election_year: 2022,
            line_item_ordinal: 1,
            last_updated_date_raw: "02/10/2022".to_owned(),
            last_updated_time_raw: "23:21:28".to_owned(),
        }
    }

    #[test]
    fn all_fields_serialize_and_round_trip() {
        let details = sample();
        let value = serde_json::to_value(&details).unwrap();
        assert_eq!(value["asset_type_code_raw"], json!("12"));
        assert_eq!(value["asset_class"], json!("real_estate"), "wire rename");
        assert_eq!(value["election_year"], json!(2022));
        assert_eq!(value["line_item_ordinal"], json!(1));
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 9, "every contract field present: {value}");
        let back: BrHoldingDetailsV1 = serde_json::from_value(value).unwrap();
        assert_eq!(back, details);
    }

    #[test]
    fn schema_requires_every_field() {
        let schema = serde_json::to_value(holding_details_schema()).unwrap();
        let required = schema["required"].as_array().unwrap();
        for field in [
            "asset_type_code_raw",
            "asset_type_label_raw",
            "asset_class",
            "asset_description_raw",
            "value_raw",
            "election_year",
            "line_item_ordinal",
            "last_updated_date_raw",
            "last_updated_time_raw",
        ] {
            assert!(
                required.contains(&json!(field)),
                "{field} must be required: {schema}"
            );
        }
    }
}
