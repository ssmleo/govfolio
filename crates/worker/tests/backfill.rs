//! Backfill dry-run integration tests (goal 080). DB-gated like the other sqlx
//! suites: `--ignored` + postgres on `DATABASE_URL`.
//!
//! Proves, against a live schema, using the four ELECTRONIC `us_house` fixtures
//! (the scanned paper fixture is the LLM-seam case — out of scope for the
//! offline archive here):
//! - a dry run over EMPTY Gold reports adds + one supersession (the amendment
//!   fixture arrives as a new `DocID` with an `Amended` row — §3.7), and writes
//!   NOTHING (every table's row count is unchanged);
//! - a dry run AFTER the same filings are published reports them all as
//!   `unchanged` — which only holds if the dry run reproduces the publish
//!   stage's fingerprints EXACTLY (supersede-not-mutate: an unchanged reprocess
//!   inserts nothing, invariant 4) — and again writes nothing.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use sqlx::{AssertSqlSafe, PgPool};

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx};
use pipeline::conformance::{fixtures_dir, workspace_root};
use pipeline::run::{LocalFiling, Runner};
use pipeline::stages::roster::seed_roster;
use pipeline::stages::seed::seed_regime;
use us_house::UsHouseAdapter;
use us_house::binding::UsHouseBinding;
use worker::backfill::{
    ArchiveSource, DiffReport, DiscoveredFiling, FilingClass, PgBaseline, classify, dry_run,
};

/// Every table migrations 0001+0002+0004 create — the "wrote nothing" assertion
/// sweeps ALL of them.
const ALL_TABLES: &[&str] = &[
    "jurisdiction",
    "disclosure_regime",
    "politician",
    "politician_alias",
    "mandate",
    "instrument",
    "instrument_alias",
    "raw_document",
    "filing",
    "disclosure_record",
    "review_task",
    "pipeline_run",
    "outbox_event",
    "stg_us_house",
    "stg_meta",
    "extraction_cache",
];

async fn table_counts(pool: &PgPool) -> Vec<(String, i64)> {
    let mut counts = Vec::with_capacity(ALL_TABLES.len());
    for table in ALL_TABLES {
        let n: i64 = sqlx::query_scalar(AssertSqlSafe(format!("select count(*) from {table}")))
            .fetch_one(pool)
            .await
            .unwrap();
        counts.push(((*table).to_owned(), n));
    }
    counts
}

fn evidence_index_xml() -> String {
    let path = workspace_root()
        .join("docs")
        .join("regimes")
        .join("us-house")
        .join("evidence")
        .join("94781947c3975677a2fa8f7839f6c0f074b3d3a2ff6019b3cfd8ee4942f6262e.2026FD-slice.xml");
    std::fs::read_to_string(path).unwrap()
}

/// The four ELECTRONIC fixture `input.pdf`s (text-layer, no LLM, no network).
fn electronic_fixture_pdfs() -> Vec<PathBuf> {
    let root = fixtures_dir("us_house");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        // The scanned paper PTR is the LLM-seam case (goal 021) — excluded here.
        .filter(|p| p.file_name().and_then(|n| n.to_str()) != Some("scanned_paper_ptr"))
        .collect();
    dirs.sort();
    assert_eq!(dirs.len(), 4, "four electronic us_house fixture cases");
    dirs.into_iter().map(|dir| dir.join("input.pdf")).collect()
}

fn temp_bronze(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "govfolio-backfill-{tag}-{}-{nanos}",
        std::process::id()
    ))
}

/// An offline [`ArchiveSource`] over the local electronic fixtures — the same
/// `UsHouseAdapter` parse/normalize the live [`ClerkArchive`] uses, but fed from
/// disk (no network). All fixtures present under one archive year.
struct FixtureArchive {
    adapter: UsHouseAdapter,
    ctx: RunCtx,
    pdfs: Vec<PathBuf>,
    year: i32,
}

impl FixtureArchive {
    fn new(pool: &PgPool) -> Self {
        let adapter = UsHouseAdapter::default();
        let ctx = RunCtx::new(
            BronzeStore::open(temp_bronze("fixture-archive")).unwrap(),
            Some(pool.clone()), // Some => normalize runs in unbound (prod) mode
            Clock::System,
            &adapter.politeness(),
        )
        .unwrap();
        Self {
            adapter,
            ctx,
            pdfs: electronic_fixture_pdfs(),
            year: 2026,
        }
    }

    /// read → scratch Bronze → parse → normalize (no persistence).
    async fn process(&self, path: &Path) -> anyhow::Result<Vec<GoldCandidate>> {
        let bytes = std::fs::read(path)?;
        let doc = self.ctx.bronze.put(&bytes)?;
        let rows = self.adapter.parse(&doc, &self.ctx).await?;
        self.adapter.normalize(&rows, &self.ctx).await
    }
}

