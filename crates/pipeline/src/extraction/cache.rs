//! Extraction cache (design §5.3 "cache by SHA"): an extraction is addressed
//! by `(document_sha256, extractor_tag, model_id)` — the document VERSION —
//! so a document is paid for once per extractor/model version.
//!
//! Two tiers share one entry shape ([`CachedExtraction`]):
//! - **File cache** ([`FileCache`]): committed `extraction.cache.json` files
//!   inside fixture case directories. This is what keeps conformance and e2e
//!   OFFLINE: a cache hit never touches the API. Conformance entries are
//!   primed MECHANICALLY from `expected.silver.json` ground truth via
//!   [`prime_from_expected_silver`] — never from a live LLM call — and their
//!   provenance records that.
//! - **Postgres tier** ([`pg_get`]/[`pg_put`]): the `extraction_cache` table
//!   (migration 0004, expand-only) for production runs.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::PgPool;

use crate::adapter::StagingRow;

/// The design §5.3 cache address: one extraction per document version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CacheKey {
    /// Bronze address of the raw document (64 lowercase hex).
    pub document_sha256: String,
    /// Adapter extractor tag, e.g. `us_house_ptr/llm@1`.
    pub extractor_tag: String,
    /// Model that produced (or would produce) the extraction.
    pub model_id: String,
}

impl CacheKey {
    /// Assembles a key.
    #[must_use]
    pub fn new(document_sha256: &str, extractor_tag: &str, model_id: &str) -> Self {
        Self {
            document_sha256: document_sha256.to_owned(),
            extractor_tag: extractor_tag.to_owned(),
            model_id: model_id.to_owned(),
        }
    }
}

/// One cached extraction: the key it answers, the Silver rows it yields, and
/// provenance metadata (where the rows came from — ground-truth priming vs a
/// live, possibly cross-checked LLM call).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachedExtraction {
    /// The cache address this entry answers.
    pub key: CacheKey,
    /// The extraction result: full Silver rows (payload + wrapper confidence).
    pub rows: Vec<StagingRow>,
    /// Provenance: how this entry was produced (audit surface, never trusted
    /// for control flow).
    pub provenance: Value,
}

/// The committed file-cache tier: scans `<root>/<case>/extraction.cache.json`
/// under a fixtures directory. Missing roots are simply empty caches — the
/// production tier is Postgres.
#[derive(Debug, Clone)]
pub struct FileCache {
    root: PathBuf,
}

/// File name of an in-case cache entry.
pub const CACHE_FILE: &str = "extraction.cache.json";

impl FileCache {
    /// Opens a file cache rooted at a fixtures directory (one subdirectory
    /// per case). The directory may not exist (non-dev hosts): that is an
    /// empty cache, not an error.
    #[must_use]
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Looks a key up. A present-but-unparseable entry is an error (fail
    /// closed — a corrupt committed cache must be loud); an absent entry is
    /// `None`.
    ///
    /// # Errors
    /// Unparseable `extraction.cache.json` under the root.
    pub fn get(&self, key: &CacheKey) -> anyhow::Result<Option<Vec<StagingRow>>> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Ok(None); // no fixtures on this host — Postgres tier only
        };
        for entry in entries.flatten() {
            let path = entry.path().join(CACHE_FILE);
            if !path.is_file() {
                continue;
            }
            let cached = read_entry(&path)?;
            if &cached.key == key {
                return Ok(Some(cached.rows));
            }
        }
        Ok(None)
    }
}

/// Reads and parses one committed cache entry.
///
/// # Errors
/// I/O or JSON failure (fail closed).
pub fn read_entry(path: &Path) -> anyhow::Result<CachedExtraction> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Mechanically transforms committed `expected.silver.json` ground truth into
/// a cache entry: the rows ARE the test-designer expecteds, byte-for-byte in
/// value space — no LLM call is involved. Provenance travels with the entry.
///
/// # Errors
/// `expected.silver.json` that is not a non-empty `Vec<StagingRow>` (fail
/// closed — an empty ground truth can prime nothing).
pub fn prime_from_expected_silver(
    expected_silver_json: &str,
    key: CacheKey,
    provenance: Value,
) -> anyhow::Result<CachedExtraction> {
    let rows: Vec<StagingRow> = serde_json::from_str(expected_silver_json)
        .context("expected.silver.json is not a Vec<StagingRow>")?;
    anyhow::ensure!(
        !rows.is_empty(),
        "expected.silver.json is empty — nothing to prime (fail closed)"
    );
    Ok(CachedExtraction {
        key,
        rows,
        provenance,
    })
}

