//! Sentinel WATCH (goal 017, design §5.6 fail-closed + §5.8 continuous drift
//! defense): a weekly job that probes every live source's discovery page and
//! escalates format/coverage changes BEFORE they silently corrupt Gold.
//!
//! Four checks per source, each fail-closed (uncertain -> anomaly, never a
//! silent pass): HTTP status (polite conditional GET), a structural
//! layout-hash of the listing markup, the discoverable filing count, and the
//! presence of regime markers. A shift is [`classify`]d into a [`DriftKind`],
//! ranked by severity ([`rank`]), deduped by a stable key, and auto-filed as a
//! `drift_report` — and, for the §5.6 kinds (a layout shift or a discoverable
//! count that falls to zero), it freezes that regime's publication and opens a
//! `review_task` (the orchestrator's work item).
//!
//! Both seams are traits so the engine is exercised offline: [`SourceProbe`]
//! over the HTTP fetch and [`WatchStore`] over Postgres. [`HttpProbe`] +
//! [`PgWatchStore`] are the live wiring; the `sentinel` bin runs one pass.

use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use sha2::{Digest as _, Sha256};
use sqlx::PgPool;

use pipeline::adapter::{PoliteClient, PolitenessCfg};

/// One polite conditional probe of a source's discovery page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    /// HTTP status (304 = not modified; non-2xx is surfaced, not an error).
    pub status: u16,
    /// `ETag` validator, when the source sends one.
    pub etag: Option<String>,
    /// `Last-Modified` validator, when the source sends one.
    pub last_modified: Option<String>,
    /// Response body (empty for 304 / bodyless statuses).
    pub body: String,
}

/// The fetch seam: a polite conditional GET. Mocked offline in tests; the live
/// implementation is [`HttpProbe`].
#[async_trait]
pub trait SourceProbe: Send + Sync {
    /// Fetches `url`, passing conditional validators when the baseline has them
    /// (invariant 10: 304s stay cheap for the source).
    ///
    /// # Errors
    /// Transport failure (a non-2xx status is a normal [`ProbeResult`], not an
    /// error — the caller classifies it).
    async fn fetch(
        &self,
        url: &str,
        if_none_match: Option<&str>,
        if_modified_since: Option<&str>,
    ) -> anyhow::Result<ProbeResult>;
}

/// Live probe: a politeness-wrapped [`PoliteClient`] (concurrency 1, min
/// interval, identified UA — invariant 10). One conservative throttle covers
/// every source; the sentinel runs sequentially, so this is never less polite
/// than per-source spacing.
#[derive(Debug)]
pub struct HttpProbe {
    client: PoliteClient,
}

impl HttpProbe {
    /// Builds the live probe with a reachable contact in the User-Agent.
    ///
    /// # Errors
    /// HTTP client construction failure.
    pub fn new(contact: impl Into<String>) -> anyhow::Result<Self> {
        let cfg = PolitenessCfg::new(Duration::from_secs(2), contact);
        Ok(Self {
            client: PoliteClient::new(&cfg)?,
        })
    }
}

#[async_trait]
impl SourceProbe for HttpProbe {
    async fn fetch(
        &self,
        url: &str,
        if_none_match: Option<&str>,
        if_modified_since: Option<&str>,
    ) -> anyhow::Result<ProbeResult> {
        let response = self
            .client
            .get_conditional(url, if_none_match, if_modified_since)
            .await?;
        let status = response.status().as_u16();
        let etag = header(&response, reqwest::header::ETAG);
        let last_modified = header(&response, reqwest::header::LAST_MODIFIED);
        let body = response.text().await.context("reading probe body")?;
        Ok(ProbeResult {
            status,
            etag,
            last_modified,
            body,
        })
    }
}

