//! sentinel WATCH (goal 017): drift probes -> ranked, deduped, auto-filed drift
//! reports. The transport (probe) and the store are traits, so the whole engine
//! runs OFFLINE here against a scripted probe + an in-memory store — no network,
//! no database. The one DB-touching case (the real `PgWatchStore`) is `--ignored`
//! like the other sqlx suites (postgres on `DATABASE_URL`).
#![allow(clippy::unwrap_used)]

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::sync::Mutex;

use async_trait::async_trait;

use worker::sentinel::{
    DriftKind, DriftOutcome, DriftReport, ProbeResult, SourceProbe, WatchState, WatchStore,
    WatchTarget, classify, observe, rank, watch_pass,
};

// --------------------------------------------------------------------------
// Offline doubles: a scripted probe and an in-memory store.
// --------------------------------------------------------------------------

/// Returns queued observations per url; the last scripted step sticks so the
/// same url can be probed across many passes.
#[derive(Default)]
struct ScriptedProbe {
    steps: Mutex<HashMap<String, VecDeque<Result<ProbeResult, String>>>>,
}

impl ScriptedProbe {
    fn script(&self, url: &str, steps: Vec<ProbeResult>) {
        self.script_raw(url, steps.into_iter().map(Ok).collect());
    }
    fn script_raw(&self, url: &str, steps: Vec<Result<ProbeResult, String>>) {
        self.steps
            .lock()
            .unwrap()
            .insert(url.to_owned(), steps.into());
    }
}

#[async_trait]
impl SourceProbe for ScriptedProbe {
    async fn fetch(
        &self,
        url: &str,
        _if_none_match: Option<&str>,
        _if_modified_since: Option<&str>,
    ) -> anyhow::Result<ProbeResult> {
        let mut map = self.steps.lock().unwrap();
        let queue = map
            .get_mut(url)
            .ok_or_else(|| anyhow::anyhow!("no script for {url}"))?;
        let step = if queue.len() > 1 {
            queue.pop_front().unwrap()
        } else {
            queue.front().cloned().unwrap()
        };
        step.map_err(|e| anyhow::anyhow!(e))
    }
}

#[derive(Default)]
struct MockStore {
    states: Mutex<HashMap<String, WatchState>>,
    detections: Mutex<HashMap<String, u32>>, // by dedup_key
    frozen: Mutex<HashSet<String>>,          // regime codes
}

impl MockStore {
    fn report_count(&self) -> usize {
        self.detections.lock().unwrap().len()
    }
    fn detections(&self, dedup_key: &str) -> u32 {
        *self.detections.lock().unwrap().get(dedup_key).unwrap()
    }
    fn is_frozen(&self, regime_code: &str) -> bool {
        self.frozen.lock().unwrap().contains(regime_code)
    }
}

#[async_trait]
impl WatchStore for MockStore {
    async fn load_state(&self, regime_code: &str) -> anyhow::Result<Option<WatchState>> {
        Ok(self.states.lock().unwrap().get(regime_code).cloned())
    }
    async fn save_state(&self, state: &WatchState) -> anyhow::Result<()> {
        self.states
            .lock()
            .unwrap()
            .insert(state.regime_code.clone(), state.clone());
        Ok(())
    }
    async fn file_drift(&self, report: &DriftReport) -> anyhow::Result<DriftOutcome> {
        let mut detections = self.detections.lock().unwrap();
        if let Some(count) = detections.get_mut(&report.dedup_key) {
            *count += 1;
            return Ok(DriftOutcome::Redetected);
        }
        if report.freeze {
            self.frozen
                .lock()
                .unwrap()
                .insert(report.regime_code.clone());
        }
        detections.insert(report.dedup_key.clone(), 1);
        Ok(DriftOutcome::Filed)
    }
}

// --------------------------------------------------------------------------
// Fixtures: bodies + a target.
// --------------------------------------------------------------------------

const MARKER: &str = "Financial Disclosure";