/// Postgres cache lookup (`extraction_cache`, migration 0004).
///
/// # Errors
/// Database failure or a stored `rows` payload that no longer deserializes
/// (fail closed).
pub async fn pg_get(pool: &PgPool, key: &CacheKey) -> anyhow::Result<Option<Vec<StagingRow>>> {
    let stored: Option<Value> = sqlx::query_scalar(
        "select rows from extraction_cache \
         where document_sha256 = $1 and extractor_tag = $2 and model_id = $3",
    )
    .bind(&key.document_sha256)
    .bind(&key.extractor_tag)
    .bind(&key.model_id)
    .fetch_optional(pool)
    .await
    .context("extraction_cache lookup")?;
    stored
        .map(|value| {
            serde_json::from_value(value).context("extraction_cache rows do not deserialize")
        })
        .transpose()
}

/// Postgres cache insert — idempotent (`ON CONFLICT DO NOTHING`, invariant 4):
/// the first extraction of a document version wins; replays add nothing.
///
/// # Errors
/// Database or serialization failure.
pub async fn pg_put(
    pool: &PgPool,
    key: &CacheKey,
    rows: &[StagingRow],
    provenance: &Value,
) -> anyhow::Result<()> {
    let rows_json = serde_json::to_value(rows).context("serializing extraction rows")?;
    sqlx::query(
        "insert into extraction_cache \
           (document_sha256, extractor_tag, model_id, rows, provenance) \
         values ($1, $2, $3, $4, $5) \
         on conflict (document_sha256, extractor_tag, model_id) do nothing",
    )
    .bind(&key.document_sha256)
    .bind(&key.extractor_tag)
    .bind(&key.model_id)
    .bind(rows_json)
    .bind(provenance)
    .execute(pool)
    .await
    .context("extraction_cache insert")?;
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)] // exact f32 bit-image IS the contract
mod tests {
    use serde_json::json;

    use super::*;

    fn key() -> CacheKey {
        CacheKey::new(
            "2f4b2b6e98e044e6368a072275804bc61dda52f6f1e15c09ddb9074ea1b8952c",
            "us_house_ptr/llm@1",
            "claude-haiku-4-5-20251001",
        )
    }

    #[test]
    fn priming_is_a_mechanical_transform_of_ground_truth() {
        let silver = r#"[{"payload": {"doc_id": "9115811"}, "confidence": 0.8999999761581421}]"#;
        let entry =
            prime_from_expected_silver(silver, key(), json!({"primed_from": "ground truth"}))
                .unwrap();
        assert_eq!(entry.rows.len(), 1);
        assert_eq!(entry.rows[0].payload["doc_id"], json!("9115811"));
        // 0.8999999761581421 is the exact f64 image of 0.9f32 (MANIFEST
        // confidence_literal convention) — priming must not disturb it.
        assert_eq!(entry.rows[0].confidence, 0.9f32);
        assert_eq!(
            serde_json::to_value(&entry.rows[0]).unwrap()["confidence"],
            json!(0.899_999_976_158_142_1_f64)
        );
    }

    #[test]
    fn priming_fails_closed_on_empty_or_malformed_ground_truth() {
        assert!(prime_from_expected_silver("[]", key(), json!({})).is_err());
        assert!(prime_from_expected_silver("not json", key(), json!({})).is_err());
        assert!(prime_from_expected_silver(r#"[{"no": "wrapper"}]"#, key(), json!({})).is_err());
    }

    #[test]
    fn file_cache_misses_on_absent_root_and_hits_on_matching_key() {
        let missing = FileCache::open("Z:/no/such/dir");
        assert!(missing.get(&key()).unwrap().is_none());

        let dir = tempfile::tempdir().unwrap();
        let case = dir.path().join("some_case");
        fs::create_dir_all(&case).unwrap();
        let silver = r#"[{"payload": {"doc_id": "9115811"}, "confidence": 0.8999999761581421}]"#;
        let entry = prime_from_expected_silver(silver, key(), json!({})).unwrap();
        fs::write(
            case.join(CACHE_FILE),
            serde_json::to_string_pretty(&entry).unwrap(),
        )
        .unwrap();

        let cache = FileCache::open(dir.path());
        let rows = cache.get(&key()).unwrap().unwrap();
        assert_eq!(rows, entry.rows);
        // A different model id is a different document version: miss.
        let other = CacheKey::new(&key().document_sha256, &key().extractor_tag, "other-model");
        assert!(cache.get(&other).unwrap().is_none());
    }

    #[test]
    fn corrupt_committed_cache_entries_fail_closed() {
        let dir = tempfile::tempdir().unwrap();
        let case = dir.path().join("bad_case");
        fs::create_dir_all(&case).unwrap();
        fs::write(case.join(CACHE_FILE), "{ not json").unwrap();
        let cache = FileCache::open(dir.path());
        assert!(cache.get(&key()).is_err(), "corrupt cache must be loud");
    }
}
