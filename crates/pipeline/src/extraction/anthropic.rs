//! Minimal Anthropic Messages API client for schema-constrained document
//! extraction (design §5.3) — reqwest + rustls(ring), no SDK dependency.
//!
//! The extraction contract is FORCED TOOL USE: the request carries exactly
//! one tool whose `input_schema` is the adapter's silver-row JSON Schema and
//! a `tool_choice` that forces it, so the model can only answer in schema
//! shape. The tool output is still re-validated locally against the same
//! schema — schema-invalid output fails closed (invariant 6), it never
//! becomes low-confidence Gold.
//!
//! High-impact documents are re-extracted by a second, distinct model and
//! compared field by field ([`CrossCheckMismatch`] on any difference).
//!
//! The API key lives only inside [`HttpTransport`] and is redacted from its
//! `Debug` output; it is never logged or echoed into errors.

use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use base64::Engine as _;
use serde_json::{Value, json};

/// Messages API endpoint.
const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
/// Pinned API version header value.
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// Output budget per extraction call (a PTR yields a small tool payload).
const MAX_TOKENS: u32 = 16_000;
/// Retries after the first attempt (429/5xx/transport, exponential backoff).
const MAX_RETRIES: u32 = 3;
/// First backoff delay; doubles per retry.
const BACKOFF_BASE: Duration = Duration::from_millis(500);

/// Default primary extraction model (cheap, vision-capable).
pub const DEFAULT_PRIMARY_MODEL: &str = "claude-haiku-4-5-20251001";
/// Default cross-check model — deliberately a DIFFERENT model family so the
/// second opinion is independent (design §5.3).
pub const DEFAULT_CROSSCHECK_MODEL: &str = "claude-sonnet-5";

/// Model configuration; overridable by environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Models {
    /// Primary extraction model (part of the cache key).
    pub primary: String,
    /// Second-opinion model for high-impact documents.
    pub crosscheck: String,
}

impl Models {
    /// Reads `GOVFOLIO_LLM_PRIMARY_MODEL` / `GOVFOLIO_LLM_CROSSCHECK_MODEL`,
    /// falling back to the defaults.
    #[must_use]
    pub fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Same resolution with an injectable lookup (deterministic tests).
    #[must_use]
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Self {
        Self {
            primary: lookup("GOVFOLIO_LLM_PRIMARY_MODEL")
                .unwrap_or_else(|| DEFAULT_PRIMARY_MODEL.to_owned()),
            crosscheck: lookup("GOVFOLIO_LLM_CROSSCHECK_MODEL")
                .unwrap_or_else(|| DEFAULT_CROSSCHECK_MODEL.to_owned()),
        }
    }
}

impl Default for Models {
    fn default() -> Self {
        Self::from_lookup(|_| None)
    }
}

/// The adapter-supplied extraction contract: one tool, one schema, one prompt.
#[derive(Debug, Clone)]
pub struct DocumentToolSpec {
    /// Tool name the model is forced to call.
    pub tool_name: String,
    /// Tool description (the model reads this as instructions).
    pub tool_description: String,
    /// The silver-row JSON Schema — the tool's `input_schema` AND the local
    /// re-validation schema.
    pub input_schema: Value,
    /// User-turn prompt accompanying the document.
    pub prompt: String,
}

/// Seam over the HTTP call so tests inject canned responses (goal 021
/// cross-check requirement). Production uses [`HttpTransport`].
#[async_trait]
pub trait Transport: Send + Sync {
    /// Sends one Messages API request body, returns the parsed response body.
    ///
    /// # Errors
    /// Transport or API failure.
    async fn send(&self, body: &Value) -> anyhow::Result<Value>;
}

#[async_trait]
impl<T: Transport + ?Sized> Transport for &T {
    async fn send(&self, body: &Value) -> anyhow::Result<Value> {
        (**self).send(body).await
    }
}

