//! The adapter contract (design §5.1): one trait every jurisdiction implements,
//! plus the run context handed to every stage. [`RunCtx`] carries the Bronze
//! store (local-dir, sha256-addressed — invariant 2), an optional Postgres pool
//! (not needed for conformance), a clock, and a politeness-wrapped HTTP client
//! (invariant 10: per-source min-interval, concurrency 1 default, identified UA).

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Semaphore, SemaphorePermit};
use tokio::time::Instant;

use govfolio_core::domain::gold::GoldCandidate;

/// Binds an adapter to its `disclosure_regime` row by stable code (design §5.1).
/// The code doubles as the adapter's package and fixture-directory name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegimeRef {
    /// Stable regime code, e.g. `fixture_fake`, `us_house`.
    pub code: &'static str,
}

/// Politeness knobs (invariant 10). One config per source; [`RunCtx`] wraps
/// them into the shared HTTP client.
#[derive(Debug, Clone)]
pub struct PolitenessCfg {
    /// Minimum spacing between request starts against this source.
    pub min_interval: Duration,
    /// Maximum in-flight requests; stays at 1 unless the source documents otherwise.
    pub concurrency: usize,
    /// Contact address embedded in the identified User-Agent.
    pub contact: String,
}

impl PolitenessCfg {
    /// Conservative defaults: `concurrency = 1`.
    #[must_use]
    pub fn new(min_interval: Duration, contact: impl Into<String>) -> Self {
        Self {
            min_interval,
            concurrency: 1,
            contact: contact.into(),
        }
    }

    /// Identified User-Agent with a reachable contact (invariant 10).
    #[must_use]
    pub fn user_agent(&self) -> String {
        format!("govfolio-bot/0.1 (+https://govfolio.io; {})", self.contact)
    }
}

/// A new/changed filing found by `discover`; deduplicated at publish time by
/// `(regime_id, external_id)` (design §5.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilingRef {
    /// The source's own identifier for the filing.
    pub external_id: String,
    /// Where `fetch` retrieves the document (URL; a local path in fixtures).
    pub url: String,
}

/// A Bronze document pointer: content-addressed, immutable (invariant 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawDocRef {
    /// sha256 of the raw bytes as 64 lowercase hex chars — the Bronze address.
    pub sha256: String,
}

/// One Silver row: source-shaped payload plus extraction confidence
/// (design §5.1 — Silver keeps the source's own vocabulary, not the Gold contract).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StagingRow {
    /// Source-shaped JSON, exactly as the filing said it.
    pub payload: serde_json::Value,
    /// Extractor confidence in `[0, 1]`.
    pub confidence: f32,
}

/// Local-directory Bronze store: files addressed by sha256, written once and
/// never rewritten (raw is sacred — invariant 2). Object storage arrives later
/// behind this same shape.
#[derive(Debug)]
pub struct BronzeStore {
    root: PathBuf,
}

impl BronzeStore {
    /// Opens (creating if needed) a Bronze directory.
    ///
    /// # Errors
    /// I/O failure creating the directory.
    pub fn open(root: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(&root)
            .with_context(|| format!("creating bronze root {}", root.display()))?;
        Ok(Self { root })
    }

    /// Stores raw bytes content-addressed; an existing document is left
    /// untouched (Bronze is immutable).
    ///
    /// # Errors
    /// I/O failure writing the document.
    pub fn put(&self, bytes: &[u8]) -> anyhow::Result<RawDocRef> {
        use sha2::{Digest as _, Sha256};
        let sha256 = hex_lower(&Sha256::digest(bytes));
        let path = self.root.join(&sha256);
        if !path.exists() {
            // Write-once via temp + rename so a crash never leaves a torn doc.
            let tmp = self
                .root
                .join(format!("{sha256}.tmp-{}", std::process::id()));
            std::fs::write(&tmp, bytes)
                .with_context(|| format!("writing bronze temp {}", tmp.display()))?;
            if let Err(e) = std::fs::rename(&tmp, &path) {
                // A concurrent writer may have materialized the same document
                // first (rename onto an existing file fails on Windows) —
                // identical content, so losing that race is benign.
                let _ = std::fs::remove_file(&tmp);
                if !path.exists() {
                    return Err(e)
                        .with_context(|| format!("publishing bronze doc {}", path.display()));
                }
            }
        }
        Ok(RawDocRef { sha256 })
    }