fn header(response: &reqwest::Response, name: reqwest::header::HeaderName) -> Option<String> {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

/// What the sentinel watches for one regime: its discovery page plus the
/// per-source signals a healthy page must show.
#[derive(Debug, Clone)]
pub struct WatchTarget {
    /// Stable regime code (`RegimeRef.code`) — the cross-system watch identity.
    pub regime_code: String,
    /// Discovery / listing page to probe.
    pub url: String,
    /// Substrings a healthy page must contain; any absent => regime change.
    pub markers: Vec<String>,
    /// Substring whose occurrence count approximates the discoverable filings.
    pub row_marker: String,
}

/// The built E1 regimes to watch (design §5.8: for now the live adapters).
/// Discovery URLs and markers come from each adapter's source + regime doc.
#[must_use]
pub fn live_targets() -> Vec<WatchTarget> {
    let t = |code: &str, url: &str, markers: &[&str], row: &str| WatchTarget {
        regime_code: code.to_owned(),
        url: url.to_owned(),
        markers: markers.iter().map(|m| (*m).to_owned()).collect(),
        row_marker: row.to_owned(),
    };
    vec![
        t(
            "us_house",
            "https://disclosures-clerk.house.gov/FinancialDisclosure",
            &["Financial Disclosure"],
            "public_disc",
        ),
        t(
            "us_senate",
            "https://efdsearch.senate.gov/search/home/",
            &["Financial Disclosure"],
            "/search/view/",
        ),
        t(
            "uk_commons_register",
            "https://interests-api.parliament.uk/api/v1/Interests",
            &["\"items\""],
            "\"id\":",
        ),
        t(
            "canada_ciec",
            "https://ciec-ccie.parl.gc.ca/en/public-registry/cards",
            &["public-registry"],
            "declarationId",
        ),
        t(
            "australia_register",
            "https://www.aph.gov.au/Senators_and_Members/Members/Register",
            &["Register"],
            "/Register/",
        ),
        t(
            "eu_parliament_dpi",
            "https://www.europarl.europa.eu/meps/en/full-list/all",
            &["MEP"],
            "/meps/en/",
        ),
        t(
            "fr_hatvp_dia",
            "https://www.hatvp.fr/open-data/",
            &["open-data"],
            "declaration",
        ),
        t(
            "de_bundestag",
            "https://www.bundestag.de/abgeordnete/biografien",
            &["Bundestag"],
            "/abgeordnete/",
        ),
        t(
            // AUTHORITY.md tos_and_politeness: dadosabertos.tse.jus.br's
            // robots.txt disallows /api/ — this is the human-facing CKAN
            // dataset page (/dataset/, not /api/), not covered by that
            // Disallow, and lists the real resource files by name.
            "br",
            "https://dadosabertos.tse.jus.br/dataset/candidatos-2022",
            &["consulta_cand_2022", "bem_candidato_2022"],
            "consulta_cand_2022",
        ),
    ]
}

/// What one probe told us about a source, after parsing the body against a
/// target. The classifier diffs this against the stored baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// HTTP status (0 when the transport itself failed).
    pub status: u16,
    /// `ETag` validator, if any.
    pub etag: Option<String>,
    /// `Last-Modified` validator, if any.
    pub last_modified: Option<String>,
    /// Structural layout hash (empty unless a 2xx body was read).
    pub layout_hash: String,
    /// Discoverable filing count (`-1` when not counted: 304, error, non-2xx).
    pub count: i64,
    /// Whether every regime marker was present (only meaningful for 2xx).
    pub markers_present: bool,
    /// Transport error message, when the fetch failed entirely (fail closed).
    pub transport_error: Option<String>,
}

impl Observation {
    /// A fetch that never completed — an anomaly by itself (fail closed).
    #[must_use]
    pub fn errored(message: impl Into<String>) -> Self {
        Self {
            status: 0,
            etag: None,
            last_modified: None,
            layout_hash: String::new(),
            count: -1,
            markers_present: true,
            transport_error: Some(message.into()),
        }
    }
}

/// Parses a probe result against its target into an [`Observation`].
#[must_use]
pub fn observe(probe: &ProbeResult, target: &WatchTarget) -> Observation {
    let counted = (200..300).contains(&probe.status);
    Observation {
        status: probe.status,
        etag: probe.etag.clone(),
        last_modified: probe.last_modified.clone(),
        layout_hash: if counted {
            layout_hash(&probe.body)
        } else {
            String::new()
        },
        count: if counted {
            i64::try_from(probe.body.matches(&target.row_marker).count()).unwrap_or(i64::MAX)
        } else {
            -1
        },
        markers_present: if counted {
            target.markers.iter().all(|m| probe.body.contains(m))
        } else {
            true
        },
        transport_error: None,
    }
}

