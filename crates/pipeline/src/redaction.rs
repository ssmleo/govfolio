//! Per-regime pre-publication redaction pass (design §7.5, goal 070). Removes
//! out-of-scope personal data (and drops un-republishable records) from PUBLIC
//! Gold BEFORE it is inserted, while Bronze and the staged Silver row keep the
//! raw verbatim (invariant 2: raw is sacred — redaction only ever touches the
//! Gold candidate that becomes public).
//!
//! Rules are DATA — one [`RegimeRedaction`] per regime code in [`RULESET`] —
//! applied deterministically by [`redact`], a pure function of `(regime_code,
//! candidate)`. Running it twice yields the same result (idempotent). Two kinds
//! of rule, both grounded in each regime doc's `personal_data_to_redact`
//! front-matter and §0.2 legal flags:
//!
//! - **Suppression** (the whole record is dropped): FR HATVP *patrimony*
//!   declarations (`dsp`/`dspm`/`dspfm`) are consultation-only — republishing
//!   any part is a €45,000 offence (Art. LO 135-2 code électoral;
//!   `docs/regimes/eu_fr_de_annual.md` §0.2). The adapter already excludes
//!   `dsp*` at discovery, so this is the belt-and-suspenders publication gate:
//!   any patrimony record that reaches publish is dropped, never Gold.
//! - **Field redaction** (keys stripped from `details`): third-party personal
//!   data that must not appear in public Gold even if it rode along in an
//!   extracted payload (e.g. `us_senate` paper-transmittal counsel names +
//!   signatures — regime doc `personal_data_to_redact`, E11). The keys are
//!   removed from the published `details`; the Bronze document and the staged
//!   Silver row keep every byte.
//!
//! `redact` runs in `publish` (and correction supersession) BEFORE the details
//! contract check, so a stored Gold row is exactly what passed the contract —
//! and a misconfigured rule that stripped a REQUIRED field fails closed at that
//! gate rather than persisting a contract-violating row.

use govfolio_core::domain::gold::GoldCandidate;

/// A per-regime redaction rule (data, not code).
struct RegimeRedaction {
    /// Regime code (`RegimeRef.code`, e.g. `fr_hatvp_dia`) this rule governs.
    regime_code: &'static str,
    /// `details` keys stripped from PUBLIC Gold (kept verbatim in Bronze/Silver).
    redact_detail_keys: &'static [&'static str],
    /// When present, records matching it are un-republishable — dropped whole.
    suppress: Option<Suppress>,
}

/// A whole-record suppression predicate: the record is dropped when its
/// `details[detail_key]` is a string beginning with any forbidden prefix
/// (case-insensitive).
struct Suppress {
    /// `details` key whose value decides suppression (e.g. `type_declaration`).
    detail_key: &'static str,
    /// Forbidden value prefixes (e.g. `dsp` for the FR patrimony family).
    forbidden_prefixes: &'static [&'static str],
    /// `review_task` reason recorded when a record is suppressed.
    reason: &'static str,
}

/// The v1 ruleset. Most launch regimes publish third-party data DELIBERATELY —
/// the official registers already do (see each regime doc's
/// `personal_data_to_redact`) — so their key lists are defensive; the one hard
/// legal line is FR patrimony suppression.
const RULESET: &[RegimeRedaction] = &[
    RegimeRedaction {
        regime_code: "fr_hatvp_dia",
        // The HATVP source itself redacts declarant private fields in-band
        // ('[Donnees non publiees]'); these keys are the belt-and-suspenders
        // strip if a declarant home address / private marker ever rode along.
        redact_detail_keys: &["declarant_address", "declarant_private"],
        suppress: Some(Suppress {
            detail_key: "type_declaration",
            // dsp / dspm / dspfm — la déclaration de situation patrimoniale.
            forbidden_prefixes: &["dsp"],
            reason: "redaction_fr_patrimony_unrepublishable",
        }),
    },
    RegimeRedaction {
        regime_code: "us_senate",
        // Paper transmittal cover letters carry third-party counsel name +
        // handwritten signature (regime doc §personal_data_to_redact, E11) —
        // never public.
        redact_detail_keys: &["counsel_name", "signature_image"],
        suppress: None,
    },
];

/// What redaction decided for one candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Redaction {
    /// Publish the (possibly stripped) candidate. `removed_keys` are the
    /// `details` keys removed from the public payload — empty when the regime
    /// had no rule or nothing to strip.
    Publish {
        /// `details` keys stripped from the public payload (for the audit log).
        removed_keys: Vec<String>,
    },
    /// The record is un-republishable and must NOT reach Gold. `reason` is the
    /// `review_task` reason to file.
    Suppress {
        /// `review_task` reason for the dropped record.
        reason: String,
    },
}

/// Applies the regime's redaction rule to `candidate` IN PLACE (public Gold
/// only — the caller passes the id-bound clone, never the source candidate, so
/// the Silver-equivalent in-memory row is untouched). Returns whether to
/// publish (with the stripped keys) or suppress the record entirely. A regime
/// with no registered rule publishes unchanged.
#[must_use]
pub fn redact(regime_code: &str, candidate: &mut GoldCandidate) -> Redaction {
    let Some(rule) = RULESET.iter().find(|r| r.regime_code == regime_code) else {
        return Redaction::Publish {
            removed_keys: Vec::new(),
        };
    };
    if let Some(suppress) = &rule.suppress
        && is_suppressed(candidate, suppress)
    {
        return Redaction::Suppress {
            reason: suppress.reason.to_owned(),
        };
    }
    let mut removed = Vec::new();
    if let serde_json::Value::Object(map) = &mut candidate.details {
        for key in rule.redact_detail_keys {
            if map.remove(*key).is_some() {
                removed.push((*key).to_owned());
            }
        }
    }
    Redaction::Publish {
        removed_keys: removed,
    }
}

