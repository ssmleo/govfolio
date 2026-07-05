//! Goal 021 acceptance: `cargo test -p pipeline extraction`.
//!
//! Proves the schema-constrained LLM extraction machinery WITHOUT any
//! network: a mock [`Transport`] injects canned Messages API responses
//! (forced-tool-use request shape, local schema re-validation, high-impact
//! second-model cross-check, field-level mismatch freeze), and the committed
//! conformance cache entry is pinned to the test-designer ground truth by a
//! mechanical re-derivation (provenance: expected.silver.json @ 77740d8 —
//! never a live LLM call).

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::float_cmp)]

use std::sync::Mutex;

use async_trait::async_trait;
use serde_json::{Value, json};

use pipeline::conformance::fixtures_dir;
use pipeline::extraction::{
    CacheKey, CachedExtraction, CrossCheckMismatch, DocumentToolSpec, LlmDocumentExtractor, Models,
    Transport, build_request, prime_from_expected_silver,
};

/// Injects canned responses and records every request (goal 021: "trait over
/// the HTTP call so tests inject canned responses").
struct MockTransport {
    requests: Mutex<Vec<Value>>,
    responses: Mutex<Vec<Value>>,
}

impl MockTransport {
    fn returning(responses: Vec<Value>) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            responses: Mutex::new(responses),
        }
    }

    fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send(&self, body: &Value) -> anyhow::Result<Value> {
        self.requests.lock().unwrap().push(body.clone());
        let mut responses = self.responses.lock().unwrap();
        anyhow::ensure!(!responses.is_empty(), "mock transport exhausted");
        Ok(responses.remove(0))
    }
}

fn spec() -> DocumentToolSpec {
    DocumentToolSpec {
        tool_name: "record_rows".to_owned(),
        tool_description: "record every row".to_owned(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "filer": { "type": "string" },
                "rows": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": { "amount_raw": { "type": "string" } },
                        "required": ["amount_raw"]
                    }
                }
            },
            "required": ["filer", "rows"]
        }),
        prompt: "transcribe verbatim".to_owned(),
    }
}

fn tool_response(input: &Value) -> Value {
    json!({
        "content": [
            { "type": "tool_use", "id": "toolu_1", "name": "record_rows", "input": input }
        ],
        "stop_reason": "tool_use"
    })
}

fn models() -> Models {
    Models::from_lookup(|_| None)
}

fn good_output() -> Value {
    json!({ "filer": "Diana Harshbarger", "rows": [{ "amount_raw": "$15,001 - $50,000" }] })
}

// ---------------------------------------------------------------------------
// Forced tool use: the request IS the schema constraint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn extraction_request_forces_the_schema_constrained_tool() {
    let transport = MockTransport::returning(vec![tool_response(&good_output())]);
    let extractor = LlmDocumentExtractor::new(&transport, models());
    let out = extractor
        .extract(b"%PDF-fake", &spec(), |_| Ok(false))
        .await
        .unwrap();
    assert_eq!(out, good_output());

    let requests = transport.requests();
    assert_eq!(requests.len(), 1, "low impact = exactly one model call");
    let request = &requests[0];
    assert_eq!(request["model"], json!("claude-haiku-4-5-20251001"));
    assert_eq!(
        request["tool_choice"],
        json!({ "type": "tool", "name": "record_rows" }),
        "tool_choice must be FORCED"
    );
    assert_eq!(
        request["tools"][0]["input_schema"],
        spec().input_schema,
        "the tool input_schema IS the silver-row schema"
    );
    // The raw document travels as a base64 PDF block ahead of the prompt.
    assert_eq!(
        request["messages"][0]["content"][0]["type"],
        json!("document")
    );
    assert_eq!(
        request["messages"][0]["content"][0]["source"]["media_type"],
        json!("application/pdf")
    );
    assert_eq!(
        request["messages"][0]["content"][1]["text"],
        json!("transcribe verbatim")
    );
}

#[test]
fn extraction_request_builder_embeds_the_document_bytes() {
    use base64::Engine as _;
    let request = build_request("m", b"raw pdf bytes", &spec());
    let data = request["messages"][0]["content"][0]["source"]["data"]
        .as_str()
        .unwrap();
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(data)
            .unwrap(),
        b"raw pdf bytes"
    );
}

// ---------------------------------------------------------------------------
// Fail closed: schema-invalid output, missing tool block
// ---------------------------------------------------------------------------

#[tokio::test]
async fn extraction_schema_invalid_tool_output_fails_closed() {
    // `rows[0]` is missing the required amount_raw.
    let bad = json!({ "filer": "X", "rows": [{}] });
    let transport = MockTransport::returning(vec![tool_response(&bad)]);
    let extractor = LlmDocumentExtractor::new(&transport, models());
    let err = extractor
        .extract(b"%PDF-fake", &spec(), |_| Ok(false))
        .await
        .unwrap_err();
    let message = format!("{err:#}");
    assert!(
        message.contains("violates the extraction schema"),
        "{message}"
    );
    assert!(message.contains("fail closed"), "{message}");
}