/// sha256 of the listing's structural skeleton — a stable layout fingerprint.
#[must_use]
pub fn layout_hash(html: &str) -> String {
    let skeleton = structural_skeleton(html);
    hex::encode(Sha256::digest(skeleton.as_bytes()))
}

/// The DISTINCT set of opening-tag names and `class` tokens in `html`, sorted —
/// text and non-class attribute values discarded. Deliberately a set, not a
/// sequence: restructured markup (renamed containers, changed selectors) adds or
/// drops tokens and shifts the hash, but MORE data rows in the SAME structure
/// reuse the same tokens and do not — so layout drift is separated from the
/// volume drift that the filing count tracks.
fn structural_skeleton(html: &str) -> String {
    let mut tokens = std::collections::BTreeSet::new();
    let bytes = html.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'<' && bytes[i + 1].is_ascii_alphabetic() {
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'-') {
                j += 1;
            }
            tokens.insert(format!("t:{}", &html[start..j]));
            i = j;
        } else {
            i += 1;
        }
    }
    let mut rest = html;
    while let Some(pos) = rest.find("class=\"") {
        let after = &rest[pos + "class=\"".len()..];
        let Some(end) = after.find('"') else { break };
        for token in after[..end].split_whitespace() {
            tokens.insert(format!("c:{token}"));
        }
        rest = &after[end + 1..];
    }
    tokens.into_iter().collect::<Vec<_>>().join(";")
}

/// A classified source anomaly. Ordered by descending severity (design §5.6:
/// a silent format shift is worse than a loud outage — garbage reaching Gold
/// undetected is the failure the sentinel exists to prevent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftKind {
    /// The listing markup restructured — selectors will silently mis-parse.
    LayoutShift,
    /// The discoverable count fell to zero — the adapter would yield nothing.
    CountZero,
    /// Expected regime markers vanished — the page is no longer what we parse.
    RegimeChange,
    /// A non-2xx HTTP status.
    StatusError,
    /// The transport failed entirely (uncertain -> fail closed).
    ProbeError,
    /// The discoverable count shrank (filings disappearing, but not to zero).
    CountDelta,
}

impl DriftKind {
    /// Severity used to rank reports (orchestrator picks the worst first,
    /// design §5.8). Layout shift > count-to-zero > regime change > status
    /// error > probe error > minor count delta.
    #[must_use]
    pub const fn priority_score(self) -> f32 {
        match self {
            Self::LayoutShift => 100.0,
            Self::CountZero => 90.0,
            Self::RegimeChange => 80.0,
            Self::StatusError => 70.0,
            Self::ProbeError => 60.0,
            Self::CountDelta => 30.0,
        }
    }

    /// Whether this drift freezes the regime's publication (design §5.6: a
    /// layout shift, a count that falls to zero, or vanished markers each mean
    /// the adapter's output can no longer be trusted).
    #[must_use]
    pub const fn freezes_publication(self) -> bool {
        matches!(
            self,
            Self::LayoutShift | Self::CountZero | Self::RegimeChange
        )
    }

    /// Stable code stored in `drift_report.drift_kind` and the dedup key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LayoutShift => "layout_shift",
            Self::CountZero => "count_zero",
            Self::RegimeChange => "regime_change",
            Self::StatusError => "status_error",
            Self::ProbeError => "probe_error",
            Self::CountDelta => "count_delta",
        }
    }
}

/// The per-source baseline the next pass diffs against (`sentinel_watch`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchState {
    /// Regime code this baseline belongs to.
    pub regime_code: String,
    /// Last HTTP status (`None` when the last attempt failed transport).
    pub last_status: Option<i32>,
    /// Last structural layout hash.
    pub last_layout_hash: Option<String>,
    /// Last discoverable count.
    pub last_count: Option<i64>,
    /// Last `ETag` validator.
    pub last_etag: Option<String>,
    /// Last `Last-Modified` validator.
    pub last_modified: Option<String>,
}