/// A transport-layer failure with retry classification.
#[derive(Debug)]
pub struct TransportError {
    /// True for 408/409/429/5xx and connection-level failures.
    pub retryable: bool,
    message: String,
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TransportError {}

/// Field-level disagreement between the primary and cross-check extractions:
/// the document freezes behind a review task, it never publishes silently.
#[derive(Debug)]
pub struct CrossCheckMismatch {
    /// JSON pointer-ish paths of the differing fields (first few).
    pub paths: Vec<String>,
    /// The two models that disagreed.
    pub models: (String, String),
}

impl std::fmt::Display for CrossCheckMismatch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "llm_crosscheck_mismatch: {} and {} disagree at {} — freeze + review_task (invariant 6)",
            self.models.0,
            self.models.1,
            self.paths.join(", ")
        )
    }
}

impl std::error::Error for CrossCheckMismatch {}

/// Real HTTPS transport. Holds the API key privately; `Debug` redacts it.
pub struct HttpTransport {
    client: reqwest::Client,
    api_key: String,
}

impl std::fmt::Debug for HttpTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpTransport")
            .field("api_key", &"<redacted>")
            .finish_non_exhaustive()
    }
}

impl HttpTransport {
    /// Builds the transport from `ANTHROPIC_API_KEY`.
    ///
    /// # Errors
    /// Missing key or client construction failure. The error never contains
    /// key material.
    pub fn from_env() -> anyhow::Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .map_err(|_| anyhow::anyhow!("ANTHROPIC_API_KEY is not set"))?;
        Self::new(api_key)
    }

    /// Builds the transport around an explicit key (tests, secret managers).
    ///
    /// # Errors
    /// TLS/client construction failure.
    pub fn new(api_key: String) -> anyhow::Result<Self> {
        // Same ring bootstrap as `PoliteClient` (reqwest `rustls-no-provider`).
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        let client = reqwest::Client::builder()
            .user_agent("govfolio-pipeline/0.1 (+https://govfolio.io)")
            .build()
            .context("building reqwest client")?;
        Ok(Self { client, api_key })
    }

    async fn post_once(&self, body: &Value) -> anyhow::Result<Value> {
        let payload = body.to_string(); // compact, infallible (Value::Display)
        let response = self
            .client
            .post(MESSAGES_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .await
            .map_err(|e| TransportError {
                retryable: true,
                // reqwest errors carry no request headers — no key material.
                message: format!("messages request failed: {e}"),
            })?;
        let status = response.status();
        let text = response.text().await.map_err(|e| TransportError {
            retryable: true,
            message: format!("reading messages response body: {e}"),
        })?;
        if !status.is_success() {
            let retryable = matches!(status.as_u16(), 408 | 409 | 429) || status.is_server_error();
            return Err(TransportError {
                retryable,
                message: format!("messages API {status}: {}", truncate(&text, 400)),
            }
            .into());
        }
        serde_json::from_str(&text).context("messages response is not JSON")
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn send(&self, body: &Value) -> anyhow::Result<Value> {
        with_backoff(MAX_RETRIES, BACKOFF_BASE, || self.post_once(body)).await
    }
}

/// Runs `op` with exponential backoff on retryable [`TransportError`]s:
/// up to `max_retries` retries after the first attempt, delay doubling from
/// `base`. Non-retryable errors surface immediately.
///
/// # Errors
/// The final attempt's error.
pub async fn with_backoff<T, F, Fut>(max_retries: u32, base: Duration, op: F) -> anyhow::Result<T>
where
    F: Fn() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    let mut attempt = 0u32;
    loop {
        match op().await {
            Ok(value) => return Ok(value),
            Err(error) => {
                let retryable = error
                    .downcast_ref::<TransportError>()
                    .is_some_and(|e| e.retryable);
                if !retryable || attempt >= max_retries {
                    return Err(error);
                }
                let delay = base * 2u32.saturating_pow(attempt);
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
        }
    }
}

/// Last-resort truncation for error surfaces (never for data).
fn truncate(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Schema-constrained document extractor: forced tool use + local schema
/// re-validation + second-model cross-check on impact (design §5.3).
#[derive(Debug)]
pub struct LlmDocumentExtractor<T: Transport> {
    transport: T,
    models: Models,
}

impl<T: Transport> LlmDocumentExtractor<T> {
    /// Wires an extractor.
    #[must_use]
    pub fn new(transport: T, models: Models) -> Self {
        Self { transport, models }
    }

    /// Extracts one PDF document into the tool's schema shape.
    ///
    /// Flow: primary-model call → local schema validation → `high_impact`
    /// predicate → (when true) cross-check-model call + field-level compare.
    /// Returns the agreed tool output.
    ///
    /// # Errors
    /// Transport/API failure, schema-invalid tool output (fail closed), a
    /// failing predicate, or [`CrossCheckMismatch`].
    pub async fn extract(
        &self,
        pdf_bytes: &[u8],
        spec: &DocumentToolSpec,
        high_impact: impl Fn(&Value) -> anyhow::Result<bool> + Send,
    ) -> anyhow::Result<Value> {
        let validator = jsonschema::validator_for(&spec.input_schema)
            .map_err(|e| anyhow::anyhow!("compiling extraction schema: {e}"))?;
        let primary = self.call(&self.models.primary, pdf_bytes, spec).await?;
        validate(&validator, &primary, &self.models.primary)?;
        if high_impact(&primary).context("evaluating high-impact predicate")? {
            let second = self.call(&self.models.crosscheck, pdf_bytes, spec).await?;
            validate(&validator, &second, &self.models.crosscheck)?;
            let paths = value_diff_paths(&primary, &second);
            if !paths.is_empty() {
                return Err(CrossCheckMismatch {
                    paths,
                    models: (self.models.primary.clone(), self.models.crosscheck.clone()),
                }
                .into());
            }
        }
        Ok(primary)
    }

    async fn call(
        &self,
        model: &str,
        pdf_bytes: &[u8],
        spec: &DocumentToolSpec,
    ) -> anyhow::Result<Value> {
        let request = build_request(model, pdf_bytes, spec);
        let response = self
            .transport
            .send(&request)
            .await
            .with_context(|| format!("messages call ({model})"))?;
        tool_use_input(&response, &spec.tool_name)
            .with_context(|| format!("messages response ({model})"))
    }
}

/// Builds the Messages API request body: base64 PDF document block + prompt,
/// one tool whose `input_schema` is the silver-row schema, forced
/// `tool_choice` (the schema-constrained extraction contract).
#[must_use]
pub fn build_request(model: &str, pdf_bytes: &[u8], spec: &DocumentToolSpec) -> Value {
    let data = base64::engine::general_purpose::STANDARD.encode(pdf_bytes);
    json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "document",
                    "source": {
                        "type": "base64",
                        "media_type": "application/pdf",
                        "data": data,
                    },
                },
                { "type": "text", "text": spec.prompt },
            ],
        }],
        "tools": [{
            "name": spec.tool_name,
            "description": spec.tool_description,
            "input_schema": spec.input_schema,
        }],
        "tool_choice": { "type": "tool", "name": spec.tool_name },
    })
}

