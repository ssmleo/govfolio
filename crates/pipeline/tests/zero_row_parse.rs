//! `pipeline::run::Runner`'s per-regime "zero-row parse is legitimate" gate
//! (`crates/pipeline/src/zero_rows.rs`; `docs/regimes/br/AUTHORITY.md` Quirks
//! log, 2026-07-06 finding). DB-gated like the other sqlx suites: `--ignored`
//! + postgres on `DATABASE_URL`.
//!
//! A synthetic adapter/binding pair whose `parse()` always returns zero rows
//! is built here rather than reusing a real adapter crate: crafting a
//! genuine zero-row `us_house`/`br` fixture is out of scope for this fix (no
//! adapter/fixture files are touched). `crates/pipeline/src/zero_rows.rs`'s
//! own unit tests already enumerate every real launch regime code (`br`
//! allowed, `us_house` and every other launch regime NOT allowed); this
//! suite instead proves the actual `Runner` control flow end to end, for
//! both sides of the gate:
//! - `br`: a zero-row parse succeeds, publishes nothing (no filing, no Gold,
//!   no outbox), and the run report reflects a processed-but-empty filing,
//!   not a failure;
//! - any other regime code (`other_regime` here — the pure `zero_rows`
//!   allow-list is what actually excludes `us_house`, tested there): a
//!   zero-row parse still fails closed with the EXACT pre-existing
//!   invariant-6 message, landing in `RunReport::failed` — zero blast
//!   radius, unchanged from before this fix.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use sqlx::PgPool;

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{
    BronzeStore, Clock, FilingRef, JurisdictionAdapter, PolitenessCfg, RawDocRef, RegimeRef,
    RunCtx, StagingRow,
};
use pipeline::run::{
    FilingIdentity, LocalFiling, RegimeBinding, RunReport, Runner, RunnerBinding, StagedSilver,
};

/// Adapter whose `parse()` always yields zero rows regardless of the Bronze
/// document's content — the one behavior this suite needs to control. `code`
/// is the only thing that varies between the two tests below.
#[derive(Debug, Clone, Copy)]
struct ZeroRowAdapter {
    code: &'static str,
}

#[async_trait]
impl JurisdictionAdapter for ZeroRowAdapter {
    fn regime(&self) -> RegimeRef {
        RegimeRef { code: self.code }
    }

    fn politeness(&self) -> PolitenessCfg {
        PolitenessCfg::new(Duration::from_secs(1), "test@govfolio.io")
    }

    async fn discover(&self, _ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        Ok(Vec::new())
    }

    async fn fetch(&self, _r: &FilingRef, _ctx: &RunCtx) -> anyhow::Result<RawDocRef> {
        unreachable!("run_local drives Bronze via LocalFiling, never this trait method")
    }

    async fn parse(&self, _d: &RawDocRef, _ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        Ok(Vec::new())
    }

    async fn normalize(
        &self,
        _rows: &[StagingRow],
        _ctx: &RunCtx,
    ) -> anyhow::Result<Vec<GoldCandidate>> {
        unreachable!(
            "a zero-row parse must never reach normalize: either the parse-stage ensure! \
             fails closed first (non-opted-in regime), or publish_document's early return \
             skips straight past it (opted-in regime)"
        )
    }
}

/// Binding paired with [`ZeroRowAdapter`]. `filing_identity` fails closed on
/// an empty slice exactly like the real `BrBinding::filing_identity` does
/// (`no_rows_fails_closed`, `crates/adapters/br/src/binding.rs`) — proving
/// it is `publish_document`'s early return, not this binding, that keeps a
/// legitimate zero-row parse from ever reaching it.
#[derive(Debug, Clone, Copy)]
struct ZeroRowBinding;