    /// Reads a document back by its sha256 address.
    ///
    /// # Errors
    /// Unknown address or I/O failure.
    pub fn get(&self, doc: &RawDocRef) -> anyhow::Result<Vec<u8>> {
        let path = self.path(doc);
        std::fs::read(&path).with_context(|| format!("reading bronze doc {}", path.display()))
    }

    /// Local filesystem address of a document (`raw_document.storage_uri`
    /// records it; object storage arrives later behind this same shape).
    #[must_use]
    pub fn path(&self, doc: &RawDocRef) -> PathBuf {
        self.root.join(&doc.sha256)
    }
}

/// Digest bytes → lowercase hex.
fn hex_lower(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

/// Time source for stages; `Fixed` keeps runs deterministic under test.
#[derive(Debug, Clone)]
pub enum Clock {
    /// Wall clock.
    System,
    /// Frozen instant for deterministic runs.
    Fixed(DateTime<Utc>),
}

impl Clock {
    /// Current time per this clock.
    #[must_use]
    pub fn now(&self) -> DateTime<Utc> {
        match self {
            Self::System => Utc::now(),
            Self::Fixed(t) => *t,
        }
    }
}

/// Spacing + concurrency guard shared by all requests against one source.
#[derive(Debug)]
struct Throttle {
    permits: Semaphore,
    min_interval: Duration,
    next_allowed: Mutex<Option<Instant>>,
}

impl Throttle {
    fn new(min_interval: Duration, concurrency: usize) -> Self {
        Self {
            permits: Semaphore::new(concurrency.max(1)),
            min_interval,
            next_allowed: Mutex::new(None),
        }
    }

    /// Waits for a concurrency permit and this source's next min-interval slot.
    async fn ready(&self) -> anyhow::Result<SemaphorePermit<'_>> {
        let permit = self
            .permits
            .acquire()
            .await
            .context("politeness semaphore closed")?;
        let wait = {
            let mut next_allowed = self.next_allowed.lock().await;
            let now = Instant::now();
            let slot = next_allowed.map_or(now, |at| at.max(now));
            *next_allowed = Some(slot + self.min_interval);
            slot.duration_since(now) // saturates to zero when the slot is past
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        Ok(permit)
    }
}

/// A `reqwest` client that cannot forget its manners: every request passes the
/// per-source throttle and carries an identified User-Agent (invariant 10).
/// Conditional GETs go through [`PoliteClient::get_conditional`].
#[derive(Debug)]
pub struct PoliteClient {
    inner: reqwest::Client,
    throttle: Throttle,
}

impl PoliteClient {
    /// Builds the client from a source's politeness config.
    ///
    /// # Errors
    /// TLS/client construction failure.
    pub fn new(cfg: &PolitenessCfg) -> anyhow::Result<Self> {
        // Under reqwest's `rustls-no-provider` feature the process default
        // crypto provider must be installed once; ring is the provider this
        // workspace standardizes on (matches sqlx `tls-rustls`).
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        let inner = reqwest::Client::builder()
            .user_agent(cfg.user_agent())
            .build()
            .context("building reqwest client")?;
        Ok(Self {
            inner,
            throttle: Throttle::new(cfg.min_interval, cfg.concurrency),
        })
    }

    /// Polite GET: throttled and identified.
    ///
    /// # Errors
    /// Transport failure (non-2xx statuses are returned, not errors).
    pub async fn get(&self, url: &str) -> anyhow::Result<reqwest::Response> {
        self.get_conditional(url, None, None).await
    }

    /// Polite conditional GET: sends `If-None-Match` / `If-Modified-Since`
    /// when validators are known (invariant 10 — 304s are cheap for the
    /// source). A 304 comes back as a normal response, not an error.
    ///
    /// # Errors
    /// Transport failure (non-2xx statuses, including 304, are returned).
    pub async fn get_conditional(
        &self,
        url: &str,
        if_none_match: Option<&str>,
        if_modified_since: Option<&str>,
    ) -> anyhow::Result<reqwest::Response> {
        let _slot = self.throttle.ready().await?;
        let mut request = self.inner.get(url);
        if let Some(etag) = if_none_match {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        if let Some(stamp) = if_modified_since {
            request = request.header(reqwest::header::IF_MODIFIED_SINCE, stamp);
        }
        request.send().await.with_context(|| format!("GET {url}"))
    }
}

/// Everything a stage may touch. Conformance runs carry `pool: None`.
#[derive(Debug)]
pub struct RunCtx {
    /// Raw-document store (invariant 2).
    pub bronze: BronzeStore,
    /// Postgres, when a stage needs it (conformance does not).
    pub pool: Option<sqlx::PgPool>,
    /// Time source.
    pub clock: Clock,
    /// Politeness-wrapped HTTP client (invariant 10).
    pub http: PoliteClient,
}

impl RunCtx {
    /// Assembles a run context, wiring the HTTP client to the adapter's
    /// politeness config.
    ///
    /// # Errors
    /// HTTP client construction failure.
    pub fn new(
        bronze: BronzeStore,
        pool: Option<sqlx::PgPool>,
        clock: Clock,
        politeness: &PolitenessCfg,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            bronze,
            pool,
            clock,
            http: PoliteClient::new(politeness)?,
        })
    }
}