/// A listing body with `rows` filing links in a fixed structure.
fn listing(rows: usize) -> String {
    let mut body = String::from(
        "<html><head><title>Financial Disclosure</title></head><body>\
         <table class=\"filings\">",
    );
    for i in 0..rows {
        write!(
            body,
            "<tr class=\"filing-row\"><td><a href=\"/ptr/{i}\">view</a></td></tr>"
        )
        .unwrap();
    }
    body.push_str("</table></body></html>");
    body
}

/// Same data, RESTRUCTURED markup (renamed containers/selectors) — a layout shift.
fn restructured(rows: usize) -> String {
    let mut body = String::from(
        "<html><head><title>Financial Disclosure</title></head><body>\
         <section class=\"results-grid\"><ul>",
    );
    for i in 0..rows {
        write!(
            body,
            "<li class=\"result-card\"><a href=\"/ptr/{i}\">view</a></li>"
        )
        .unwrap();
    }
    body.push_str("</ul></section></body></html>");
    body
}

fn target() -> WatchTarget {
    WatchTarget {
        regime_code: "us_house".to_owned(),
        url: "https://src.test/list".to_owned(),
        markers: vec![MARKER.to_owned()],
        // Count filings by their LINK, which survives a layout restructure — so
        // the count signal stays honest even when the row markup changes.
        row_marker: "/ptr/".to_owned(),
    }
}

fn ok_200(body: String) -> ProbeResult {
    ProbeResult {
        status: 200,
        etag: None,
        last_modified: None,
        body,
    }
}

// --------------------------------------------------------------------------
// Required assertions.
// --------------------------------------------------------------------------

/// Layout-hash delta detected -> a `layout_shift` drift is filed at the
/// top-of-ranking severity.
#[tokio::test]
async fn sentinel_layout_shift_files_ranked_drift() {
    let probe = ScriptedProbe::default();
    // pass 1 baselines the healthy structure; pass 2 sees restructured markup.
    probe.script(
        &target().url,
        vec![ok_200(listing(5)), ok_200(restructured(5))],
    );
    let store = MockStore::default();
    let targets = vec![target()];

    // Pass 1: baseline, nothing drifts.
    let first = watch_pass(&probe, &store, &targets).await.unwrap();
    assert_eq!(first.filed, 0, "first pass only establishes the baseline");
    assert_eq!(store.report_count(), 0);

    // Pass 2: the structural skeleton changed -> layout_shift, ranked worst.
    let second = watch_pass(&probe, &store, &targets).await.unwrap();
    assert_eq!(second.filed, 1);
    assert_eq!(second.reports.len(), 1);
    let drift = &second.reports[0];
    assert_eq!(drift.kind, DriftKind::LayoutShift);
    assert!((drift.priority_score - DriftKind::LayoutShift.priority_score()).abs() < f32::EPSILON);
    assert!(drift.priority_score >= DriftKind::CountZero.priority_score());
    assert!(
        drift.freeze,
        "a layout shift freezes publication (design §5.6)"
    );
}

/// A discoverable count that falls to zero is a high-priority, freeze-capable drift.
#[tokio::test]
async fn sentinel_count_to_zero_freezes_publication() {
    let probe = ScriptedProbe::default();
    probe.script(&target().url, vec![ok_200(listing(7)), ok_200(listing(0))]);
    let store = MockStore::default();
    let targets = vec![target()];

    watch_pass(&probe, &store, &targets).await.unwrap(); // baseline: 7 filings
    let pass = watch_pass(&probe, &store, &targets).await.unwrap(); // now 0

    assert_eq!(pass.filed, 1);
    let drift = &pass.reports[0];
    assert_eq!(drift.kind, DriftKind::CountZero);
    assert!(
        drift.freeze,
        "zero discoverable filings freezes publication"
    );
    assert!(store.is_frozen("us_house"), "freeze flag set on the source");
    assert!(
        drift.priority_score >= DriftKind::StatusError.priority_score(),
        "count-to-zero outranks a mere status error"
    );
}

