//! Document-level LLM-extraction seam (design §5.3, regime doc §6.3), wired
//! by goal 021: scanned/paper PTRs (no text layer) extract through the
//! Anthropic Messages API, schema-constrained by forced tool use, cached by
//! `(document_sha256, extractor_tag, model_id)`, confidence-stamped at
//! 0.9 f32, and cross-checked by a second model on high-impact documents.
//!
//! Fail-closed order of business (invariant 6 at every exit):
//! 1. committed file cache (`fixtures/<case>/extraction.cache.json`) — this
//!    is what keeps conformance and e2e OFFLINE; a hit never calls the API;
//! 2. `extraction_cache` Postgres tier (pool-backed runs);
//! 3. live extraction — requires `ANTHROPIC_API_KEY` AND a resolvable `DocID`
//!    (from `raw_document.source_url`, regime doc §2.3 URL shape; the paper
//!    form prints no Filing ID). Anything short of that freezes the document
//!    behind a `needs_llm_extraction` error → review path, never silent rows.

use anyhow::Context as _;
use async_trait::async_trait;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use pipeline::adapter::{RawDocRef, RunCtx, StagingRow};
use pipeline::extraction::{
    CacheKey, CrossCheckMismatch, DocumentToolSpec, FileCache, HttpTransport, LlmDocumentExtractor,
    Models, Transport, WATCHLIST_POLITICIANS, pg_get, pg_put,
};
use pipeline::stages::roster::open_review_task_once;

use crate::parse::SilverRow;
use crate::tables;

/// Extractor tag recorded on every LLM-path Silver row (regime doc §4;
/// fixtures MANIFEST `paper_form_conventions`). The text path stays
/// `us_house_ptr/text@1`.
pub(crate) const EXTRACTOR_LLM: &str = "us_house_ptr/llm@1";

/// Wrapper confidence every llm@1 row carries (staged convention: 0.9 f32,
/// design §5.3 confidence-scored extraction; design §7 spot-check band).
pub(crate) const LLM_CONFIDENCE: f32 = 0.9;

/// Rows below this wrapper confidence fail closed (review path) — llm@1
/// emits exactly [`LLM_CONFIDENCE`], so anything lower is a tampered or
/// foreign cache entry.
const MIN_ACCEPT_CONFIDENCE: f32 = 0.9;

/// §6.3 high-impact band floor: bands from `$500,001 - $1,000,000` up take
/// the second-model cross-check (regime doc §6.3 "≥ $500,001 bands").
const CROSS_CHECK_BAND_LOW_MIN: &str = "500001.00";

/// Extraction seam for documents the deterministic text-layer path cannot
/// handle: zero rows, mean row confidence < 0.90, or paper filings.
#[async_trait]
pub trait Extractor: Send + Sync {
    /// Extracts Silver rows from one Bronze document.
    ///
    /// # Errors
    /// Extraction failure — the document freezes behind the error (review
    /// path); rows are never guessed.
    async fn extract(&self, doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>>;
}

/// What one extraction call must transcribe: the filer block plus every
/// transaction row, verbatim as printed — the tool `input_schema` is derived
/// from this type, so the model can only answer in this shape. Field docs are
/// contract surface: they steer the model (schemars embeds them as
/// `description`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct LlmDocExtraction {
    /// The NAME line verbatim. Paper forms print it WITHOUT the `Hon.`
    /// honorific — transcribe exactly what is written, never add a prefix.
    pub(crate) filer_name_raw: String,
    /// `Member` when the `Member of the U.S. House of Representatives` box is
    /// checked; `Officer or Employee` when that box is checked (electronic
    /// vocabulary).
    pub(crate) filer_status_raw: String,
    /// State + zero-padded 2-digit district composed from the State and
    /// District form fields, e.g. `TN01` (index `StateDst` format).
    pub(crate) state_district_raw: String,
    /// The filing date printed IN the document. Paper forms have no signature
    /// block: use the clerk received stamp verbatim (e.g. `2026 MAY -6`).
    /// Electronic forms use the `Digitally Signed:` MM/DD/YYYY date.
    pub(crate) signed_date_raw: String,
    /// Every transaction row, in document order.
    pub(crate) rows: Vec<LlmTransactionRow>,
}

