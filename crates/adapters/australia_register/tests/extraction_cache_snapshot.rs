//! The four committed `extraction.cache.json` files (one per fixture case) MUST
//! equal the MECHANICAL transform of each case's `expected.silver.json`
//! (test-designer ground truth, goal 063 leg B) — the cache entries are
//! DERIVED via `prime_from_expected_silver`, never extracted by a live model.
//! This is what keeps `cargo run -p pipeline --bin conformance -- australia_register`
//! OFFLINE (`us_house` `scanned_paper_ptr` precedent, commit 2dfed7a).
//!
//! Regenerate deliberately with
//! `UPDATE_EXTRACTION_CACHE=1 cargo test -p australia_register --test extraction_cache_snapshot`
//! and commit the diff. NEVER hand-edit a cache file.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use serde_json::json;

use pipeline::conformance::fixtures_dir;
use pipeline::extraction::{CacheKey, CachedExtraction, prime_from_expected_silver};

/// (case dir, MANIFEST `cases.*.sha256` Bronze pin) — the sha256 is the raw
/// input.pdf bytes the conformance harness content-addresses.
const CASES: &[(&str, &str)] = &[
    (
        "shareholding_selfspouse",
        "cf09599e7c2aa71786c349c35b8afbafeddb6a46d8050a545b1528d2bc4c39bb",
    ),
    (
        "real_estate_textlayer",
        "f1822e24096b66aea77c78fbea8c791806e30ad80a983b94de6903d308ca21f0",
    ),
    (
        "scanned_alterations",
        "b632987be13fadacdec1ca5a12faa496a327f184ea603b1b92ccc15a8a35b68b",
    ),
    (
        "scanned_formA",
        "625134ad3d9959d3d46548273bcf1c3840b09c588a7f87d00de1a49a6b996806",
    ),
];

/// Extractor tag (regime doc §4) + default primary model (the cache key the
/// adapter's `LlmExtractor` looks up offline; `Models::from_env` default).
const EXTRACTOR_TAG: &str = "australia_register/llm@1";
const MODEL_ID: &str = "claude-haiku-4-5-20251001";

#[test]
fn extraction_cache_entries_are_primed_from_test_designer_ground_truth() {
    for (case, sha256) in CASES {
        let case_dir = fixtures_dir("australia_register").join(case);
        let expected_silver =
            std::fs::read_to_string(case_dir.join("expected.silver.json")).unwrap();
        let key = CacheKey::new(sha256, EXTRACTOR_TAG, MODEL_ID);
        let provenance = json!({
            "primed_from": "expected.silver.json",
            "ground_truth": "test-designer goal 063 leg B — independent transcription (pdftotext -layout + WinRT render), commit d3ed1a8",
            "derived_by": "pipeline::extraction::prime_from_expected_silver (mechanical transform, NOT a live LLM call)",
            "enforced_by": "cargo test -p australia_register --test extraction_cache_snapshot",
        });
        let derived = prime_from_expected_silver(&expected_silver, key, provenance).unwrap();

        let cache_path = case_dir.join("extraction.cache.json");
        if std::env::var_os("UPDATE_EXTRACTION_CACHE").is_some() {
            let mut text = serde_json::to_string_pretty(&derived).unwrap();
            text.push('\n');
            std::fs::write(&cache_path, text).unwrap();
        }
        let committed: CachedExtraction =
            serde_json::from_str(&std::fs::read_to_string(&cache_path).unwrap_or_else(|_| {
                panic!(
                    "missing {case}/extraction.cache.json; regenerate mechanically with \
                     UPDATE_EXTRACTION_CACHE=1"
                )
            }))
            .unwrap();
        assert_eq!(
            committed, derived,
            "{case}/extraction.cache.json drifted from expected.silver.json — regenerate \
             mechanically (UPDATE_EXTRACTION_CACHE=1), never hand-edit"
        );
        // Only the two MANIFEST confidence images occur (1.0 / 0.98f32).
        assert!(
            committed
                .rows
                .iter()
                .all(|row| row.confidence == 1.0f32 || row.confidence == 0.98f32)
        );
    }
}