/// A ranked, dedupable drift report ready to file.
#[derive(Debug, Clone, PartialEq)]
pub struct DriftReport {
    /// Regime code the drift is against.
    pub regime_code: String,
    /// The classified anomaly.
    pub kind: DriftKind,
    /// Severity (`kind.priority_score()`), copied for storage + ranking.
    pub priority_score: f32,
    /// Whether filing this freezes publication.
    pub freeze: bool,
    /// `regime_code:kind:signature` — at most one OPEN report per key.
    pub dedup_key: String,
    /// Observed-vs-expected evidence for the report body.
    pub detail: serde_json::Value,
}

/// Classifies a source's [`Observation`] against its baseline. Returns `None`
/// when the source is healthy (or a 304, or a first-run baseline). Fail-closed:
/// transport/status/marker anomalies fire even without a baseline.
#[must_use]
pub fn classify(
    regime_code: &str,
    prior: Option<&WatchState>,
    obs: &Observation,
) -> Option<DriftReport> {
    // Transport failure: uncertain never passes silently.
    if let Some(error) = &obs.transport_error {
        return Some(report(
            regime_code,
            DriftKind::ProbeError,
            "transport",
            serde_json::json!({ "error": error }),
        ));
    }
    // The source explicitly said "unchanged".
    if obs.status == 304 {
        return None;
    }
    // A non-2xx status is a drift even on the first run.
    if !(200..300).contains(&obs.status) {
        return Some(report(
            regime_code,
            DriftKind::StatusError,
            &obs.status.to_string(),
            serde_json::json!({ "status": obs.status }),
        ));
    }
    // The page loaded but is not the page we parse.
    if !obs.markers_present {
        return Some(report(
            regime_code,
            DriftKind::RegimeChange,
            "markers",
            serde_json::json!({ "reason": "expected regime markers absent" }),
        ));
    }
    // Layout / count deltas need a baseline to diff against.
    let prior = prior?;
    // Count-to-zero is checked BEFORE the layout hash: an emptied listing
    // legitimately drops its row markup, which would otherwise read as a
    // layout shift. "The adapter would yield nothing" is the clearer signal;
    // both freeze publication, so §5.6 holds either way.
    if let Some(prior_count) = prior.last_count
        && obs.count == 0
        && prior_count > 0
    {
        return Some(report(
            regime_code,
            DriftKind::CountZero,
            "zero",
            serde_json::json!({ "prior_count": prior_count, "observed_count": 0 }),
        ));
    }
    if let Some(prior_hash) = &prior.last_layout_hash
        && prior_hash != &obs.layout_hash
    {
        return Some(report(
            regime_code,
            DriftKind::LayoutShift,
            &obs.layout_hash,
            serde_json::json!({
                "prior_layout_hash": prior_hash,
                "observed_layout_hash": obs.layout_hash,
            }),
        ));
    }
    if let Some(prior_count) = prior.last_count
        && obs.count >= 0
        && obs.count < prior_count
    {
        return Some(report(
            regime_code,
            DriftKind::CountDelta,
            "shrink",
            serde_json::json!({
                "prior_count": prior_count,
                "observed_count": obs.count,
            }),
        ));
    }
    None
}

fn report(
    regime_code: &str,
    kind: DriftKind,
    signature: &str,
    detail: serde_json::Value,
) -> DriftReport {
    DriftReport {
        regime_code: regime_code.to_owned(),
        kind,
        priority_score: kind.priority_score(),
        freeze: kind.freezes_publication(),
        dedup_key: format!("{regime_code}:{}:{signature}", kind.as_str()),
        detail,
    }
}

/// Ranks reports worst-first: severity descending, then a stable
/// (`regime_code`, `dedup_key`) tie-break so equal-severity drifts never
/// reorder between runs.
pub fn rank(reports: &mut [DriftReport]) {
    reports.sort_by(|a, b| {
        b.priority_score
            .total_cmp(&a.priority_score)
            .then_with(|| a.regime_code.cmp(&b.regime_code))
            .then_with(|| a.dedup_key.cmp(&b.dedup_key))
    });
}