/// One transaction row, verbatim (regime doc §4 vocabulary).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub(crate) struct LlmTransactionRow {
    /// 10-digit eFD transaction id — printed only on amended electronic rows;
    /// null on paper forms.
    pub(crate) row_id_raw: Option<String>,
    /// Owner code exactly as printed (`SP`/`DC`/`JT`); null when the owner
    /// column is blank.
    pub(crate) owner_code_raw: Option<String>,
    /// Full asset cell verbatim, line wraps joined by single spaces.
    pub(crate) asset_raw: String,
    /// The `XX` from a trailing `[XX]` asset-type code; null when absent
    /// (paper forms print no codes).
    pub(crate) asset_type_code_raw: Option<String>,
    /// Transaction type token: `P`/`S`/`S (partial)`/`E`. Paper checkbox
    /// columns map to this vocabulary: Purchase→P, Sale→S,
    /// Partial Sale→S (partial), Exchange→E.
    pub(crate) transaction_type_raw: String,
    /// Transaction date as printed (M/D/YYYY or MM/DD/YYYY).
    pub(crate) transaction_date_raw: String,
    /// Notification date as printed (M/D/YYYY or MM/DD/YYYY).
    pub(crate) notification_date_raw: String,
    /// Amount band as the canonical electronic string with spaces around the
    /// hyphen, e.g. `$15,001 - $50,000` — paper column headers print the same
    /// band without spaces; emit the canonical form.
    pub(crate) amount_raw: String,
    /// Cap. Gains > $200 checkbox; null when the form has no such column
    /// (paper) or the state is indeterminate.
    pub(crate) cap_gains_over_200: Option<bool>,
    /// `New` when Initial Report is checked, `Amended` when Amendment is
    /// checked (electronic vocabulary).
    pub(crate) filing_status_raw: String,
    /// `SUBHOLDING OF:` value; null when absent.
    pub(crate) subholding_of_raw: Option<String>,
    /// `DESCRIPTION:` value; null when absent.
    pub(crate) description_raw: Option<String>,
    /// `COMMENTS:` value; null when absent.
    pub(crate) comments_raw: Option<String>,
    /// `(Owner: XX)` of the row's matching investment-vehicle bullet; null
    /// when absent.
    pub(crate) vehicle_owner_code_raw: Option<String>,
    /// `LOCATION:` of the matching vehicle bullet; null when absent.
    pub(crate) vehicle_location_raw: Option<String>,
}

/// The goal-021 LLM extractor: cache tiers + Anthropic Messages fallback.
#[derive(Debug, Clone)]
pub struct LlmExtractor {
    file_cache: FileCache,
}

impl Default for LlmExtractor {
    fn default() -> Self {
        // Committed conformance/e2e cache tier; on hosts without a source
        // tree the directory is simply absent (= empty cache, Postgres tier
        // takes over).
        Self {
            file_cache: FileCache::open(pipeline::conformance::fixtures_dir("us_house")),
        }
    }
}

#[async_trait]
impl Extractor for LlmExtractor {
    async fn extract(&self, doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        let models = Models::from_env();
        let key = CacheKey::new(&doc.sha256, EXTRACTOR_LLM, &models.primary);
        // Tier 1: committed file cache (conformance/e2e — offline, no API).
        if let Some(rows) = self.file_cache.get(&key)? {
            return validated(rows, &doc.sha256);
        }
        // Tier 2: extraction_cache (pool-backed runs).
        if let Some(pool) = &ctx.pool
            && let Some(rows) = pg_get(pool, &key).await?
        {
            return validated(rows, &doc.sha256);
        }
        // Tier 3: live extraction — pay per document version, once.
        let Ok(transport) = HttpTransport::from_env() else {
            anyhow::bail!(
                "needs_llm_extraction: no cached extraction for document {} \
                 (key: {EXTRACTOR_LLM} / {}) and ANTHROPIC_API_KEY is absent — \
                 freeze + review_task (invariant 6)",
                doc.sha256,
                models.primary
            );
        };
        extract_live(doc, ctx, &transport, &models, &key).await
    }
}