/// The jurisdiction adapter contract — design §5.1, copied faithfully.
/// An adapter ships as code + config + golden fixtures + the shared
/// conformance suite; core never changes when coverage grows.
#[async_trait]
pub trait JurisdictionAdapter: Send + Sync {
    /// Binds adapter → `disclosure_regime` row.
    fn regime(&self) -> RegimeRef;
    /// Min interval, concurrency (default 1), UA contact.
    fn politeness(&self) -> PolitenessCfg;

    /// New/changed filings.
    async fn discover(&self, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>>;
    /// Download → Bronze.
    async fn fetch(&self, r: &FilingRef, ctx: &RunCtx) -> anyhow::Result<RawDocRef>;
    /// Bronze → Silver (+ confidence).
    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>>;
    /// Silver → Gold.
    async fn normalize(
        &self,
        rows: &[StagingRow],
        ctx: &RunCtx,
    ) -> anyhow::Result<Vec<GoldCandidate>>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn user_agent_is_identified_with_contact() {
        let cfg = PolitenessCfg::new(Duration::from_secs(1), "ops@govfolio.io");
        let ua = cfg.user_agent();
        assert!(ua.contains("govfolio"), "ua must identify us: {ua}");
        assert!(
            ua.contains("ops@govfolio.io"),
            "ua must carry contact: {ua}"
        );
        assert_eq!(
            cfg.concurrency, 1,
            "default concurrency is 1 (invariant 10)"
        );
    }

    #[test]
    fn fixed_clock_is_deterministic() {
        let t = Utc::now();
        let clock = Clock::Fixed(t);
        assert_eq!(clock.now(), t);
        assert_eq!(clock.now(), t);
    }

    fn temp_bronze(tag: &str) -> BronzeStore {
        let root =
            std::env::temp_dir().join(format!("govfolio-bronze-test-{tag}-{}", std::process::id()));
        BronzeStore::open(root).unwrap()
    }

    #[test]
    fn bronze_put_is_content_addressed_and_round_trips() {
        let store = temp_bronze("roundtrip");
        let doc = store.put(b"raw filing bytes").unwrap();
        // sha256 of the exact bytes, 64 lowercase hex (verified: `sha256sum`).
        assert_eq!(
            doc.sha256,
            "f1064a17821b4b846508e2ad33cf44abcce5ebb8c282c33d6b9978c19dbd61dc"
        );
        assert_eq!(store.get(&doc).unwrap(), b"raw filing bytes".to_vec());
    }

    #[test]
    fn bronze_put_same_bytes_same_address_and_never_rewrites() {
        let store = temp_bronze("immutability");
        let a = store.put(b"identical").unwrap();
        let b = store.put(b"identical").unwrap();
        assert_eq!(a, b, "content addressing must be deterministic");
        let c = store.put(b"different").unwrap();
        assert_ne!(a.sha256, c.sha256);
    }

    #[tokio::test(start_paused = true)]
    async fn throttle_spaces_request_starts_by_min_interval() {
        let throttle = Throttle::new(Duration::from_millis(500), 1);
        let start = Instant::now();
        drop(throttle.ready().await.unwrap());
        drop(throttle.ready().await.unwrap());
        drop(throttle.ready().await.unwrap());
        assert!(
            start.elapsed() >= Duration::from_secs(1),
            "three request starts must span >= 2 * min_interval, got {:?}",
            start.elapsed()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn throttle_concurrency_one_blocks_second_request() {
        let throttle = Throttle::new(Duration::ZERO, 1);
        let held = throttle.ready().await.unwrap();
        let second = tokio::time::timeout(Duration::from_millis(10), throttle.ready()).await;
        assert!(second.is_err(), "second request must wait for the permit");
        drop(held);
        let third = tokio::time::timeout(Duration::from_millis(10), throttle.ready()).await;
        assert!(third.is_ok(), "released permit must unblock the queue");
    }
}
