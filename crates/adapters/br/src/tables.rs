//! `CD_TIPO_BEM_CANDIDATO -> AssetClass` lookup (plan.md's resolved 5-code
//! table, mirroring the `us_house`/`us_senate` `asset_class_for_code`
//! convention, `crates/adapters/us_house/src/tables.rs:56`). `None` = a code
//! never observed in the 3 committed fixtures: the caller buckets it to
//! `AssetClass::Other` and applies a confidence penalty (plan.md edge case
//! 3) — never guessed (invariant 3).

use govfolio_core::domain::enums::AssetClass;

/// Looks a `CD_TIPO_BEM_CANDIDATO` code up in the resolved table (plan.md
/// "`CD_TIPO_BEM_CANDIDATO` -> `AssetClass` table", both audit-resolved to
/// `High` confidence — `32` and `97` are pinned `Some(...)` entries, not a
/// confidence-penalized default).
pub(crate) fn asset_class_for_code(code: &str) -> Option<AssetClass> {
    match code {
        "12" | "13" => Some(AssetClass::RealEstate),
        "21" | "97" => Some(AssetClass::Other),
        "32" => Some(AssetClass::Private),
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn observed_codes_map_per_plan_md() {
        assert_eq!(asset_class_for_code("12"), Some(AssetClass::RealEstate));
        assert_eq!(asset_class_for_code("13"), Some(AssetClass::RealEstate));
        assert_eq!(asset_class_for_code("21"), Some(AssetClass::Other));
        assert_eq!(asset_class_for_code("32"), Some(AssetClass::Private));
        assert_eq!(asset_class_for_code("97"), Some(AssetClass::Other));
    }

    #[test]
    fn unobserved_code_is_refused_not_guessed() {
        assert_eq!(
            asset_class_for_code("61"),
            None,
            "never guess (invariant 3) — caller buckets to Other + penalty"
        );
    }
}