/// Live extraction against the Messages API. Separated (and generic over
/// [`Transport`]) so tests drive the full seam with canned responses.
pub(crate) async fn extract_live<T: Transport>(
    doc: &RawDocRef,
    ctx: &RunCtx,
    transport: &T,
    models: &Models,
    key: &CacheKey,
) -> anyhow::Result<Vec<StagingRow>> {
    // The paper form prints no Filing ID (fixtures MANIFEST
    // paper_form_conventions): the DocID is threaded from pipeline context —
    // the recorded fetch URL (regime doc §2.3 `…/ptr-pdfs/<year>/<DocID>.pdf`).
    let doc_id = resolve_doc_id(doc, ctx).await?.context(
        "needs_llm_extraction: DocID unresolvable from pipeline context (no indexed \
                  source_url for this document) — freeze + review_task (invariant 6)",
    )?;
    let bytes = ctx.bronze.get(doc)?;
    let extractor = LlmDocumentExtractor::new(transport, models.clone());
    let outcome = extractor.extract(&bytes, &tool_spec()?, high_impact).await;
    let value = match outcome {
        Ok(value) => value,
        Err(error) => {
            if error.downcast_ref::<CrossCheckMismatch>().is_some()
                && let Some(pool) = &ctx.pool
            {
                // Freeze paper trail: the mismatch opens a review task; the
                // error still propagates so the document never publishes.
                open_review_task_once(
                    pool,
                    "raw_document",
                    &format!("us_house:{}", doc.sha256),
                    "llm_crosscheck_mismatch",
                )
                .await?;
            }
            return Err(error);
        }
    };
    let extraction: LlmDocExtraction = serde_json::from_value(value)
        .context("schema-valid tool output does not deserialize — fail closed")?;
    let rows = to_staging_rows(&extraction, &doc_id)?;
    if let Some(pool) = &ctx.pool {
        let provenance = serde_json::json!({
            "source": "live anthropic messages call",
            "primary_model": models.primary,
            "crosscheck_model": models.crosscheck,
            "cross_checked": high_impact_rows(&extraction)?,
        });
        pg_put(pool, key, &rows, &provenance).await?;
    }
    Ok(rows)
}

/// The forced-tool extraction contract for one PTR document.
fn tool_spec() -> anyhow::Result<DocumentToolSpec> {
    let schema = schemars::schema_for!(LlmDocExtraction);
    Ok(DocumentToolSpec {
        tool_name: "record_ptr_transactions".to_owned(),
        tool_description: "Record every transaction row of this US House Periodic Transaction \
                           Report exactly as printed. Transcribe verbatim — never normalize, \
                           summarize, infer, or guess a value that is not visibly on the form. \
                           Use null for any field the form does not carry."
            .to_owned(),
        input_schema: serde_json::to_value(schema).context("serializing extraction schema")?,
        prompt: "This is a US House of Representatives Periodic Transaction Report (PTR). It may \
                 be a scanned paper form. Transcribe the filer block and every transaction row \
                 exactly as printed using the record_ptr_transactions tool. Paper-form \
                 conventions: checked transaction-type columns map to the electronic tokens \
                 (Purchase→P, Sale→S, Partial Sale→S (partial), Exchange→E); the checked amount \
                 column maps to the canonical band string with spaces around the hyphen (e.g. \
                 $15,001 - $50,000); Initial Report→New, Amendment→Amended; a checked 'Member of \
                 the U.S. House of Representatives' box→Member; state_district_raw is State plus \
                 zero-padded 2-digit District (e.g. TN01); the clerk received stamp (e.g. \
                 '2026 MAY -6') is the signed_date_raw when no signature block exists. Transcribe \
                 asset names verbatim even when the handwriting or scan is unclear."
            .to_owned(),
    })
}

/// §6.3 high-impact predicate over the primary extraction: any row in a
/// `≥ $500,001` band, or a watchlist filer (stub list, design §5.3).
fn high_impact(value: &Value) -> anyhow::Result<bool> {
    let extraction: LlmDocExtraction = serde_json::from_value(value.clone())
        .context("high-impact predicate: tool output does not deserialize")?;
    high_impact_rows(&extraction)
}