/// Pulls the forced tool's `input` out of a Messages response; anything else
/// (refusal, plain text, wrong tool) is an error, never absorbed.
fn tool_use_input(response: &Value, tool_name: &str) -> anyhow::Result<Value> {
    let blocks = response
        .get("content")
        .and_then(Value::as_array)
        .context("response has no content array")?;
    for block in blocks {
        if block.get("type").and_then(Value::as_str) == Some("tool_use")
            && block.get("name").and_then(Value::as_str) == Some(tool_name)
        {
            return block
                .get("input")
                .cloned()
                .context("tool_use block has no input");
        }
    }
    let stop_reason = response
        .get("stop_reason")
        .and_then(Value::as_str)
        .unwrap_or("<none>");
    anyhow::bail!(
        "no {tool_name:?} tool_use block in the response (stop_reason {stop_reason:?}) — \
         fail closed (invariant 6)"
    )
}

/// Local re-validation of the tool output against the extraction schema:
/// forced tool use constrains generation, this check makes it a guarantee.
fn validate(validator: &jsonschema::Validator, output: &Value, model: &str) -> anyhow::Result<()> {
    let problems: Vec<String> = validator
        .iter_errors(output)
        .map(|err| format!("`{}`: {err}", err.instance_path()))
        .collect();
    anyhow::ensure!(
        problems.is_empty(),
        "{model} tool output violates the extraction schema — fail closed (invariant 6): {}",
        problems.join("; ")
    );
    Ok(())
}

/// Recursive field-level diff: paths where the two extractions disagree
/// (capped — the first differences are the evidence, not an exhaustive list).
#[must_use]
pub fn value_diff_paths(a: &Value, b: &Value) -> Vec<String> {
    const CAP: usize = 8;
    let mut paths = Vec::new();
    diff_into(a, b, "$", &mut paths, CAP);
    paths
}