/// Whether filing produced a fresh report or bumped an existing open one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftOutcome {
    /// A new report row (and, when freezing, a `review_task`).
    Filed,
    /// The same open drift was seen again — bumped, not duplicated.
    Redetected,
}

/// Whether a healthy pass cleared a standing freeze (design §5.6 recovery).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoverOutcome {
    /// The regime was frozen and is now cleared — its freezing drift report(s)
    /// and linked `review_task`(s) resolved, publication re-enabled.
    Recovered,
    /// The regime was already healthy — nothing to clear.
    WasHealthy,
}

/// The persistence seam: baselines + deduped drift reports. Mocked offline in
/// tests; the live implementation is [`PgWatchStore`].
#[async_trait]
pub trait WatchStore: Send + Sync {
    /// Loads a regime's baseline, if one exists.
    ///
    /// # Errors
    /// Storage failure.
    async fn load_state(&self, regime_code: &str) -> anyhow::Result<Option<WatchState>>;

    /// Upserts a regime's baseline (advances it every pass).
    ///
    /// # Errors
    /// Storage failure.
    async fn save_state(&self, state: &WatchState) -> anyhow::Result<()>;

    /// Files a drift report, deduping on its key: a fresh key inserts (and, when
    /// freezing, sets the freeze flag + opens a `review_task`); a repeat bumps
    /// the existing open report.
    ///
    /// # Errors
    /// Storage failure.
    async fn file_drift(&self, report: &DriftReport) -> anyhow::Result<DriftOutcome>;

    /// Clears a standing freeze after a healthy pass (design §5.6 recovery): if
    /// the regime is frozen, unfreeze it and resolve its open freezing drift
    /// report(s) + linked `review_task`(s), re-enabling publication. A no-op
    /// when the regime is not frozen.
    ///
    /// # Errors
    /// Storage failure.
    async fn recover(&self, regime_code: &str) -> anyhow::Result<RecoverOutcome>;
}

/// What one WATCH pass did.
#[derive(Debug, Default, Clone)]
pub struct WatchSummary {
    /// Sources probed.
    pub checked: usize,
    /// Fresh drift reports filed.
    pub filed: usize,
    /// Drifts re-detected (deduped, not re-filed).
    pub redetected: usize,
    /// Frozen regimes cleared by a healthy pass this run (recovery, §5.6).
    pub recovered: usize,
    /// Every drift observed this pass, ranked worst-first.
    pub reports: Vec<DriftReport>,
}

/// Runs one WATCH pass over `targets`, sequentially (concurrency 1 politeness).
/// Per source: load baseline -> polite conditional probe -> advance baseline ->
/// classify -> file any drift (deduped). Reports are ranked before returning.
///
/// # Errors
/// A storage failure aborts the pass; a per-source transport failure does not
/// (it is classified as a `probe_error` drift — fail closed).
pub async fn watch_pass(
    probe: &dyn SourceProbe,
    store: &dyn WatchStore,
    targets: &[WatchTarget],
) -> anyhow::Result<WatchSummary> {
    let mut summary = WatchSummary::default();
    let mut reports = Vec::new();
    for target in targets {
        let prior = store.load_state(&target.regime_code).await?;
        let observation = match probe
            .fetch(
                &target.url,
                prior.as_ref().and_then(|p| p.last_etag.as_deref()),
                prior.as_ref().and_then(|p| p.last_modified.as_deref()),
            )
            .await
        {
            Ok(result) => observe(&result, target),
            Err(error) => Observation::errored(format!("{error:#}")),
        };
        let drift = classify(&target.regime_code, prior.as_ref(), &observation);
        // Persist the baseline BEFORE filing so the freeze flag has a row to
        // land on. A drifted pass keeps the prior known-good layout/count (so
        // the SAME drift re-detects next pass and the store dedups it); only a
        // healthy pass advances the baseline.
        let next = advance_state(
            &target.regime_code,
            prior.as_ref(),
            &observation,
            drift.is_none(),
        );
        store.save_state(&next).await?;
        if let Some(drift) = drift {
            match store.file_drift(&drift).await? {
                DriftOutcome::Filed => summary.filed += 1,
                DriftOutcome::Redetected => summary.redetected += 1,
            }
            reports.push(drift);
        } else if let RecoverOutcome::Recovered = store.recover(&target.regime_code).await? {
            // Healthy pass on a regime that was frozen: it recovered — clear the
            // freeze and resolve the freezing drift, re-enabling publication.
            summary.recovered += 1;
        }
        summary.checked += 1;
    }
    rank(&mut reports);
    summary.reports = reports;
    Ok(summary)
}