/// The same drift across consecutive passes files ONE report; re-detection just
/// bumps the existing one (dedup).
#[tokio::test]
async fn sentinel_dedups_repeat_drift() {
    let probe = ScriptedProbe::default();
    // baseline, then the SAME shifted layout twice more.
    probe.script(
        &target().url,
        vec![
            ok_200(listing(5)),
            ok_200(restructured(5)),
            ok_200(restructured(5)),
        ],
    );
    let store = MockStore::default();
    let targets = vec![target()];

    watch_pass(&probe, &store, &targets).await.unwrap(); // baseline
    let second = watch_pass(&probe, &store, &targets).await.unwrap(); // files
    let third = watch_pass(&probe, &store, &targets).await.unwrap(); // re-detects

    assert_eq!(second.filed, 1);
    assert_eq!(third.filed, 0, "no duplicate report on re-detection");
    assert_eq!(third.redetected, 1);
    assert_eq!(store.report_count(), 1, "exactly one drift report exists");
    let dedup_key = &second.reports[0].dedup_key;
    assert_eq!(store.detections(dedup_key), 2, "re-detection bumps count");
}

/// Ranking is deterministic across a mixed batch: severity desc, then a stable
/// tie-break so equal-severity drifts never reorder run-to-run.
#[test]
fn sentinel_ranking_is_deterministic() {
    let mk = |code: &str, kind: DriftKind| DriftReport {
        regime_code: code.to_owned(),
        kind,
        priority_score: kind.priority_score(),
        freeze: kind.freezes_publication(),
        dedup_key: format!("{code}:{}", kind.as_str()),
        detail: serde_json::json!({}),
    };
    let mut batch = vec![
        mk("australia_register", DriftKind::CountDelta),
        mk("us_senate", DriftKind::CountZero),
        mk("canada_ciec", DriftKind::LayoutShift),
        mk("uk_commons_register", DriftKind::StatusError),
        // equal severity to the canada one -> tie broken by regime_code.
        mk("us_house", DriftKind::LayoutShift),
    ];
    rank(&mut batch);
    let order = |batch: &[DriftReport]| -> Vec<(String, DriftKind)> {
        batch
            .iter()
            .map(|r| (r.regime_code.clone(), r.kind))
            .collect()
    };
    assert_eq!(
        order(&batch),
        vec![
            ("canada_ciec".to_owned(), DriftKind::LayoutShift), // 100, tie -> code asc
            ("us_house".to_owned(), DriftKind::LayoutShift),    // 100
            ("us_senate".to_owned(), DriftKind::CountZero),     // 90
            ("uk_commons_register".to_owned(), DriftKind::StatusError), // 70
            ("australia_register".to_owned(), DriftKind::CountDelta), // 30
        ]
    );
    // Idempotent: ranking an already-ranked batch is a no-op.
    let before = order(&batch);
    rank(&mut batch);
    assert_eq!(before, order(&batch));
}

/// A clean source (unchanged listing) files nothing and keeps its baseline.
#[tokio::test]
async fn sentinel_clean_source_files_no_drift() {
    let probe = ScriptedProbe::default();
    probe.script(&target().url, vec![ok_200(listing(5)), ok_200(listing(6))]);
    let store = MockStore::default();
    let targets = vec![target()];

    watch_pass(&probe, &store, &targets).await.unwrap(); // baseline: 5
    // Count GREW to 6 (healthy weekly growth) with the same structure.
    let pass = watch_pass(&probe, &store, &targets).await.unwrap();

    assert_eq!(pass.filed, 0, "same structure + growing count is healthy");
    assert_eq!(pass.reports.len(), 0);
    assert_eq!(store.report_count(), 0);
    assert!(!store.is_frozen("us_house"));
}