#[async_trait]
impl ArchiveSource for FixtureArchive {
    async fn discover_year(&self, year: i32) -> anyhow::Result<Vec<DiscoveredFiling>> {
        if year != self.year {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for path in &self.pdfs {
            let candidates = self.process(path).await?;
            let external_id = candidates[0].details["doc_id"].as_str().unwrap().to_owned();
            out.push(DiscoveredFiling {
                external_id,
                url: path.display().to_string(),
            });
        }
        Ok(out)
    }

    async fn dry_process(&self, filing: &DiscoveredFiling) -> anyhow::Result<Vec<GoldCandidate>> {
        self.process(Path::new(&filing.url)).await
    }
}

async fn migrate_seed_regime_and_roster(pool: &PgPool) {
    govfolio_core::db::migrate(pool).await.unwrap();
    seed_regime(pool, &us_house::seed::regime_seed())
        .await
        .unwrap();
    let roster = us_house::seed::roster_from_index_xml(&evidence_index_xml()).unwrap();
    seed_roster(pool, &us_house::seed::regime_binding(), &roster)
        .await
        .unwrap();
}

fn pg_baseline(pool: &PgPool) -> PgBaseline {
    PgBaseline::new(pool.clone(), us_house::seed::REGIME_ID.to_owned())
}

async fn assert_wrote_nothing(pool: &PgPool, before: &[(String, i64)]) {
    let after = table_counts(pool).await;
    assert_eq!(
        before, &after,
        "dry-run must write NOTHING to Bronze/Silver/Gold or any table"
    );
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn dry_run_on_empty_gold_reports_adds_and_supersession_and_writes_nothing(pool: PgPool) {
    migrate_seed_regime_and_roster(&pool).await;
    let source = FixtureArchive::new(&pool);
    let before = table_counts(&pool).await;

    let report: DiffReport = dry_run(&source, &pg_baseline(&pool), 2026, 2026, 10)
        .await
        .unwrap();

    let year = &report.years[0];
    assert_eq!(year.discovered, 4, "four electronic fixtures");
    assert_eq!(year.sampled, 4);
    assert_eq!(year.failed, Vec::<String>::new(), "all parse offline");
    // The amendment fixture (DocID 20033759, `F S : Amended`) is a supersession;
    // the other three are pure adds (§3.7).
    assert_eq!(year.supersessions, 1, "the amendment fixture");
    assert_eq!(year.adds, 3);
    assert_eq!(year.unchanged, 0);
    assert_eq!(year.changes, 0);

    assert_wrote_nothing(&pool, &before).await;
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn dry_run_after_publish_reports_unchanged_and_writes_nothing(pool: PgPool) {
    migrate_seed_regime_and_roster(&pool).await;

    // Publish the four electronic fixtures through the REAL pipeline first.
    let adapter = UsHouseAdapter::default();
    let binding = UsHouseBinding;
    let ctx = RunCtx::new(
        BronzeStore::open(temp_bronze("publish")).unwrap(),
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )
    .unwrap();
    let runner = Runner::new(&adapter, &binding, us_house::seed::regime_binding(), ctx).unwrap();
    let inputs: Vec<LocalFiling> = electronic_fixture_pdfs()
        .into_iter()
        .map(|path| LocalFiling { path })
        .collect();
    let published = runner.run_local(&inputs).await.unwrap();
    assert_eq!(published.failed, Vec::<String>::new());
    assert!(published.gold_inserted >= 4, "fixtures published");

    // Now dry-run the SAME filings: every one must classify as unchanged, which
    // only holds if the dry run reproduces the publish fingerprints exactly.
    let source = FixtureArchive::new(&pool);
    let before = table_counts(&pool).await;
    let report = dry_run(&source, &pg_baseline(&pool), 2026, 2026, 10)
        .await
        .unwrap();

    let year = &report.years[0];
    assert_eq!(year.sampled, 4);
    assert_eq!(
        year.unchanged, 4,
        "immutable PDFs + same extractor => replay inserts nothing (fingerprint parity)"
    );
    assert_eq!(year.adds, 0);
    assert_eq!(year.supersessions, 0);
    assert_eq!(year.changes, 0);
    assert_eq!(year.failed, Vec::<String>::new());

    assert_wrote_nothing(&pool, &before).await;
}

/// Direct check that the baseline lookup + classify pair reproduces publish
/// fingerprints (belt-and-suspenders on the parity the `unchanged` test relies
/// on): a published filing looked up and re-classified is `Unchanged`.
#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn baseline_lookup_reproduces_publish_fingerprints(pool: PgPool) {
    use worker::backfill::GoldBaseline as _;

    migrate_seed_regime_and_roster(&pool).await;
    let adapter = UsHouseAdapter::default();
    let ctx = RunCtx::new(
        BronzeStore::open(temp_bronze("parity")).unwrap(),
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )
    .unwrap();
    let runner = Runner::new(
        &adapter,
        &UsHouseBinding,
        us_house::seed::regime_binding(),
        ctx,
    )
    .unwrap();
    let inputs: Vec<LocalFiling> = electronic_fixture_pdfs()
        .into_iter()
        .map(|path| LocalFiling { path })
        .collect();
    runner.run_local(&inputs).await.unwrap();

    let source = FixtureArchive::new(&pool);
    let baseline = pg_baseline(&pool);
    for filing in source.discover_year(2026).await.unwrap() {
        let candidates = source.dry_process(&filing).await.unwrap();
        let base = baseline.lookup(&filing.external_id).await.unwrap();
        assert!(
            base.is_some(),
            "published filing {} must have a baseline",
            filing.external_id
        );
        assert_eq!(
            classify(base.as_ref(), &candidates).unwrap(),
            FilingClass::Unchanged,
            "filing {} fingerprints must match publish",
            filing.external_id
        );
    }
}