/// The next baseline after an observation. Only a `healthy` (no-drift) 2xx
/// advances the known-good layout/count — a drifted pass keeps the last-good
/// baseline so the same drift keeps being detected (and deduped) until it is
/// resolved. A 304 refreshes validators but keeps layout/count. `last_status`
/// always reflects the latest attempt for observability.
fn advance_state(
    regime_code: &str,
    prior: Option<&WatchState>,
    obs: &Observation,
    healthy: bool,
) -> WatchState {
    let mut state = prior.cloned().unwrap_or(WatchState {
        regime_code: regime_code.to_owned(),
        last_status: None,
        last_layout_hash: None,
        last_count: None,
        last_etag: None,
        last_modified: None,
    });
    // `prior` (when present) already carries this regime_code; only the
    // freshly-defaulted state needs it, and the default above set it.
    state.last_status = if obs.transport_error.is_some() {
        None
    } else {
        Some(i32::from(obs.status))
    };
    if healthy && (200..300).contains(&obs.status) {
        state.last_layout_hash = Some(obs.layout_hash.clone());
        state.last_count = Some(obs.count);
        state.last_etag.clone_from(&obs.etag);
        state.last_modified.clone_from(&obs.last_modified);
    } else if obs.status == 304 {
        // Conditional GET confirmed unchanged: keep the baseline, refresh only
        // the validators the source re-sent.
        if obs.etag.is_some() {
            state.last_etag.clone_from(&obs.etag);
        }
        if obs.last_modified.is_some() {
            state.last_modified.clone_from(&obs.last_modified);
        }
    }
    state
}

/// Postgres-backed [`WatchStore`]: `sentinel_watch` baselines + `drift_report`
/// rows deduped by a partial-unique index over open reports (migration 0008).
#[derive(Debug, Clone)]
pub struct PgWatchStore {
    pool: PgPool,
}

