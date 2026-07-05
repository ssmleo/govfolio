//! Normative mapping tables from the regime doc (`docs/regimes/canada_ciec.md`):
//! §3.1 declaration-type census + grammar families, §3.3 h1↔type integrity
//! vocabulary, §3.5 owner grammar, §3.10 check 2 law binary. Unknown values are
//! never guessed — the caller freezes (invariant 6) or rejects the row.

use govfolio_core::domain::enums::Owner;

/// Payload grammar family (regime doc §3.4). Sponsored Travel (A′) shares the
/// typed-`<dl>` parse of families A; the record-level difference lives in
/// `normalize` (Purpose→description, travel fields), keyed off the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// A / A′: a second `<dl>` of typed `dt`/`dd` pairs → `fields_raw`.
    TypedFields,
    /// B: a `Description` `<dd>` of flat text → `description_raw`.
    Flat,
    /// C: an itemized `Description` `<dd>` → one row per `disclosureitem`.
    Itemized,
}

/// `Declaration type` cell → grammar family, restricted to the v1 in-scope
/// census (regime doc §3.1/§3.2). `None` = an out-of-scope or unknown type —
/// the caller freezes (a new/unswept type is a rules change, invariant 6).
pub(crate) fn family_for_type(declaration_type_raw: &str) -> Option<Family> {
    match declaration_type_raw {
        "Gifts (Act)" | "Gifts (Code)" | "Forfeited Gifts" | "Sponsored Travel" => {
            Some(Family::TypedFields)
        }
        "Declarable Assets"
        | "Liabilities"
        | "Outside Activities"
        | "Summary Statements (Act)"
        | "Travel" => Some(Family::Flat),
        "Disclosure Summaries (Code)" | "Material Changes" => Some(Family::Itemized),
        _ => None,
    }
}

/// True for the one type that maps to `record_type = change_notification`
/// (regime doc §3.5); every other in-scope type is `interest`.
pub(crate) fn is_material_change(declaration_type_raw: &str) -> bool {
    declaration_type_raw == "Material Changes"
}

/// True for the family-A′ Sponsored Travel type: `asset_description_raw` comes
/// from the `Purpose` field (not `Nature`), owner is `self` (regime doc §3.5).
pub(crate) fn is_sponsored_travel(declaration_type_raw: &str) -> bool {
    declaration_type_raw == "Sponsored Travel"
}

/// Expected `<h1>` title for a declaration type (regime doc §3.3/§3.10 check 1).
/// `None` for the evidence-only types (Travel, Sponsored Travel, Forfeited
/// Gifts) whose exact h1 is unarchived — the caller then only requires a
/// non-empty h1 (documented live-path gap, never a false freeze).
pub(crate) fn expected_h1(declaration_type_raw: &str) -> Option<&'static str> {
    let title = match declaration_type_raw {
        "Declarable Assets" => "Public Declaration of Assets",
        "Liabilities" => "Public Declaration of Liabilities ($10,000 or more)",
        "Outside Activities" => "Public Declaration of Outside Activities",
        "Summary Statements (Act)" => "Summary Statement",
        "Disclosure Summaries (Code)" => "Disclosure Summary",
        "Material Changes" => "Notice of Material Change",
        "Gifts (Act)" => "Public Declaration of Gifts or Other Advantages",
        "Gifts (Code)" => "Public Statement of Gifts or Other Benefits",
        _ => return None,
    };
    Some(title)
}

/// `Regime` cell → `details.law` enum (regime doc §3.10 check 2 binary).
/// `None` = a value outside the two legal instruments — a hard reject.
pub(crate) fn law_code(law_raw: &str) -> Option<&'static str> {
    match law_raw {
        "Conflict of Interest Act" => Some("act"),
        "Conflict of Interest Code for Members of the House of Commons" => Some("code"),
        _ => None,
    }
}

/// Family-C `disclosurelabel` section labels that map to `owner = self`
/// (regime doc §3.5), lowercased and stripped of any trailing `[…]` statute
/// reference.
const KNOWN_SELF_LABELS: &[&str] = &[
    "assets",
    "liabilities",
    "activities",
    "other sources of income",
    "investment in private corporations",
    "affiliated corporations",
];