#[tokio::test]
async fn extraction_without_a_tool_use_block_fails_closed() {
    let refusal = json!({ "content": [], "stop_reason": "refusal" });
    let transport = MockTransport::returning(vec![refusal]);
    let extractor = LlmDocumentExtractor::new(&transport, models());
    let err = extractor
        .extract(b"%PDF-fake", &spec(), |_| Ok(false))
        .await
        .unwrap_err();
    let message = format!("{err:#}");
    assert!(message.contains("refusal"), "{message}");
}

// ---------------------------------------------------------------------------
// Cross-check on impact (design §5.3)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn extraction_high_impact_takes_a_distinct_second_model_and_agreement_proceeds() {
    let transport = MockTransport::returning(vec![
        tool_response(&good_output()),
        tool_response(&good_output()),
    ]);
    let extractor = LlmDocumentExtractor::new(&transport, models());
    let out = extractor
        .extract(b"%PDF-fake", &spec(), |_| Ok(true))
        .await
        .unwrap();
    assert_eq!(out, good_output(), "agreement = proceed");

    let requests = transport.requests();
    assert_eq!(requests.len(), 2, "high impact = second-model call");
    assert_eq!(requests[0]["model"], json!("claude-haiku-4-5-20251001"));
    assert_eq!(requests[1]["model"], json!("claude-sonnet-5"));
    assert_ne!(
        requests[0]["model"], requests[1]["model"],
        "cross-check model must be DISTINCT"
    );
}

#[tokio::test]
async fn extraction_cross_check_mismatch_freezes_with_field_level_evidence() {
    let disagreeing =
        json!({ "filer": "Diana Harshbarger", "rows": [{ "amount_raw": "$50,001 - $100,000" }] });
    let transport = MockTransport::returning(vec![
        tool_response(&good_output()),
        tool_response(&disagreeing),
    ]);
    let extractor = LlmDocumentExtractor::new(&transport, models());
    let err = extractor
        .extract(b"%PDF-fake", &spec(), |_| Ok(true))
        .await
        .unwrap_err();
    let mismatch = err
        .downcast_ref::<CrossCheckMismatch>()
        .expect("typed mismatch for the review-task path");
    assert_eq!(mismatch.paths, ["$.rows[0].amount_raw"]);
    let message = format!("{err:#}");
    assert!(message.contains("llm_crosscheck_mismatch"), "{message}");
    assert!(message.contains("review_task"), "{message}");
}

// ---------------------------------------------------------------------------
// Conformance cache priming: mechanical, from test-designer ground truth
// ---------------------------------------------------------------------------

/// The committed `scanned_paper_ptr` cache entry must equal the MECHANICAL
/// transform of `expected.silver.json` (test-designer ground truth, commit
/// 77740d8) — the entry is derived, never extracted. Regenerate with
/// `UPDATE_EXTRACTION_CACHE=1 cargo test -p pipeline extraction_cache`.
#[test]
fn extraction_cache_entry_is_primed_from_test_designer_ground_truth() {
    let case_dir = fixtures_dir("us_house").join("scanned_paper_ptr");
    let expected_silver = std::fs::read_to_string(case_dir.join("expected.silver.json")).unwrap();
    let key = CacheKey::new(
        // fixtures/MANIFEST.json cases.scanned_paper_ptr.sha256
        "2f4b2b6e98e044e6368a072275804bc61dda52f6f1e15c09ddb9074ea1b8952c",
        "us_house_ptr/llm@1",
        "claude-haiku-4-5-20251001",
    );
    let provenance = json!({
        "primed_from": "expected.silver.json",
        "ground_truth": "test-designer goal 021 first leg, commit 77740d8 — independent visual transcription",
        "derived_by": "pipeline::extraction::prime_from_expected_silver (mechanical transform, NOT a live LLM call)",
        "enforced_by": "cargo test -p pipeline extraction_cache_entry_is_primed_from_test_designer_ground_truth",
    });
    let derived = prime_from_expected_silver(&expected_silver, key, provenance).unwrap();

    let cache_path = case_dir.join("extraction.cache.json");
    if std::env::var_os("UPDATE_EXTRACTION_CACHE").is_some() {
        let mut text = serde_json::to_string_pretty(&derived).unwrap();
        text.push('\n');
        std::fs::write(&cache_path, text).unwrap();
    }
    let committed: CachedExtraction =
        serde_json::from_str(&std::fs::read_to_string(&cache_path).unwrap()).unwrap();
    assert_eq!(
        committed, derived,
        "extraction.cache.json drifted from the expected.silver.json ground truth — \
         regenerate mechanically (UPDATE_EXTRACTION_CACHE=1), never hand-edit"
    );
    // The staged confidence convention survives the round trip: 0.9f32.
    assert!(committed.rows.iter().all(|row| row.confidence == 0.9f32));
}