/// Whether `candidate` matches a suppression predicate (a `details` string value
/// beginning with a forbidden prefix, case-insensitive).
fn is_suppressed(candidate: &GoldCandidate, suppress: &Suppress) -> bool {
    let Some(value) = candidate
        .details
        .get(suppress.detail_key)
        .and_then(serde_json::Value::as_str)
    else {
        return false;
    };
    let lower = value.to_ascii_lowercase();
    suppress
        .forbidden_prefixes
        .iter()
        .any(|prefix| lower.starts_with(&prefix.to_ascii_lowercase()))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use govfolio_core::domain::enums::{AssetClass, RecordType, Side};

    use super::*;

    fn fr_interest(details: serde_json::Value) -> GoldCandidate {
        GoldCandidate {
            filing_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse().unwrap(),
            politician_id: "01BX5ZZKBKACTAV9WEVGEMMVRZ".parse().unwrap(),
            regime_id: "01BX5ZZKBKACTAV9WEVGEMMVS0".parse().unwrap(),
            instrument_id: None,
            asset_description_raw: "Activité de conseil — Cabinet X".to_owned(),
            record_type: RecordType::Interest,
            asset_class: AssetClass::Other,
            side: None,
            transaction_date: None,
            as_of_date: None,
            notified_date: Some(NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()),
            value: None,
            owner: None,
            extraction_confidence: Some(0.99),
            extracted_by: "fixture:redaction@0".to_owned(),
            fingerprint: None,
            details,
        }
    }

    #[test]
    fn redact_is_noop_for_regime_without_rules() {
        let mut candidate = fr_interest(json!({ "type_declaration": "dia" }));
        candidate.regime_id = "01BX5ZZKBKACTAV9WEVGEMMVS0".parse().unwrap();
        let before = candidate.details.clone();
        let decision = redact("us_house", &mut candidate);
        assert_eq!(
            decision,
            Redaction::Publish {
                removed_keys: vec![]
            }
        );
        assert_eq!(candidate.details, before, "no rule => untouched");
    }

    #[test]
    fn redact_drops_flagged_detail_keys_and_keeps_the_rest() {
        let mut candidate = fr_interest(json!({
            "type_declaration": "dia",
            "section_tag": "activitesProfessionnellesDto",
            "declarant_address": "12 rue Privée, 75000 Paris",
            "declarant_private": "unlisted",
        }));
        let before_desc = candidate.asset_description_raw.clone();
        let decision = redact("fr_hatvp_dia", &mut candidate);
        match decision {
            Redaction::Publish { mut removed_keys } => {
                removed_keys.sort();
                assert_eq!(removed_keys, ["declarant_address", "declarant_private"]);
            }
            Redaction::Suppress { .. } => panic!("a dia record must publish, not suppress"),
        }
        // Flagged keys gone; unflagged keys + the as-filed raw stay (invariant 2).
        assert!(candidate.details.get("declarant_address").is_none());
        assert!(candidate.details.get("declarant_private").is_none());
        assert_eq!(candidate.details["type_declaration"], json!("dia"));
        assert_eq!(
            candidate.details["section_tag"],
            json!("activitesProfessionnellesDto")
        );
        assert_eq!(candidate.asset_description_raw, before_desc);
    }

    #[test]
    fn redact_is_idempotent() {
        let mut once = fr_interest(json!({
            "type_declaration": "dia",
            "declarant_address": "x",
        }));
        let mut twice = once.clone();
        let _ = redact("fr_hatvp_dia", &mut once);
        let _ = redact("fr_hatvp_dia", &mut twice);
        let _ = redact("fr_hatvp_dia", &mut twice); // second application
        assert_eq!(once.details, twice.details, "redaction is idempotent");
    }

    #[test]
    fn redact_suppresses_fr_patrimony_records() {
        for code in ["dsp", "dspm", "DSPFM"] {
            let mut candidate = fr_interest(json!({ "type_declaration": code }));
            let decision = redact("fr_hatvp_dia", &mut candidate);
            assert_eq!(
                decision,
                Redaction::Suppress {
                    reason: "redaction_fr_patrimony_unrepublishable".to_owned()
                },
                "patrimony {code} is un-republishable (Art. LO 135-2)"
            );
        }
    }

    #[test]
    fn redact_publishes_in_scope_fr_declarations() {
        for code in ["dia", "diam", "DIA"] {
            let mut candidate = fr_interest(json!({ "type_declaration": code }));
            assert!(
                matches!(
                    redact("fr_hatvp_dia", &mut candidate),
                    Redaction::Publish { .. }
                ),
                "dia/diam are in scope and publish"
            );
        }
    }

    #[test]
    fn redact_strips_us_senate_paper_transmittal_pii() {
        let mut candidate = fr_interest(json!({
            "amount_band_raw": "$1,001 - $15,000",
            "counsel_name": "Jane Roe, Esq.",
            "signature_image": "data:image/png;base64,AAAA",
        }));
        candidate.record_type = RecordType::Transaction;
        candidate.side = Some(Side::Buy);
        candidate.transaction_date = Some(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap());
        let decision = redact("us_senate", &mut candidate);
        match decision {
            Redaction::Publish { mut removed_keys } => {
                removed_keys.sort();
                assert_eq!(removed_keys, ["counsel_name", "signature_image"]);
            }
            Redaction::Suppress { .. } => panic!("no suppression rule for us_senate"),
        }
        assert!(candidate.details.get("counsel_name").is_none());
        assert!(candidate.details.get("signature_image").is_none());
        assert_eq!(
            candidate.details["amount_band_raw"],
            json!("$1,001 - $15,000")
        );
    }
}
