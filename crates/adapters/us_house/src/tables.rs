//! Normative mapping tables from the regime doc (docs/regimes/us-house.md):
//! §3.3 owner codes, §3.5 amount bands, §3.6 asset-type buckets. Unknown
//! values are never guessed — callers fail closed or bucket to `other` with
//! a confidence penalty exactly as the doc prescribes.

use govfolio_core::domain::enums::{AssetClass, Owner};

/// Amount-band grammar (front-matter `band_table`): verbatim string → decimal
/// bounds. Open-ended band stores the stated threshold as `low`, `high = None`.
/// Any string outside this table is a hard reject (invariant 6).
pub(crate) const BANDS: &[(&str, &str, Option<&str>)] = &[
    ("$1,001 - $15,000", "1001.00", Some("15000.00")),
    ("$15,001 - $50,000", "15001.00", Some("50000.00")),
    ("$50,001 - $100,000", "50001.00", Some("100000.00")),
    ("$100,001 - $250,000", "100001.00", Some("250000.00")),
    ("$250,001 - $500,000", "250001.00", Some("500000.00")),
    ("$500,001 - $1,000,000", "500001.00", Some("1000000.00")),
    ("$1,000,001 - $5,000,000", "1000001.00", Some("5000000.00")),
    (
        "$5,000,001 - $25,000,000",
        "5000001.00",
        Some("25000000.00"),
    ),
    (
        "$25,000,001 - $50,000,000",
        "25000001.00",
        Some("50000000.00"),
    ),
    ("Over $50,000,000", "50000000.00", None),
];

/// Looks a band string up in the grammar.
pub(crate) fn band_bounds(raw: &str) -> Option<(&'static str, Option<&'static str>)> {
    BANDS
        .iter()
        .find(|(band, _, _)| *band == raw)
        .map(|(_, low, high)| (*low, *high))
}

/// Owner-code map (regime doc §3.3). `None` = code outside the map (caller
/// records `Owner::Unknown`, never guesses).
pub(crate) fn owner_for_code(code: &str) -> Option<Owner> {
    match code {
        "SP" => Some(Owner::Spouse),
        "DC" => Some(Owner::Dependent),
        "JT" => Some(Owner::Joint),
        _ => None,
    }
}

/// The row-level owner tokens the row grammar accepts (regime doc §3.2).
pub(crate) const ROW_OWNER_TOKENS: &[&str] = &["SP", "DC", "JT"];

/// Asset-type code → Gold `asset_class` buckets (regime doc §3.6, legend E2).
/// `None` = code not in the legend: bucket to `other` + confidence penalty.
pub(crate) fn asset_class_for_code(code: &str) -> Option<AssetClass> {
    match code {
        "ST" | "PS" | "RS" | "SA" => Some(AssetClass::Equity),
        "CS" | "GS" | "AB" | "ET" => Some(AssetClass::Bond),
        "EF" | "MF" | "HE" | "HN" | "RF" | "RE" | "RN" | "5C" | "5F" | "5P" | "4K" | "IR"
        | "IH" | "IC" | "MA" | "BK" => Some(AssetClass::Fund),
        "OP" => Some(AssetClass::Option),
        "CT" => Some(AssetClass::Crypto),
        "PM" | "FU" | "FE" | "CO" => Some(AssetClass::Commodity),
        "RP" | "FA" | "MO" | "DS" => Some(AssetClass::RealEstate),
        "OI" | "OL" => Some(AssetClass::Private),
        "OT" | "BA" | "TR" | "EQ" | "DB" | "PE" | "DO" | "IP" | "FN" | "VA" | "VI" | "WU" => {
            Some(AssetClass::Other)
        }
        _ => None,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn observed_bands_map_to_decimal_bounds() {
        assert_eq!(
            band_bounds("$250,001 - $500,000").unwrap(),
            ("250001.00", Some("500000.00"))
        );
        assert_eq!(
            band_bounds("Over $50,000,000").unwrap(),
            ("50000000.00", None)
        );
        assert!(band_bounds("$1,001 - $14,000").is_none(), "outside grammar");
        assert!(
            band_bounds("Spouse/DC over $1,000,000").is_none(),
            "unobserved variant stays outside the grammar until archived (§3.5)"
        );
    }

    #[test]
    fn owner_codes_map_and_unknown_is_refused() {
        assert_eq!(owner_for_code("SP"), Some(Owner::Spouse));
        assert_eq!(owner_for_code("DC"), Some(Owner::Dependent));
        assert_eq!(owner_for_code("JT"), Some(Owner::Joint));
        assert_eq!(owner_for_code("XX"), None, "never guess (invariant 3)");
    }

    #[test]
    fn asset_codes_bucket_per_regime_doc() {
        assert_eq!(asset_class_for_code("ST"), Some(AssetClass::Equity));
        assert_eq!(asset_class_for_code("HN"), Some(AssetClass::Fund));
        assert_eq!(asset_class_for_code("OP"), Some(AssetClass::Option));
        assert_eq!(asset_class_for_code("GS"), Some(AssetClass::Bond));
        assert_eq!(asset_class_for_code("RP"), Some(AssetClass::RealEstate));
        assert_eq!(asset_class_for_code("ZZ"), None, "outside the legend");
    }
}