impl PgWatchStore {
    /// Wraps a connection pool.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl WatchStore for PgWatchStore {
    async fn load_state(&self, regime_code: &str) -> anyhow::Result<Option<WatchState>> {
        let row: Option<(
            String,
            Option<i32>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(
            "select regime_code, last_status, last_layout_hash, last_count, \
                 last_etag, last_modified from sentinel_watch where regime_code = $1",
        )
        .bind(regime_code)
        .fetch_optional(&self.pool)
        .await
        .with_context(|| format!("loading sentinel_watch for {regime_code}"))?;
        Ok(row.map(
            |(regime_code, last_status, last_layout_hash, last_count, last_etag, last_modified)| {
                WatchState {
                    regime_code,
                    last_status,
                    last_layout_hash,
                    last_count,
                    last_etag,
                    last_modified,
                }
            },
        ))
    }

    async fn save_state(&self, state: &WatchState) -> anyhow::Result<()> {
        sqlx::query(
            "insert into sentinel_watch \
               (regime_code, last_status, last_layout_hash, last_count, \
                last_etag, last_modified, last_checked_at) \
             values ($1, $2, $3, $4, $5, $6, now()) \
             on conflict (regime_code) do update set \
               last_status = excluded.last_status, \
               last_layout_hash = excluded.last_layout_hash, \
               last_count = excluded.last_count, \
               last_etag = excluded.last_etag, \
               last_modified = excluded.last_modified, \
               last_checked_at = now()",
        )
        .bind(&state.regime_code)
        .bind(state.last_status)
        .bind(&state.last_layout_hash)
        .bind(state.last_count)
        .bind(&state.last_etag)
        .bind(&state.last_modified)
        .execute(&self.pool)
        .await
        .with_context(|| format!("saving sentinel_watch for {}", state.regime_code))?;
        Ok(())
    }

    async fn file_drift(&self, report: &DriftReport) -> anyhow::Result<DriftOutcome> {
        let mut tx = self.pool.begin().await.context("opening drift txn")?;
        let id = ulid::Ulid::new().to_string();
        // `xmax = 0` is true only for the freshly inserted row, false for the
        // conflict-update path — that is how we tell Filed from Redetected.
        let (inserted,): (bool,) = sqlx::query_as(
            "insert into drift_report \
               (id, regime_code, drift_kind, priority_score, freezes_publication, \
                dedup_key, detail) \
             values ($1, $2, $3, $4, $5, $6, $7) \
             on conflict (dedup_key) where status = 'open' do update set \
               last_detected_at = now(), detections = drift_report.detections + 1 \
             returning (xmax = 0)",
        )
        .bind(&id)
        .bind(&report.regime_code)
        .bind(report.kind.as_str())
        .bind(report.priority_score)
        .bind(report.freeze)
        .bind(&report.dedup_key)
        .bind(&report.detail)
        .fetch_one(&mut *tx)
        .await
        .with_context(|| format!("filing drift {}", report.dedup_key))?;

        if !inserted {
            tx.commit().await.context("committing drift redetect")?;
            return Ok(DriftOutcome::Redetected);
        }

        if report.freeze {
            let review_task_id = ulid::Ulid::new().to_string();
            let reason = format!(
                "sentinel drift: {} on {} — publication frozen (design §5.6)",
                report.kind.as_str(),
                report.regime_code
            );
            sqlx::query(
                "insert into review_task \
                   (id, target_kind, target_id, reason, priority_score) \
                 values ($1, 'regime', $2, $3, $4)",
            )
            .bind(&review_task_id)
            .bind(&report.regime_code)
            .bind(&reason)
            .bind(report.priority_score)
            .execute(&mut *tx)
            .await
            .context("opening drift review_task")?;
            sqlx::query(
                "update sentinel_watch set frozen = true, frozen_kind = $2, \
                 frozen_at = now() where regime_code = $1",
            )
            .bind(&report.regime_code)
            .bind(report.kind.as_str())
            .execute(&mut *tx)
            .await
            .context("freezing publication")?;
            sqlx::query("update drift_report set review_task_id = $2 where id = $1")
                .bind(&id)
                .bind(&review_task_id)
                .execute(&mut *tx)
                .await
                .context("linking drift review_task")?;
        }
        tx.commit().await.context("committing drift filing")?;
        Ok(DriftOutcome::Filed)
    }

    async fn recover(&self, regime_code: &str) -> anyhow::Result<RecoverOutcome> {
        let mut tx = self.pool.begin().await.context("opening recover txn")?;
        // Only act if the regime is CURRENTLY frozen — a clean pass on a healthy
        // regime is the common case and must stay a cheap no-op.
        let unfroze = sqlx::query(
            "update sentinel_watch set frozen = false, frozen_kind = null, frozen_at = null \
             where regime_code = $1 and frozen = true",
        )
        .bind(regime_code)
        .execute(&mut *tx)
        .await
        .with_context(|| format!("unfreezing {regime_code}"))?
        .rows_affected();
        if unfroze == 0 {
            tx.commit().await.context("committing recover no-op")?;
            return Ok(RecoverOutcome::WasHealthy);
        }
        // Resolve the open freezing drift report(s) and collect their linked
        // review_task ids.
        let task_ids: Vec<Option<String>> = sqlx::query_scalar(
            "update drift_report set status = 'resolved', last_detected_at = now() \
             where regime_code = $1 and status = 'open' and freezes_publication = true \
             returning review_task_id",
        )
        .bind(regime_code)
        .fetch_all(&mut *tx)
        .await
        .with_context(|| format!("resolving freezing drift reports for {regime_code}"))?;
        // Resolve each linked review_task (still open).
        let recovered = serde_json::json!({ "verdict": "drift_recovered" });
        for task_id in task_ids.into_iter().flatten() {
            sqlx::query(
                "update review_task set status = 'resolved', resolved_at = now(), \
                 resolution = $2 where id = $1 and status = 'open'",
            )
            .bind(&task_id)
            .bind(&recovered)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("resolving recovered review_task {task_id}"))?;
        }
        tx.commit().await.context("committing recovery")?;
        Ok(RecoverOutcome::Recovered)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn target() -> WatchTarget {
        WatchTarget {
            regime_code: "us_house".to_owned(),
            url: "https://src.test/list".to_owned(),
            markers: vec!["Financial Disclosure".to_owned()],
            row_marker: "class=\"row\"".to_owned(),
        }
    }

    fn body(rows: usize, container: &str) -> String {
        let mut html = format!("<html><body>Financial Disclosure<{container}>");
        for _ in 0..rows {
            html.push_str("<tr class=\"row\"><a>x</a></tr>");
        }
        html.push_str("</");
        html.push_str(container);
        html.push_str("></body></html>");
        html
    }

    #[test]
    fn skeleton_stable_across_data_rows_but_shifts_on_restructure() {
        // Same structure, different row COUNTS -> identical layout hash.
        assert_eq!(
            layout_hash(&body(3, "table")),
            layout_hash(&body(9, "table"))
        );
        // Restructured container -> different hash.
        assert_ne!(
            layout_hash(&body(3, "table")),
            layout_hash(&body(3, "section"))
        );
    }

    #[test]
    fn skeleton_shifts_when_class_selectors_change() {
        assert_ne!(
            layout_hash("<div class=\"filings\"><a>x</a></div>"),
            layout_hash("<div class=\"results\"><a>x</a></div>")
        );
    }

    #[test]
    fn observe_counts_rows_and_finds_markers() {
        let probe = ProbeResult {
            status: 200,
            etag: None,
            last_modified: None,
            body: body(4, "table"),
        };
        let obs = observe(&probe, &target());
        assert_eq!(obs.count, 4);
        assert!(obs.markers_present);
    }

    #[test]
    fn classify_first_run_establishes_baseline_without_drift() {
        let obs = observe(
            &ProbeResult {
                status: 200,
                etag: None,
                last_modified: None,
                body: body(4, "table"),
            },
            &target(),
        );
        assert!(classify("us_house", None, &obs).is_none());
    }

    #[test]
    fn classify_flags_missing_markers_as_regime_change() {
        let obs = observe(
            &ProbeResult {
                status: 200,
                etag: None,
                last_modified: None,
                body: "<html><body>totally different page</body></html>".to_owned(),
            },
            &target(),
        );
        let drift = classify("us_house", None, &obs).unwrap();
        assert_eq!(drift.kind, DriftKind::RegimeChange);
        assert!(drift.freeze);
    }

    #[test]
    fn dedup_key_is_stable_and_kind_scoped() {
        let a = report(
            "us_house",
            DriftKind::CountZero,
            "zero",
            serde_json::json!({}),
        );
        let b = report(
            "us_house",
            DriftKind::CountZero,
            "zero",
            serde_json::json!({ "prior_count": 9 }),
        );
        assert_eq!(a.dedup_key, b.dedup_key, "dedup ignores the detail body");
        assert_eq!(a.dedup_key, "us_house:count_zero:zero");
    }

    #[test]
    fn priority_order_matches_documented_ranking() {
        assert!(DriftKind::LayoutShift.priority_score() > DriftKind::CountZero.priority_score());
        assert!(DriftKind::CountZero.priority_score() > DriftKind::StatusError.priority_score());
        assert!(DriftKind::StatusError.priority_score() > DriftKind::CountDelta.priority_score());
    }

    #[test]
    fn advance_state_304_preserves_layout_and_count() {
        let prior = WatchState {
            regime_code: "us_house".to_owned(),
            last_status: Some(200),
            last_layout_hash: Some("hash-a".to_owned()),
            last_count: Some(5),
            last_etag: Some("\"e\"".to_owned()),
            last_modified: None,
        };
        let obs = Observation {
            status: 304,
            etag: None,
            last_modified: None,
            layout_hash: String::new(),
            count: -1,
            markers_present: true,
            transport_error: None,
        };
        let next = advance_state("us_house", Some(&prior), &obs, true);
        assert_eq!(next.last_layout_hash.as_deref(), Some("hash-a"));
        assert_eq!(next.last_count, Some(5));
        assert_eq!(next.last_status, Some(304));
    }
}