fn high_impact_rows(extraction: &LlmDocExtraction) -> anyhow::Result<bool> {
    if WATCHLIST_POLITICIANS.contains(&extraction.filer_name_raw.as_str()) {
        return Ok(true);
    }
    let floor: Decimal = CROSS_CHECK_BAND_LOW_MIN
        .parse()
        .map_err(|e| anyhow::anyhow!("cross-check floor: {e}"))?;
    for row in &extraction.rows {
        // A band outside the grammar hard-rejects at normalize anyway; for
        // the impact decision it is treated as high (conservative).
        let Some((low, _)) = tables::band_bounds(&row.amount_raw) else {
            return Ok(true);
        };
        let low: Decimal = low
            .parse()
            .map_err(|e| anyhow::anyhow!("band low {low:?}: {e}"))?;
        if low >= floor {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Assembles full Silver rows from an extraction: the `DocID` and ordinals are
/// threaded here (the model cannot know them), the tag and 0.9 confidence
/// are stamped per the staged convention.
fn to_staging_rows(extraction: &LlmDocExtraction, doc_id: &str) -> anyhow::Result<Vec<StagingRow>> {
    anyhow::ensure!(
        !extraction.rows.is_empty(),
        "LLM extraction produced zero rows — fail closed (invariant 6)"
    );
    let mut rows = Vec::with_capacity(extraction.rows.len());
    for (index, row) in extraction.rows.iter().enumerate() {
        let row_ordinal = u32::try_from(index + 1).context("row ordinal overflow")?;
        let silver = SilverRow {
            doc_id: doc_id.to_owned(),
            row_ordinal,
            filer_name_raw: extraction.filer_name_raw.clone(),
            filer_status_raw: extraction.filer_status_raw.clone(),
            state_district_raw: extraction.state_district_raw.clone(),
            row_id_raw: row.row_id_raw.clone(),
            owner_code_raw: row.owner_code_raw.clone(),
            asset_raw: row.asset_raw.clone(),
            asset_type_code_raw: row.asset_type_code_raw.clone(),
            transaction_type_raw: row.transaction_type_raw.clone(),
            transaction_date_raw: row.transaction_date_raw.clone(),
            notification_date_raw: row.notification_date_raw.clone(),
            amount_raw: row.amount_raw.clone(),
            cap_gains_over_200: row.cap_gains_over_200,
            filing_status_raw: row.filing_status_raw.clone(),
            subholding_of_raw: row.subholding_of_raw.clone(),
            description_raw: row.description_raw.clone(),
            comments_raw: row.comments_raw.clone(),
            vehicle_owner_code_raw: row.vehicle_owner_code_raw.clone(),
            vehicle_location_raw: row.vehicle_location_raw.clone(),
            signed_date_raw: extraction.signed_date_raw.clone(),
            extractor: EXTRACTOR_LLM.to_owned(),
        };
        rows.push(StagingRow {
            payload: serde_json::to_value(&silver).context("serializing LLM silver payload")?,
            confidence: LLM_CONFIDENCE,
        });
    }
    Ok(rows)
}

/// Threads the `DocID` from pipeline context: the recorded fetch URL of this
/// Bronze document (`raw_document.source_url`, regime doc §2.3 shape).
async fn resolve_doc_id(doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Option<String>> {
    let Some(pool) = &ctx.pool else {
        return Ok(None);
    };
    let source_url: Option<Option<String>> =
        sqlx::query_scalar("select source_url from raw_document where sha256 = $1")
            .bind(&doc.sha256)
            .fetch_optional(pool)
            .await
            .context("reading raw_document.source_url")?;
    Ok(source_url.flatten().as_deref().and_then(doc_id_from_url))
}

/// `…/ptr-pdfs/<year>/<DocID>.pdf` → `DocID` (4–8 digits, §2.2 shape).
fn doc_id_from_url(url: &str) -> Option<String> {
    let stem = url.rsplit('/').next()?.strip_suffix(".pdf")?;
    ((4..=8).contains(&stem.len()) && stem.bytes().all(|b| b.is_ascii_digit()))
        .then(|| stem.to_owned())
}

/// Fail-closed validation of cached rows: every payload must be a real
/// `SilverRow` carrying the llm@1 tag, and the wrapper confidence must sit in
/// `[0.9, 1.0]` — below-threshold or schema-invalid entries never publish.
fn validated(rows: Vec<StagingRow>, sha256: &str) -> anyhow::Result<Vec<StagingRow>> {
    anyhow::ensure!(
        !rows.is_empty(),
        "cached extraction for {sha256} is empty — fail closed (invariant 6)"
    );
    for (index, staged) in rows.iter().enumerate() {
        let row: SilverRow = serde_json::from_value(staged.payload.clone()).with_context(|| {
            format!("cached extraction row {index} for {sha256} is not a SilverRow — fail closed")
        })?;
        anyhow::ensure!(
            row.extractor == EXTRACTOR_LLM,
            "cached extraction row {index} for {sha256} carries tag {:?}, want {EXTRACTOR_LLM:?} \
             — fail closed",
            row.extractor
        );
        anyhow::ensure!(
            (MIN_ACCEPT_CONFIDENCE..=1.0).contains(&staged.confidence),
            "cached extraction row {index} for {sha256} has confidence {} below the {} floor — \
             fail closed (review path, never silent Gold)",
            staged.confidence,
            MIN_ACCEPT_CONFIDENCE
        );
    }
    Ok(rows)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::float_cmp)] // exact f32 bit-image IS the contract
mod tests {
    use serde_json::json;

    use super::*;

    fn extraction(amount_raw: &str) -> LlmDocExtraction {
        LlmDocExtraction {
            filer_name_raw: "Diana Harshbarger".to_owned(),
            filer_status_raw: "Member".to_owned(),
            state_district_raw: "TN01".to_owned(),
            signed_date_raw: "2026 MAY -6".to_owned(),
            rows: vec![LlmTransactionRow {
                row_id_raw: None,
                owner_code_raw: None,
                asset_raw: "Black Belt Energy Gas DI SR C RV BE/R/, Municipal Bond".to_owned(),
                asset_type_code_raw: None,
                transaction_type_raw: "P".to_owned(),
                transaction_date_raw: "4/17/2026".to_owned(),
                notification_date_raw: "4/29/2026".to_owned(),
                amount_raw: amount_raw.to_owned(),
                cap_gains_over_200: None,
                filing_status_raw: "New".to_owned(),
                subholding_of_raw: None,
                description_raw: None,
                comments_raw: None,
                vehicle_owner_code_raw: None,
                vehicle_location_raw: None,
            }],
        }
    }

    #[test]
    fn staging_rows_thread_doc_id_and_stamp_tag_plus_confidence() {
        let rows = to_staging_rows(&extraction("$15,001 - $50,000"), "9115811").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].payload["doc_id"], json!("9115811"));
        assert_eq!(rows[0].payload["row_ordinal"], json!(1));
        assert_eq!(rows[0].payload["extractor"], json!(EXTRACTOR_LLM));
        assert_eq!(rows[0].confidence, LLM_CONFIDENCE);
        // The staged convention literal: the exact f64 image of 0.9f32.
        assert_eq!(
            serde_json::to_value(&rows[0]).unwrap()["confidence"],
            json!(0.899_999_976_158_142_1_f64)
        );
    }

    #[test]
    fn zero_extracted_rows_fail_closed() {
        let mut empty = extraction("$15,001 - $50,000");
        empty.rows.clear();
        assert!(to_staging_rows(&empty, "9115811").is_err());
    }

    #[test]
    fn high_impact_predicate_follows_the_section_6_3_band_floor() {
        // Below the §6.3 floor: no cross-check.
        assert!(!high_impact_rows(&extraction("$15,001 - $50,000")).unwrap());
        assert!(!high_impact_rows(&extraction("$250,001 - $500,000")).unwrap());
        // At/above the floor: cross-check.
        assert!(high_impact_rows(&extraction("$500,001 - $1,000,000")).unwrap());
        assert!(high_impact_rows(&extraction("Over $50,000,000")).unwrap());
        // Outside the grammar: conservative (normalize hard-rejects anyway).
        assert!(high_impact_rows(&extraction("$1 - $2")).unwrap());
        // Watchlist is a stub — empty until the product surface exists.
        assert!(WATCHLIST_POLITICIANS.is_empty());
    }

    #[test]
    fn cached_rows_below_confidence_floor_or_foreign_tag_fail_closed() {
        let good = to_staging_rows(&extraction("$15,001 - $50,000"), "9115811").unwrap();
        assert!(validated(good.clone(), "sha").is_ok());

        let mut low = good.clone();
        low[0].confidence = 0.5;
        let err = validated(low, "sha").unwrap_err().to_string();
        assert!(err.contains("below the"), "{err}");

        let mut foreign = good.clone();
        foreign[0].payload["extractor"] = json!("us_house_ptr/text@1");
        assert!(validated(foreign, "sha").is_err());

        let mut garbage = good;
        garbage[0].payload = json!({"not": "a silver row"});
        assert!(validated(garbage, "sha").is_err());
        assert!(validated(Vec::new(), "sha").is_err(), "empty cache entry");
    }

    #[test]
    fn doc_id_threads_from_the_ptr_pdf_url_shape() {
        assert_eq!(
            doc_id_from_url(
                "https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/9115811.pdf"
            ),
            Some("9115811".to_owned())
        );
        assert_eq!(doc_id_from_url("file:///fixtures/input.pdf"), None);
        assert_eq!(doc_id_from_url("https://x/123456789012.pdf"), None);
    }

    fn scanned_fixture_ctx(tag: &str) -> (RunCtx, RawDocRef) {
        use pipeline::adapter::{BronzeStore, Clock, PolitenessCfg};
        let root =
            std::env::temp_dir().join(format!("govfolio-llm-seam-{tag}-{}", std::process::id()));
        let ctx = RunCtx::new(
            BronzeStore::open(root).unwrap(),
            None,
            Clock::System,
            &PolitenessCfg::new(std::time::Duration::ZERO, "test@govfolio.io"),
        )
        .unwrap();
        let bytes = std::fs::read(
            pipeline::conformance::fixtures_dir("us_house")
                .join("scanned_paper_ptr")
                .join("input.pdf"),
        )
        .unwrap();
        let doc = ctx.bronze.put(&bytes).unwrap();
        (ctx, doc)
    }

    /// Cache hit = no API call (design §5.3: pay per document VERSION, once):
    /// no transport is even constructed — this test runs with no key and no
    /// network and must return the primed ground-truth rows.
    #[tokio::test]
    async fn cache_hit_extracts_offline_without_any_api_call() {
        let (ctx, doc) = scanned_fixture_ctx("cache-hit");
        let rows = LlmExtractor::default().extract(&doc, &ctx).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].payload["doc_id"], json!("9115811"));
        assert_eq!(rows[0].payload["extractor"], json!(EXTRACTOR_LLM));
        assert_eq!(rows[0].confidence, LLM_CONFIDENCE);
    }

    /// A cache miss with no `ANTHROPIC_API_KEY` freezes the document behind
    /// the review path — never silent rows (invariant 6).
    #[tokio::test]
    async fn cache_miss_without_api_key_fails_closed() {
        let (ctx, _) = scanned_fixture_ctx("cache-miss");
        let unknown = ctx.bronze.put(b"some other document").unwrap();
        if std::env::var_os("ANTHROPIC_API_KEY").is_some() {
            eprintln!("ANTHROPIC_API_KEY present — skipping the no-key fail-closed assertion");
            return;
        }
        let err = LlmExtractor::default()
            .extract(&unknown, &ctx)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("needs_llm_extraction"), "{err}");
        assert!(err.contains("review_task"), "{err}");
    }

    /// Live integration: the real Messages API extracts the scanned fixture
    /// and must agree with the test-designer ground truth on every contract
    /// field. Skips (loudly) when the key is absent — the default path stays
    /// offline.
    #[tokio::test]
    #[ignore = "needs ANTHROPIC_API_KEY"]
    async fn live_extraction_agrees_with_ground_truth() {
        if std::env::var_os("ANTHROPIC_API_KEY").is_none() {
            eprintln!("ANTHROPIC_API_KEY absent — skipping the live extraction test");
            return;
        }
        let (ctx, doc) = scanned_fixture_ctx("live");
        let bytes = ctx.bronze.get(&doc).unwrap();
        let transport = HttpTransport::from_env().unwrap();
        let extractor = LlmDocumentExtractor::new(transport, Models::from_env());
        let value = extractor
            .extract(&bytes, &tool_spec().unwrap(), high_impact)
            .await
            .unwrap();
        let live: LlmDocExtraction = serde_json::from_value(value).unwrap();
        let truth = extraction("$15,001 - $50,000");
        assert_eq!(live.filer_name_raw, truth.filer_name_raw);
        assert_eq!(live.state_district_raw, truth.state_district_raw);
        assert_eq!(live.rows.len(), 1);
        assert_eq!(live.rows[0].transaction_type_raw, "P");
        assert_eq!(live.rows[0].amount_raw, truth.rows[0].amount_raw);
        assert_eq!(
            live.rows[0].transaction_date_raw,
            truth.rows[0].transaction_date_raw
        );
    }

    #[test]
    fn tool_schema_requires_the_row_vocabulary_fields() {
        let spec = tool_spec().unwrap();
        let required = spec.input_schema["required"].as_array().unwrap();
        for field in [
            "filer_name_raw",
            "filer_status_raw",
            "state_district_raw",
            "signed_date_raw",
            "rows",
        ] {
            assert!(required.contains(&json!(field)), "{field}: {required:?}");
        }
        let row_schema = &spec.input_schema["$defs"]["LlmTransactionRow"];
        let row_required = row_schema["required"].as_array().unwrap();
        for field in ["asset_raw", "transaction_type_raw", "amount_raw"] {
            assert!(
                row_required.contains(&json!(field)),
                "{field}: {row_required:?}"
            );
        }
    }
}