fn diff_into(a: &Value, b: &Value, at: &str, out: &mut Vec<String>, cap: usize) {
    if out.len() >= cap || a == b {
        return;
    }
    match (a, b) {
        (Value::Object(left), Value::Object(right)) => {
            let keys: std::collections::BTreeSet<&String> =
                left.keys().chain(right.keys()).collect();
            for key in keys {
                match (left.get(key.as_str()), right.get(key.as_str())) {
                    (Some(x), Some(y)) => diff_into(x, y, &format!("{at}.{key}"), out, cap),
                    _ => push_capped(out, format!("{at}.{key}"), cap),
                }
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                push_capped(out, format!("{at}.length"), cap);
                return;
            }
            for (index, (x, y)) in left.iter().zip(right).enumerate() {
                diff_into(x, y, &format!("{at}[{index}]"), out, cap);
            }
        }
        _ => push_capped(out, at.to_owned(), cap),
    }
}

fn push_capped(out: &mut Vec<String>, path: String, cap: usize) {
    if out.len() < cap {
        out.push(path);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn models_default_and_env_override_resolve() {
        let defaults = Models::from_lookup(|_| None);
        assert_eq!(defaults.primary, DEFAULT_PRIMARY_MODEL);
        assert_eq!(defaults.crosscheck, DEFAULT_CROSSCHECK_MODEL);
        assert_ne!(
            defaults.primary, defaults.crosscheck,
            "cross-check must be a DISTINCT second model (design §5.3)"
        );
        let overridden = Models::from_lookup(|name| match name {
            "GOVFOLIO_LLM_PRIMARY_MODEL" => Some("model-a".to_owned()),
            "GOVFOLIO_LLM_CROSSCHECK_MODEL" => Some("model-b".to_owned()),
            _ => None,
        });
        assert_eq!(overridden.primary, "model-a");
        assert_eq!(overridden.crosscheck, "model-b");
    }

    #[test]
    fn http_transport_debug_redacts_the_api_key() {
        let transport = HttpTransport::new("sk-ant-SECRET-MATERIAL".to_owned()).unwrap();
        let debug = format!("{transport:?}");
        assert!(!debug.contains("SECRET"), "key leaked into Debug: {debug}");
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn value_diff_reports_field_level_paths() {
        let a = serde_json::json!({"rows": [{"amount_raw": "$15,001 - $50,000"}], "x": 1});
        let b = serde_json::json!({"rows": [{"amount_raw": "$50,001 - $100,000"}], "x": 1});
        assert_eq!(value_diff_paths(&a, &b), ["$.rows[0].amount_raw"]);
        assert!(value_diff_paths(&a, &a).is_empty());
        let c = serde_json::json!({"rows": [], "x": 1});
        assert_eq!(value_diff_paths(&a, &c), ["$.rows.length"]);
    }

    #[tokio::test(start_paused = true)]
    async fn backoff_retries_retryable_errors_then_gives_up() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let attempts = AtomicU32::new(0);
        let started = tokio::time::Instant::now();
        let result: anyhow::Result<()> = with_backoff(2, Duration::from_millis(100), || {
            attempts.fetch_add(1, Ordering::SeqCst);
            async {
                Err(TransportError {
                    retryable: true,
                    message: "messages API 529: overloaded".to_owned(),
                }
                .into())
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 3, "initial + 2 retries");
        // 100ms + 200ms of backoff elapsed (paused clock auto-advances).
        assert!(started.elapsed() >= Duration::from_millis(300));
    }

    #[tokio::test]
    async fn backoff_does_not_retry_terminal_errors() {
        use std::sync::atomic::{AtomicU32, Ordering};
        let attempts = AtomicU32::new(0);
        let result: anyhow::Result<()> = with_backoff(3, Duration::from_millis(1), || {
            attempts.fetch_add(1, Ordering::SeqCst);
            async {
                Err(TransportError {
                    retryable: false,
                    message: "messages API 400: invalid_request_error".to_owned(),
                }
                .into())
            }
        })
        .await;
        assert!(result.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1, "no retry on 4xx");
    }
}
