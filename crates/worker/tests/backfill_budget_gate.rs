//! Goal 081 Task 4: `BACKFILL_BUDGET` gate — unit-level proof, no Postgres or
//! network needed (mirrors `scripts/check-tf-plan.sh`'s numeric-count-vs-
//! env-var-budget shape; the underlying `dry_run` classification itself is
//! already covered by `crates/worker/src/backfill.rs`'s own tests). Runs in
//! the default (non-`--ignored`) suite, per this crate's convention that
//! `--ignored` is reserved for tests needing a live Postgres/network
//! dependency (see `backfill.rs`, `backfill_real.rs`).
#![allow(clippy::unwrap_used)]

use std::collections::BTreeMap;

use async_trait::async_trait;
use serde_json::json;

use govfolio_core::domain::gold::GoldCandidate;
use worker::backfill::{
    ArchiveSource, BudgetVerdict, DiscoveredFiling, NoBaseline, gate_year, log_budget_skip,
};

/// A minimal `us_house` PTR candidate (unbound identity, as `normalize`
/// would emit it) — status `"New"`, so every fake filing here classifies as
/// an ADD against `NoBaseline` (never an amendment/supersession), keeping
/// `record_delta` == candidate count == filing count for these fakes.
fn candidate() -> GoldCandidate {
    serde_json::from_value(json!({
        "filing_id": "00000000000000000000000000",
        "politician_id": "00000000000000000000000000",
        "regime_id": "00000000000000000000000000",
        "instrument_id": null,
        "asset_description_raw": "Apple Inc. (AAPL) [ST]",
        "record_type": "transaction",
        "asset_class": "equity",
        "side": "buy",
        "transaction_date": "2026-03-02",
        "as_of_date": null,
        "notified_date": "2026-03-02",
        "value": {"low": "1001.00", "high": "15000.00", "currency": "USD"},
        "owner": "self",
        "extraction_confidence": 0.98,
        "extracted_by": "us_house_ptr/text@1",
        "fingerprint": null,
        "details": {"filing_status_raw": "New"}
    }))
    .unwrap()
}

/// One synthetic archive year: `n_filings` filings, each dry-processing to
/// ONE new (unpublished, non-amendment) candidate — so `record_delta ==
/// n_filings` for this fake, against [`NoBaseline`].
struct FakeYear {
    year: i32,
    filings: Vec<DiscoveredFiling>,
    candidates: BTreeMap<String, Vec<GoldCandidate>>,
}

impl FakeYear {
    fn new(year: i32, n_filings: usize) -> Self {
        let filings: Vec<_> = (0..n_filings)
            .map(|i| DiscoveredFiling {
                external_id: format!("doc{year}-{i}"),
                url: format!("file://doc{year}-{i}"),
            })
            .collect();
        let candidates = filings
            .iter()
            .map(|f| (f.external_id.clone(), vec![candidate()]))
            .collect();
        Self {
            year,
            filings,
            candidates,
        }
    }
}

#[async_trait]
impl ArchiveSource for FakeYear {
    async fn discover_year(&self, year: i32) -> anyhow::Result<Vec<DiscoveredFiling>> {
        anyhow::ensure!(year == self.year, "this fake only knows year {}", self.year);
        Ok(self.filings.clone())
    }

    async fn dry_process(&self, filing: &DiscoveredFiling) -> anyhow::Result<Vec<GoldCandidate>> {
        Ok(self
            .candidates
            .get(&filing.external_id)
            .cloned()
            .unwrap_or_default())
    }
}

/// A scratch root under the OS temp dir — `log_budget_skip` writes its
/// `agents/JOURNAL.md` here, never touching the real repo's journal file.
fn scratch_root(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "govfolio-backfill-budget-gate-test-{tag}-{}",
        std::process::id()
    ))
}

#[tokio::test]
async fn high_count_year_halts_skips_the_write_and_logs_cleanly() {
    let budget = 5;
    let year = 2018;
    let over_budget_delta = budget + 5; // 10 filings, record_delta 10 > budget 5
    let source = FakeYear::new(year, over_budget_delta);

    let verdict = gate_year(&source, &NoBaseline, "us_house", year, budget)
        .await
        .unwrap();
    assert_eq!(
        verdict,
        BudgetVerdict::Skip {
            record_delta: over_budget_delta
        },
        "over-budget year halts (does not proceed)"
    );

    // Mirrors bin/backfill-real.rs's per-year wiring: on Skip, the real write
    // is never attempted, and the skip is logged — nothing blocks the range.
    let mut write_attempts = 0;
    let root = scratch_root("high");
    match verdict {
        BudgetVerdict::Skip { record_delta } => {
            log_budget_skip(&root, "us_house", year, record_delta, budget).unwrap();
        }
        BudgetVerdict::Proceed { .. } => {
            write_attempts += 1;
        }
    }
    assert_eq!(
        write_attempts, 0,
        "the real write is never attempted for a skipped year"
    );

    let journal = std::fs::read_to_string(root.join("agents").join("JOURNAL.md")).unwrap();
    assert!(
        journal.contains("081/T4"),
        "journal line references the goal/task: {journal:?}"
    );
    assert!(
        journal.contains(&format!("us_house {year}")),
        "journal line names the skipped year: {journal:?}"
    );
    assert!(
        journal.contains(&format!("record_delta={over_budget_delta}")),
        "journal line carries the over-budget count: {journal:?}"
    );
    assert!(
        journal.contains(&format!("budget={budget}")),
        "journal line carries the budget: {journal:?}"
    );

    // returns cleanly — no panic, no error, function call above already
    // proved this by not unwrapping to a panic.
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn low_count_year_proceeds_to_a_real_write() {
    let budget = 5;
    let year = 2026;
    let under_budget_delta = 2; // 2 filings, record_delta 2 <= budget 5
    let source = FakeYear::new(year, under_budget_delta);

    let verdict = gate_year(&source, &NoBaseline, "us_house", year, budget)
        .await
        .unwrap();
    assert_eq!(
        verdict,
        BudgetVerdict::Proceed {
            record_delta: under_budget_delta
        },
        "under-budget year proceeds"
    );

    // Mirrors bin/backfill-real.rs: Proceed drives the real write pass
    // (mocked here — no Postgres/network in this non-`--ignored` test).
    let mut write_attempts = 0;
    match verdict {
        BudgetVerdict::Proceed { .. } => write_attempts += 1,
        BudgetVerdict::Skip { .. } => panic!("must not skip a year within budget"),
    }
    assert_eq!(
        write_attempts, 1,
        "the real write is attempted exactly once for an in-budget year"
    );
}
