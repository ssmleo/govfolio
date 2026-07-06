//! Mechanically primes the THREE EU DPI offline extraction caches
//! (`fixtures/eu_*/extraction.cache.json`) from `expected.silver.json` ground
//! truth via `pipeline::extraction::prime_from_expected_silver` — no LLM call is
//! involved (the `australia_register` / `us_house` precedent, fixtures
//! `MANIFEST.json` `builder_notes.eu`). The committed cache is what keeps EU
//! conformance OFFLINE. Regenerate deliberately with
//! `PRIME_EU_CACHE=1 cargo test -p eu_fr_de_annual --test prime_eu_cache`
//! (with NO `GOVFOLIO_LLM_PRIMARY_MODEL` override) and commit the three files.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

use pipeline::adapter::BronzeStore;
use pipeline::extraction::{CacheKey, Models, prime_from_expected_silver};

const EU_CASES: &[&str] = &["eu_eur_exact", "eu_pln_multilingual", "eu_english_baseline"];
const EXTRACTOR_TAG: &str = "eu_parliament_dpi/llm@1";

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

/// Regeneration (opt-in): writes the three committed cache files.
#[test]
fn prime_eu_caches() {
    if std::env::var_os("PRIME_EU_CACHE").is_none() {
        return; // no-op unless explicitly regenerating
    }
    let model = Models::from_env().primary;
    let bronze = BronzeStore::open(
        std::env::temp_dir().join(format!("prime-eu-cache-{}", std::process::id())),
    )
    .expect("bronze");

    for case in EU_CASES {
        let dir = fixtures_dir().join(case);
        let pdf = fs::read(dir.join("input.pdf")).expect("read input.pdf");
        let sha = bronze.put(&pdf).expect("hash pdf").sha256;
        let silver = fs::read_to_string(dir.join("expected.silver.json")).expect("read silver");
        let key = CacheKey::new(&sha, EXTRACTOR_TAG, &model);
        let provenance = serde_json::json!({
            "primed_from": "expected.silver.json ground truth",
            "method": "prime_from_expected_silver (no LLM call)",
            "case": case,
        });
        let entry = prime_from_expected_silver(&silver, key, provenance).expect("prime");
        let mut json = serde_json::to_string_pretty(&entry).expect("serialize");
        json.push('\n');
        fs::write(dir.join("extraction.cache.json"), json).expect("write cache");
    }
}

/// Sanity: whatever is committed must round-trip to a non-empty cache under the
/// default model id (so EU conformance stays offline). Skipped when a model
/// override is set (the committed key pins the default).
#[test]
fn committed_caches_are_present_and_keyed_to_the_default_model() {
    if std::env::var_os("GOVFOLIO_LLM_PRIMARY_MODEL").is_some() {
        return;
    }
    let model = Models::from_env().primary;
    for case in EU_CASES {
        let path = fixtures_dir().join(case).join("extraction.cache.json");
        let text = fs::read_to_string(&path)
            .unwrap_or_else(|_| panic!("missing committed cache {}", path.display()));
        let value: serde_json::Value = serde_json::from_str(&text).expect("cache is json");
        assert_eq!(
            value["key"]["extractor_tag"], EXTRACTOR_TAG,
            "cache {case} extractor tag"
        );
        assert_eq!(
            value["key"]["model_id"], model,
            "cache {case} must be keyed to the default primary model"
        );
        assert!(
            value["rows"].as_array().is_some_and(|r| !r.is_empty()),
            "cache {case} must carry rows"
        );
    }
}
