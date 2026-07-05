//! Normative mapping tables from the regime doc (`docs/regimes/us_senate.md`):
//! §3.2 owner words, §3.3 transaction types, §3.4 amount bands, §3.5 asset
//! types. Unknown values are never guessed — callers fail closed or bucket to
//! `other` with a confidence penalty exactly as the doc prescribes.

use govfolio_core::domain::enums::{AssetClass, Owner, Side};

/// Amount-band grammar (front-matter `band_table`): verbatim string → decimal
/// bounds. Identical statutory bands to `us_house`; the spouse/DC
/// `Over $1,000,000***` variant's electronic string is UNOBSERVED and stays
/// outside the grammar until archived (regime doc §3.4). Open-ended band
/// stores the stated threshold as `low`, `high = None`. Any string outside
/// this table is a hard reject (invariant 6).
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

/// Owner-word map (regime doc §3.2): full words, always populated. `None` =
/// word outside the map — the caller hard-rejects the row (fail closed).
pub(crate) fn owner_for_word(word: &str) -> Option<Owner> {
    match word {
        "Self" => Some(Owner::Self_),
        "Spouse" => Some(Owner::Spouse),
        "Joint" => Some(Owner::Joint),
        "Child" => Some(Owner::Dependent),
        _ => None,
    }
}

/// Transaction-type map (regime doc §3.3): `Type` cell → (`side`,
/// `partial_sale`). `Exchange` is form-standard but electronically UNOBSERVED
/// (paper column, E10). `None` = token outside the grammar — hard reject.
pub(crate) fn side_for_type(raw: &str) -> Option<(Side, bool)> {
    match raw {
        "Purchase" => Some((Side::Buy, false)),
        "Sale (Full)" => Some((Side::Sell, false)),
        "Sale (Partial)" => Some((Side::Sell, true)),
        "Exchange" => Some((Side::Exchange, false)),
        _ => None,
    }
}

/// Asset Type words observed in evidence (regime doc §3.5) — the §6 scoring
/// vocabulary: values outside it cost −0.05 confidence and bucket to `other`.
pub(crate) const OBSERVED_ASSET_TYPES: &[&str] = &["Stock", "Corporate Bond"];

/// Asset Type → Gold `asset_class` (regime doc §3.5). eFD types ETFs as
/// `Stock` — they land in `equity` honestly (name-based fund detection would
/// be guessing, invariant 3 spirit). `None` = unknown vocabulary member:
/// caller buckets to `other` (extend THIS table first, then reparse).
pub(crate) fn asset_class_for_type(raw: &str) -> Option<AssetClass> {
    match raw {
        "Stock" => Some(AssetClass::Equity),
        "Corporate Bond" => Some(AssetClass::Bond),
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
            band_bounds("$1,001 - $15,000").unwrap(),
            ("1001.00", Some("15000.00"))
        );
        assert_eq!(
            band_bounds("Over $50,000,000").unwrap(),
            ("50000000.00", None)
        );
        assert!(band_bounds("$1,001 - $14,000").is_none(), "outside grammar");
        assert!(
            band_bounds("Over $1,000,000").is_none(),
            "spouse/DC variant stays outside the grammar until archived (§3.4)"
        );
    }

    #[test]
    fn owner_words_map_and_unknown_is_refused() {
        assert_eq!(owner_for_word("Self"), Some(Owner::Self_));
        assert_eq!(owner_for_word("Spouse"), Some(Owner::Spouse));
        assert_eq!(owner_for_word("Joint"), Some(Owner::Joint));
        assert_eq!(owner_for_word("Child"), Some(Owner::Dependent));
        assert_eq!(owner_for_word("Trust"), None, "never guess (invariant 3)");
    }

    #[test]
    fn transaction_types_map_per_regime_doc() {
        assert_eq!(side_for_type("Purchase"), Some((Side::Buy, false)));
        assert_eq!(side_for_type("Sale (Full)"), Some((Side::Sell, false)));
        assert_eq!(side_for_type("Sale (Partial)"), Some((Side::Sell, true)));
        assert_eq!(side_for_type("Exchange"), Some((Side::Exchange, false)));
        assert_eq!(side_for_type("Sale"), None, "bare Sale is outside grammar");
    }

    #[test]
    fn asset_types_bucket_per_regime_doc() {
        assert_eq!(asset_class_for_type("Stock"), Some(AssetClass::Equity));
        assert_eq!(
            asset_class_for_type("Corporate Bond"),
            Some(AssetClass::Bond)
        );
        assert_eq!(
            asset_class_for_type("Municipal Security"),
            None,
            "unknown vocabulary member — caller buckets to other"
        );
    }
}