#[async_trait]
impl RunnerBinding for ZeroRowBinding {
    fn silver_table(&self) -> &'static str {
        "stg_zero_row_test"
    }

    fn filing_identity(&self, rows: &[StagingRow]) -> anyhow::Result<FilingIdentity> {
        anyhow::ensure!(
            !rows.is_empty(),
            "no silver rows — cannot derive filing identity"
        );
        unreachable!("this suite never stages a non-empty row")
    }

    async fn stage_silver(
        &self,
        _pool: &PgPool,
        _raw_document_id: &str,
        rows: &[StagingRow],
    ) -> anyhow::Result<Vec<StagedSilver>> {
        assert!(
            rows.is_empty(),
            "this suite only ever stages a zero-row parse"
        );
        Ok(Vec::new())
    }

    async fn load_silver(
        &self,
        _pool: &PgPool,
        _raw_document_id: &str,
    ) -> anyhow::Result<Vec<StagingRow>> {
        Ok(Vec::new())
    }

    fn review_reasons(&self, _candidate: &GoldCandidate) -> Vec<String> {
        Vec::new()
    }
}

fn temp_path(tag: &str, suffix: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "govfolio-zero-row-{tag}-{}-{nanos}{suffix}",
        std::process::id()
    ))
}

fn dummy_regime_binding() -> RegimeBinding {
    RegimeBinding {
        regime_id: "test-regime".to_owned(),
        jurisdiction_id: "test-jurisdiction".to_owned(),
        body: "Test Body".to_owned(),
    }
}

async fn run_once(
    pool: &PgPool,
    code: &'static str,
    bronze_root: &Path,
    input: &Path,
) -> RunReport {
    let adapter = ZeroRowAdapter { code };
    let binding = ZeroRowBinding;
    let ctx = RunCtx::new(
        BronzeStore::open(bronze_root).unwrap(),
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )
    .unwrap();
    let runner = Runner::new(&adapter, &binding, dummy_regime_binding(), ctx).unwrap();
    runner
        .run_local(&[LocalFiling {
            path: input.to_path_buf(),
        }])
        .await
        .unwrap()
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn br_zero_row_parse_succeeds_and_publishes_nothing(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let bronze_root = temp_path("br-allowed", "");
    let input = temp_path("br-allowed-input", ".bin");
    std::fs::write(&input, b"zero-asset candidate declaration").unwrap();

    let report = run_once(&pool, "br", &bronze_root, &input).await;

    assert_eq!(
        report.failed,
        Vec::<String>::new(),
        "a zero-row br parse must not fail"
    );
    assert_eq!(report.filings, 1);
    assert_eq!(
        report.published, 1,
        "the publish stage still ran to completion, just with nothing to write"
    );
    assert_eq!(report.gold_inserted, 0);
    assert_eq!(report.outbox_written, 0);
    assert_eq!(report.review_tasks, 0);

    let (filings, gold): (i64, i64) = sqlx::query_as(
        "select (select count(*) from filing), (select count(*) from disclosure_record)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        filings, 0,
        "no filing identity could be derived from zero rows — nothing to publish"
    );
    assert_eq!(gold, 0);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn non_br_zero_row_parse_still_fails_closed_exactly_as_before(pool: PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let bronze_root = temp_path("reject", "");
    let input = temp_path("reject-input", ".bin");
    std::fs::write(&input, b"some non-br document").unwrap();

    let report = run_once(&pool, "other_regime", &bronze_root, &input).await;

    assert_eq!(report.filings, 1);
    assert_eq!(report.published, 0, "the publish stage never ran");
    assert_eq!(report.gold_inserted, 0);
    assert_eq!(
        report.failed.len(),
        1,
        "a zero-row parse must fail closed for every non-opted-in regime"
    );
    assert!(
        report.failed[0].contains("parse produced zero rows for"),
        "the exact pre-existing message must survive unchanged: {:?}",
        report.failed
    );
    assert!(
        report.failed[0].contains("fail closed (invariant 6)"),
        "the exact pre-existing message must survive unchanged: {:?}",
        report.failed
    );

    let (filings, gold): (i64, i64) = sqlx::query_as(
        "select (select count(*) from filing), (select count(*) from disclosure_record)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(filings, 0, "the failure is total, nothing partial");
    assert_eq!(gold, 0);
}