/// The `Spouse's/Common-Law Partner's …` section-label prefix (regime doc
/// §3.5), lowercased. Straight apostrophe `0x27` per fixture bytes.
const SPOUSE_PREFIX: &str = "spouse's/common-law partner's";

/// Strips a trailing `[…]` statute-reference suffix from a section label
/// (`INVESTMENT IN PRIVATE CORPORATIONS [Paragraph 24(1)(a)]` → the label),
/// regime doc §3.5.
fn label_base(label: &str) -> &str {
    let trimmed = label.trim();
    if trimmed.ends_with(']')
        && let Some(idx) = trimmed.rfind('[')
    {
        return trimmed[..idx].trim();
    }
    trimmed
}

/// Family-C owner grammar (regime doc §3.5). Returns `(owner, is_dependent)`
/// where `is_dependent` flags the −0.05 confidence deduction (§6.2 — the
/// Dependent-children owner mapping is unobserved). `None` = a section label
/// outside the archived grammar: the caller rejects the row → `review_task`,
/// never a low-confidence Gold row (invariant 6).
pub(crate) fn family_c_owner(section_label: &str) -> Option<(Owner, bool)> {
    let base = label_base(section_label).to_lowercase();
    if base.starts_with(SPOUSE_PREFIX) {
        Some((Owner::Spouse, false))
    } else if base.starts_with("dependent") {
        Some((Owner::Dependent, true))
    } else if KNOWN_SELF_LABELS.contains(&base.as_str()) {
        Some((Owner::Self_, false))
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn families_map_per_census_and_unknown_freezes() {
        assert_eq!(family_for_type("Gifts (Code)"), Some(Family::TypedFields));
        assert_eq!(
            family_for_type("Sponsored Travel"),
            Some(Family::TypedFields)
        );
        assert_eq!(family_for_type("Liabilities"), Some(Family::Flat));
        assert_eq!(
            family_for_type("Disclosure Summaries (Code)"),
            Some(Family::Itemized)
        );
        assert_eq!(family_for_type("Material Changes"), Some(Family::Itemized));
        assert_eq!(family_for_type("Recusals"), None, "out of scope — freeze");
        assert_eq!(family_for_type("Brand New Type"), None);
    }

    #[test]
    fn law_binary_is_enforced() {
        assert_eq!(law_code("Conflict of Interest Act"), Some("act"));
        assert_eq!(
            law_code("Conflict of Interest Code for Members of the House of Commons"),
            Some("code")
        );
        assert_eq!(law_code("Some Other Act"), None);
    }

    #[test]
    fn family_c_owner_maps_known_labels_spouse_and_statute_variant() {
        assert_eq!(family_c_owner("Assets"), Some((Owner::Self_, false)));
        assert_eq!(family_c_owner("Activities"), Some((Owner::Self_, false)));
        // Uppercase + statute-ref variant strips the `[…]` and matches.
        assert_eq!(
            family_c_owner("INVESTMENT IN PRIVATE CORPORATIONS [Paragraph 24(1)(a)]"),
            Some((Owner::Self_, false))
        );
        assert_eq!(
            family_c_owner("Spouse's/Common-Law Partner's assets"),
            Some((Owner::Spouse, false))
        );
        assert_eq!(
            family_c_owner("Spouse's/Common-Law Partner's source(s) of income"),
            Some((Owner::Spouse, false))
        );
        assert_eq!(
            family_c_owner("Dependent children"),
            Some((Owner::Dependent, true)),
            "unobserved — accepted with the −0.05 flag"
        );
        assert_eq!(
            family_c_owner("Cryptic New Section"),
            None,
            "reject → review"
        );
    }

    #[test]
    fn expected_h1_covers_the_fixtured_types() {
        assert_eq!(
            expected_h1("Gifts (Code)"),
            Some("Public Statement of Gifts or Other Benefits")
        );
        assert_eq!(expected_h1("Sponsored Travel"), None, "unarchived h1 gap");
    }
}