/// A 304 Not Modified is not a drift, and it preserves the layout/count baseline
/// (conditional GETs stay cheap for the source, invariant 10).
#[tokio::test]
async fn sentinel_not_modified_preserves_baseline() {
    let probe = ScriptedProbe::default();
    let not_modified = ProbeResult {
        status: 304,
        etag: Some("\"abc\"".to_owned()),
        last_modified: None,
        body: String::new(),
    };
    probe.script(&target().url, vec![ok_200(listing(5)), not_modified]);
    let store = MockStore::default();
    let targets = vec![target()];

    watch_pass(&probe, &store, &targets).await.unwrap();
    let baseline = store.load_state("us_house").await.unwrap().unwrap();
    let pass = watch_pass(&probe, &store, &targets).await.unwrap();

    assert_eq!(pass.filed, 0, "304 is not a drift");
    let after = store.load_state("us_house").await.unwrap().unwrap();
    assert_eq!(
        after.last_layout_hash, baseline.last_layout_hash,
        "304 preserves the layout baseline"
    );
    assert_eq!(after.last_count, baseline.last_count);
}

/// A transport error is fail-closed: uncertain never silently passes.
#[tokio::test]
async fn sentinel_transport_error_is_flagged() {
    let probe = ScriptedProbe::default();
    probe.script_raw(&target().url, vec![Err("connection reset".to_owned())]);
    let store = MockStore::default();
    let targets = vec![target()];

    let pass = watch_pass(&probe, &store, &targets).await.unwrap();
    assert_eq!(pass.filed, 1);
    assert_eq!(pass.reports[0].kind, DriftKind::ProbeError);
}

/// A 5xx status is flagged even without a prior baseline (fail closed).
#[test]
fn sentinel_classify_status_error_on_first_run() {
    let obs = observe(
        &ProbeResult {
            status: 503,
            etag: None,
            last_modified: None,
            body: String::new(),
        },
        &target(),
    );
    let drift = classify("us_house", None, &obs).unwrap();
    assert_eq!(drift.kind, DriftKind::StatusError);
    assert!(!drift.freeze, "a transient status error does not freeze");
}

// --------------------------------------------------------------------------
// DB-touching: the real PgWatchStore (dedup + freeze + review_task).
// --------------------------------------------------------------------------

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn sentinel_pg_store_dedups_and_freezes(pool: sqlx::PgPool) {
    use worker::sentinel::PgWatchStore;
    govfolio_core::db::migrate(&pool).await.unwrap();
    let store = PgWatchStore::new(pool.clone());

    // Baseline must exist before file_drift so the freeze flag can land on it.
    store
        .save_state(&WatchState {
            regime_code: "us_house".to_owned(),
            last_status: Some(200),
            last_layout_hash: Some("hash-a".to_owned()),
            last_count: Some(9),
            last_etag: None,
            last_modified: None,
        })
        .await
        .unwrap();

    let obs = observe(&ok_200(listing(0)), &target());
    let baseline = store.load_state("us_house").await.unwrap().unwrap();
    let drift = classify("us_house", Some(&baseline), &obs).unwrap();
    assert_eq!(drift.kind, DriftKind::CountZero);

    // First filing: inserts, freezes, opens a review_task.
    assert!(matches!(
        store.file_drift(&drift).await.unwrap(),
        DriftOutcome::Filed
    ));
    // Second filing of the same drift: dedups (no new row).
    assert!(matches!(
        store.file_drift(&drift).await.unwrap(),
        DriftOutcome::Redetected
    ));

    let (count, detections): (i64, i32) = sqlx::query_as(
        "select count(*)::int8, max(detections) from drift_report where regime_code = $1",
    )
    .bind("us_house")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 1, "one report despite two filings");
    assert_eq!(detections, 2, "re-detection bumped the existing report");

    let frozen: bool =
        sqlx::query_scalar("select frozen from sentinel_watch where regime_code = $1")
            .bind("us_house")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(frozen, "count-to-zero froze publication (design §5.6)");

    let review_tasks: i64 = sqlx::query_scalar(
        "select count(*) from review_task where target_id = $1 and status = 'open'",
    )
    .bind("us_house")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(review_tasks, 1, "exactly one review_task, not duplicated");
}
