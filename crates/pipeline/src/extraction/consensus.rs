//! Consensus extraction (goal 021 consensus addendum): N independently
//! sampled model passes over one document are aligned by row, scored for
//! field-level agreement, and routed to either full-confidence publication
//! or a held review state. Zero I/O lives in this module — alignment,
//! scoring, and routing are pure functions over already-fetched sample
//! payloads (`SamplePass`); the network-calling orchestration
//! (`ConsensusExtractor`, a later task) is ADDITIVE on top and never
//! modifies the frozen v1 single-pass seam
//! (`extraction::anthropic::LlmDocumentExtractor`).

use serde_json::Value;

use crate::extraction::anthropic::DocumentToolSpec;

/// Closed confidence set for consensus-published rows (`policy_v1`). No path
/// in this module — or any caller of it — emits a value `>= 0.95` or a
/// value outside this set.
pub mod policy {
    /// All N samples agreed on every critical field.
    pub const CONF_AGREED: f32 = 0.9;
    /// Disagreement resolved by a distinct escalation-model call agreeing
    /// with the sampled majority. Reserved here so the closed set lives in
    /// one place — no path in Tasks 1-4 emits this value yet (escalation is
    /// a later task; disagreement holds, it is never auto-resolved here).
    pub const CONF_ESCALATED: f32 = 0.75;
    /// An otherwise fully-agreed row failed an adapter-supplied sanity
    /// check.
    pub const CONF_SANITY_CAPPED: f32 = 0.79;
    /// Policy version tag folded into `composite_model_id` (config.rs, a
    /// later task).
    pub const POLICY_VERSION: &str = "pol1";
}

/// What one document's consensus pass needs from the adapter: the forced-
/// tool contract (same shape as the frozen v1 seam), where the row array
/// lives inside one sample payload, which fields identify "the same row"
/// across samples, and which fields must agree verbatim for a row to
/// publish at full confidence.
#[derive(Debug, Clone)]
pub struct ConsensusSpec {
    /// The forced-tool extraction contract.
    pub tool: DocumentToolSpec,
    /// JSON pointer to the row array within one sample payload, e.g.
    /// `"/rows"`.
    pub rows_pointer: String,
    /// Row-relative JSON pointers (e.g. `"/asset_raw"`) whose values
    /// compose a row's content identity across samples.
    pub key_fields: Vec<String>,
    /// Row-relative JSON pointers that must agree verbatim for a row to
    /// publish at full confidence.
    pub critical_fields: Vec<String>,
}

/// One model sample pass over a document.
#[derive(Debug, Clone)]
pub struct SamplePass {
    /// The model that produced this pass.
    pub model_id: String,
    /// The forced-tool output (already schema-validated, same as the v1
    /// seam, before this type exists).
    pub payload: Value,
    /// Raw API usage block (tokens) — threaded into `ExtractionStats` by a
    /// later task.
    pub usage: Value,
}

/// A row's content identity within one document: the canonical join of its
/// key-field values plus an occurrence index, so that two structurally
/// identical rows in the SAME document (e.g. two identical small buys) are
/// never merged into one.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RowKey(String, usize);

/// Derives a row's content key from its key-field values. `occurrence` is
/// the caller's 0-based count of how many prior rows in the SAME sample
/// already produced this same content key — pass `0` for the first row
/// with a given key, `1` for the second identical row, and so on. Two rows
/// that differ only in `occurrence` are never equal.
#[must_use]
pub fn row_key(row: &Value, key_fields: &[String], occurrence: usize) -> RowKey {
    RowKey(key_fields_content(row, key_fields), occurrence)
}

/// Joins a row's key-field values into one canonical content string — the
/// shared building block behind `row_key` and (Task 2's) `align`'s content
/// grouping. Row-relative JSON-pointer values that are absent are folded to
/// the literal string `"null"` (never panics on a missing field).
fn key_fields_content(row: &Value, key_fields: &[String]) -> String {
    key_fields
        .iter()
        .map(|pointer| {
            row.pointer(pointer)
                .map_or_else(|| "null".to_owned(), ToString::to_string)
        })
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn row(asset: &str, date: &str) -> Value {
        json!({
            "asset_raw": asset,
            "transaction_date_raw": date,
            "amount_raw": "$15,001 - $50,000",
            "transaction_type_raw": "P",
        })
    }

    #[test]
    fn row_key_ignores_non_key_fields_and_field_order() {
        let fields = vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()];
        let a = row("Apple Inc Common Stock", "4/17/2026");
        // Same key-field values, a different non-key field value
        // (amount_raw), and a different JSON object construction order.
        let b = json!({
            "transaction_type_raw": "S",
            "transaction_date_raw": "4/17/2026",
            "amount_raw": "$1,001 - $15,000",
            "asset_raw": "Apple Inc Common Stock",
        });
        assert_eq!(row_key(&a, &fields, 0), row_key(&b, &fields, 0));
    }

    #[test]
    fn identical_rows_in_one_document_get_distinct_occurrence_keys() {
        let fields = vec!["/asset_raw".to_owned(), "/transaction_date_raw".to_owned()];
        let a = row("Apple Inc Common Stock", "4/17/2026");
        let first = row_key(&a, &fields, 0);
        let second = row_key(&a, &fields, 1);
        assert_ne!(first, second, "two identical rows must never merge");
    }
}
