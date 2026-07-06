//! Per-regime "zero-row parse is legitimate" opt-out (design invariant 6).
//!
//! `run.rs`'s `parse_and_stage()` and `parse_stage()`'s replay branch both
//! fail closed on a zero-row parse by default (invariant 6: "Zero-row parses
//! ... freeze the adapter and open a `review_task`") — a parser that silently
//! swallowed every row looks identical to a genuine zero-row source
//! document, so the default must reject both.
//!
//! `br` is the first regime where a zero-row parse is a real, expected
//! outcome of valid source data, not a parser defect: a candidate who
//! declares zero assets legitimately has no matching `bem_candidato` row at
//! all (`docs/regimes/br/AUTHORITY.md` Quirks log, 2026-07-06 finding —
//! confirmed directly against several `DEPUTADO FEDERAL` candidates in the
//! sampled data). `conformance.rs`'s harness already carries an analogous,
//! already-fixed exception scoped per FIXTURE (zero rows allowed only when
//! that fixture's own `expected.silver.json` is also `[]`); the real
//! `Runner` has no per-document ground truth to check a parse against, so
//! this hook is a per-REGIME opt-in instead.
//!
//! Follows the same `regime_code: &str` dispatch idiom as
//! [`crate::fingerprint_content::fingerprint_content`],
//! [`crate::redaction::redact`], and [`crate::conformance::check_details`].

/// Regimes whose zero-row parses are a legitimate outcome, not an
/// invariant-6 fail-closed condition. Empty for every regime not listed here
/// — every other regime rejects a zero-row parse exactly as before this hook
/// existed.
const REGIMES_ALLOWING_ZERO_ROWS: &[&str] = &["br"];

/// Whether `regime_code` treats a zero-row parse as legitimate rather than a
/// fail-closed invariant-6 violation.
#[must_use]
pub fn allowed(regime_code: &str) -> bool {
    REGIMES_ALLOWING_ZERO_ROWS.contains(&regime_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn br_is_allowed() {
        assert!(allowed("br"));
    }

    #[test]
    fn every_other_launch_regime_is_not_allowed() {
        // Zero blast radius (requirement 1): every regime code besides `br`
        // must still reject a zero-row parse exactly as before.
        for regime_code in [
            "us_house",
            "us_senate",
            "uk_commons_register",
            "canada_ciec",
            "australia_register",
            "eu_parliament_dpi",
            "fr_hatvp_dia",
            "de_bundestag",
            "fixture_fake",
        ] {
            assert!(!allowed(regime_code), "{regime_code} must not be allowed");
        }
    }
}
