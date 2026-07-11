use std::sync::OnceLock;

use chrono::{DateTime, Duration, Utc};
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};

use super::MAX_STDERR_CLASSIFIER_BYTES;
use crate::model::{NormalizedResult, ResultClass};

const MAX_SESSION_CHARS: usize = 512;
const MAX_EVIDENCE_CHARS: usize = 4 * 1024;
const MAX_SUMMARY_CHARS: usize = 512;

#[derive(Debug, Default)]
pub(crate) struct StreamState {
    pub(crate) structured_started: bool,
    pub(crate) corrupt: bool,
    pub(crate) session_id: Option<String>,
    pub(crate) terminal: Option<TerminalEvidence>,
}

impl StreamState {
    pub(crate) fn parse_line(&mut self, line: &[u8]) -> Option<Value> {
        let trimmed = trim_ascii(line);
        if trimmed.is_empty() {
            return None;
        }
        self.structured_started = true;
        if let Ok(value @ Value::Object(_)) = serde_json::from_slice::<Value>(trimmed) {
            Some(value)
        } else {
            self.corrupt = true;
            None
        }
    }

    pub(crate) fn capture_session(&mut self, session_id: &str) {
        if !session_id.is_empty() {
            self.session_id = Some(take_chars(session_id, MAX_SESSION_CHARS));
        }
    }
}

#[derive(Debug)]
pub(crate) struct TerminalEvidence {
    terminal_type: String,
    completed: bool,
    code: Option<String>,
    message: String,
    retry_absolute: Option<DateTime<Utc>>,
    retry_after: Option<Duration>,
    status: Option<u16>,
}

pub(crate) fn terminal_success(terminal_type: impl Into<String>) -> TerminalEvidence {
    TerminalEvidence {
        terminal_type: terminal_type.into(),
        completed: true,
        code: None,
        message: "completed".to_owned(),
        retry_absolute: None,
        retry_after: None,
        status: None,
    }
}

pub(crate) fn terminal_error(
    terminal_type: impl Into<String>,
    value: &Value,
    fallback_code: Option<&str>,
) -> TerminalEvidence {
    let message = extract_message(value).unwrap_or_else(|| "provider terminal failure".to_owned());
    TerminalEvidence {
        terminal_type: terminal_type.into(),
        completed: false,
        code: extract_code(value).or_else(|| fallback_code.map(str::to_owned)),
        retry_absolute: extract_absolute_retry(value).or_else(|| extract_rfc3339(&message)),
        retry_after: extract_retry_after(value),
        status: extract_status(value),
        message: take_chars(&message, MAX_EVIDENCE_CHARS),
    }
}

pub(crate) fn finish_stream(
    stream: StreamState,
    exit_code: Option<i32>,
    bounded_stderr: &[u8],
    observed_at: DateTime<Utc>,
    operator_stopped: bool,
) -> NormalizedResult {
    if operator_stopped {
        return NormalizedResult {
            class: ResultClass::OperatorStop,
            terminal_type: None,
            structured_started: stream.structured_started,
            session_id: stream.session_id,
            provider_error_code: None,
            stable_error_hash: None,
            retry_at: None,
            exit_code,
            summary: "operator stopped provider process group".to_owned(),
        };
    }

    if let Some(terminal) = stream.terminal {
        return finish_terminal(
            terminal,
            stream.structured_started,
            stream.session_id,
            exit_code,
            observed_at,
        );
    }

    if stream.structured_started {
        let message = if stream.corrupt {
            "structured stdout was corrupt or truncated"
        } else {
            "structured stdout ended without a terminal event"
        };
        return ambiguous_result(
            message,
            true,
            stream.session_id,
            exit_code,
            Some(stable_error_hash(None, message)),
        );
    }

    let stderr = stderr_suffix(bounded_stderr);
    if stderr.is_empty() {
        return ambiguous_result(
            "provider exited without structured stdout or stderr evidence",
            false,
            stream.session_id,
            exit_code,
            None,
        );
    }

    let evidence = TerminalEvidence {
        terminal_type: "stderr_fallback".to_owned(),
        completed: false,
        code: None,
        message: tail_chars(&String::from_utf8_lossy(stderr), MAX_EVIDENCE_CHARS),
        retry_absolute: extract_rfc3339(&String::from_utf8_lossy(stderr)),
        retry_after: extract_retry_duration_from_text(&String::from_utf8_lossy(stderr)),
        status: extract_http_status_from_text(&String::from_utf8_lossy(stderr)),
    };
    finish_terminal(evidence, false, stream.session_id, exit_code, observed_at)
}

fn finish_terminal(
    terminal: TerminalEvidence,
    structured_started: bool,
    session_id: Option<String>,
    exit_code: Option<i32>,
    observed_at: DateTime<Utc>,
) -> NormalizedResult {
    if terminal.completed {
        return NormalizedResult {
            class: ResultClass::Completed,
            terminal_type: Some(terminal.terminal_type),
            structured_started,
            session_id,
            provider_error_code: None,
            stable_error_hash: None,
            retry_at: None,
            exit_code,
            summary: "completed".to_owned(),
        };
    }

    let class = classify_error(terminal.code.as_deref(), &terminal.message, terminal.status);
    let retry_at = terminal
        .retry_absolute
        .or_else(|| terminal.retry_after.map(|delay| observed_at + delay));
    let hash = stable_error_hash(terminal.code.as_deref(), &terminal.message);
    NormalizedResult {
        class,
        terminal_type: Some(terminal.terminal_type),
        structured_started,
        session_id,
        provider_error_code: terminal.code,
        stable_error_hash: Some(hash),
        retry_at,
        exit_code,
        summary: tail_chars(&terminal.message, MAX_SUMMARY_CHARS),
    }
}

fn ambiguous_result(
    summary: &str,
    structured_started: bool,
    session_id: Option<String>,
    exit_code: Option<i32>,
    stable_error_hash: Option<String>,
) -> NormalizedResult {
    NormalizedResult {
        class: ResultClass::Ambiguous,
        terminal_type: None,
        structured_started,
        session_id,
        provider_error_code: None,
        stable_error_hash,
        retry_at: None,
        exit_code,
        summary: summary.to_owned(),
    }
}

fn classify_error(code: Option<&str>, message: &str, status: Option<u16>) -> ResultClass {
    let identity = format!("{} {message}", code.unwrap_or_default()).to_ascii_lowercase();

    if is_quota(&identity) {
        ResultClass::QuotaExhausted
    } else if is_invalid_session(&identity) {
        ResultClass::SessionInvalid
    } else if is_auth(&identity, status) {
        ResultClass::Auth
    } else if is_policy(&identity) {
        ResultClass::Policy
    } else if is_runner_config(&identity) {
        ResultClass::RunnerConfig
    } else if is_rate_limited(&identity, status) {
        ResultClass::RateLimited
    } else if is_provider_unavailable(&identity, status) {
        ResultClass::ProviderUnavailable
    } else if is_transport(&identity) {
        ResultClass::TransientTransport
    } else {
        ResultClass::Ambiguous
    }
}

fn is_quota(identity: &str) -> bool {
    contains_any(
        identity,
        &[
            "usage_limit",
            "usage limit",
            "quota_exhausted",
            "quota exhausted",
            "insufficient_quota",
            "monthly limit",
            "monthly usage",
            "billing_hard_limit",
            "out of credits",
            "credit balance",
        ],
    )
}

fn is_invalid_session(identity: &str) -> bool {
    contains_any(
        identity,
        &[
            "session_not_found",
            "thread_not_found",
            "conversation_not_found",
            "invalid_session",
            "invalid session",
            "invalid thread",
            "resume session",
        ],
    )
}

fn is_auth(identity: &str, status: Option<u16>) -> bool {
    status == Some(401)
        || contains_any(
            identity,
            &[
                "authentication_error",
                "authentication failed",
                "invalid_api_key",
                "invalid api key",
                "incorrect api key",
                "unauthorized",
                "expired token",
            ],
        )
}

fn is_policy(identity: &str) -> bool {
    contains_any(
        identity,
        &[
            "error_max_budget",
            "error_max_turns",
            "maximum configured turn budget",
            "permission_denied",
            "permission denied",
            "policy violation",
            "blocked by policy",
            "sandbox violation",
            "approval required",
        ],
    )
}

fn is_runner_config(identity: &str) -> bool {
    contains_any(
        identity,
        &[
            "model_not_found",
            "unknown model",
            "requested model does not exist",
            "invalid configuration",
            "invalid argument",
            "unrecognized option",
            "error_max_structured_output_retries",
        ],
    )
}

fn is_rate_limited(identity: &str, status: Option<u16>) -> bool {
    status == Some(429)
        || contains_any(identity, &["rate_limit", "rate limit", "too many requests"])
}

fn is_provider_unavailable(identity: &str, status: Option<u16>) -> bool {
    contains_any(
        identity,
        &[
            "service_unavailable",
            "service unavailable",
            "provider unavailable",
            "model unavailable",
            "overloaded",
            "over capacity",
        ],
    ) || matches!(status, Some(502 | 503 | 529))
}

fn is_transport(identity: &str) -> bool {
    contains_any(
        identity,
        &[
            "api_connection_error",
            "connection reset",
            "connection refused",
            "connection timed out",
            "network error",
            "network unreachable",
            "tls connection",
            "dns error",
            "socket error",
            "unexpected eof",
        ],
    )
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn extract_code(value: &Value) -> Option<String> {
    [
        &["error", "code"][..],
        &["error", "type"][..],
        &["code"][..],
        &["error_code"][..],
        &["codex_error_info"][..],
        &["api_error_status"][..],
    ]
    .into_iter()
    .find_map(|path| value_at_path(value, path).and_then(value_to_short_string))
}

fn extract_message(value: &Value) -> Option<String> {
    for path in [&["error", "message"][..], &["message"][..], &["error"][..]] {
        if let Some(message) = value_at_path(value, path).and_then(Value::as_str)
            && !message.is_empty()
        {
            return Some(message.to_owned());
        }
    }

    value
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .filter_map(Value::as_str)
                .take(8)
                .collect::<Vec<_>>()
                .join("; ")
        })
        .filter(|message| !message.is_empty())
}

fn extract_status(value: &Value) -> Option<u16> {
    [
        &["error", "status_code"][..],
        &["error", "status"][..],
        &["status_code"][..],
        &["status"][..],
        &["api_error_status"][..],
    ]
    .into_iter()
    .find_map(|path| value_at_path(value, path).and_then(value_to_u16))
}

fn extract_absolute_retry(value: &Value) -> Option<DateTime<Utc>> {
    find_named_value(
        value,
        &[
            "reset_at",
            "reset_time",
            "retry_at",
            "retry_time",
            "next_retry_at",
        ],
    )
    .and_then(parse_datetime_value)
}

fn extract_retry_after(value: &Value) -> Option<Duration> {
    if let Some(value) = find_named_value(
        value,
        &["retry_after_ms", "retry_in_ms", "retry_after_milliseconds"],
    ) && let Some(milliseconds) = value_to_i64(value)
    {
        return Some(Duration::milliseconds(milliseconds.max(0)));
    }

    find_named_value(
        value,
        &["retry_after", "retry_after_seconds", "retry_in_seconds"],
    )
    .and_then(value_to_duration)
}

fn find_named_value<'a>(value: &'a Value, names: &[&str]) -> Option<&'a Value> {
    match value {
        Value::Object(map) => {
            for name in names {
                if let Some(found) = map.get(*name) {
                    return Some(found);
                }
            }
            map.values()
                .find_map(|nested| find_named_value(nested, names))
        }
        Value::Array(values) => values
            .iter()
            .find_map(|nested| find_named_value(nested, names)),
        _ => None,
    }
}

fn value_at_path<'a>(mut value: &'a Value, path: &[&str]) -> Option<&'a Value> {
    for key in path {
        value = value.get(*key)?;
    }
    Some(value)
}

fn value_to_short_string(value: &Value) -> Option<String> {
    match value {
        Value::String(text) if !text.is_empty() => Some(take_chars(text, 256)),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn value_to_u16(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|number| u16::try_from(number).ok())
        .or_else(|| value.as_str().and_then(|text| text.parse::<u16>().ok()))
}

fn value_to_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
}

fn value_to_duration(value: &Value) -> Option<Duration> {
    if let Some(seconds) = value_to_i64(value) {
        return Some(Duration::seconds(seconds.max(0)));
    }
    value.as_str().and_then(extract_retry_duration_from_text)
}

fn parse_datetime_value(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(text) = value.as_str() {
        return DateTime::parse_from_rfc3339(text)
            .ok()
            .map(|timestamp| timestamp.with_timezone(&Utc))
            .or_else(|| text.parse::<i64>().ok().and_then(datetime_from_integer));
    }
    value.as_i64().and_then(datetime_from_integer)
}

fn datetime_from_integer(value: i64) -> Option<DateTime<Utc>> {
    if value >= 1_000_000_000_000 {
        DateTime::from_timestamp_millis(value)
    } else if value >= 1_000_000_000 {
        DateTime::from_timestamp(value, 0)
    } else {
        None
    }
}

fn extract_rfc3339(text: &str) -> Option<DateTime<Utc>> {
    static TIMESTAMP: OnceLock<Option<Regex>> = OnceLock::new();
    let regex = TIMESTAMP
        .get_or_init(|| {
            Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})").ok()
        })
        .as_ref()?;
    regex.find(text).and_then(|capture| {
        DateTime::parse_from_rfc3339(capture.as_str())
            .ok()
            .map(|timestamp| timestamp.with_timezone(&Utc))
    })
}

fn extract_retry_duration_from_text(text: &str) -> Option<Duration> {
    static RETRY: OnceLock<Option<Regex>> = OnceLock::new();
    let regex = RETRY
        .get_or_init(|| {
            Regex::new(r"(?i)retry(?:ing)?\s+(?:after|in)\s+(\d+)\s*(ms|milliseconds?|s|seconds?|minutes?|m)?")
                .ok()
        })
        .as_ref()?;
    let captures = regex.captures(text)?;
    let amount = captures.get(1)?.as_str().parse::<i64>().ok()?;
    let unit = captures
        .get(2)
        .map_or("seconds", |capture| capture.as_str())
        .to_ascii_lowercase();
    if matches!(unit.as_str(), "ms" | "millisecond" | "milliseconds") {
        Some(Duration::milliseconds(amount))
    } else if matches!(unit.as_str(), "m" | "minute" | "minutes") {
        Some(Duration::minutes(amount))
    } else {
        Some(Duration::seconds(amount))
    }
}

fn extract_http_status_from_text(text: &str) -> Option<u16> {
    static STATUS: OnceLock<Option<Regex>> = OnceLock::new();
    let regex = STATUS
        .get_or_init(|| Regex::new(r"(?:^|\D)(401|429|502|503|529)(?:\D|$)").ok())
        .as_ref()?;
    regex
        .captures(text)
        .and_then(|captures| captures.get(1))
        .and_then(|capture| capture.as_str().parse::<u16>().ok())
}

/// Hashes stable error identity after removing volatile time and request IDs.
#[must_use]
pub fn stable_error_hash(code: Option<&str>, message: &str) -> String {
    let mut normalized = message.to_ascii_lowercase();
    normalized = replace_pattern(
        normalized,
        &RFC3339_PATTERN,
        r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})",
        "<time>",
    );
    normalized = replace_pattern(
        normalized,
        &REQUEST_ID_PATTERN,
        r"(?i)\b(?:req(?:uest)?|trace|session|thread)[-_=: ]+[a-z0-9_-]+\b",
        "<id>",
    );
    normalized = replace_pattern(
        normalized,
        &RESET_CLAUSE_PATTERN,
        r"(?i)\b(retry(?:ing)?\s+(?:after|in)|resets?(?:\s+at|\s+on)?|reset(?:_at)?[=:]?)\s+[^;,\n]+",
        "$1 <time>",
    );
    normalized = replace_pattern(
        normalized,
        &UUID_PATTERN,
        r"(?i)\b[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\b",
        "<id>",
    );
    normalized = replace_pattern(normalized, &EPOCH_PATTERN, r"\b\d{10,13}\b", "<time>");
    normalized = normalized.split_whitespace().collect::<Vec<_>>().join(" ");

    let mut digest = Sha256::new();
    digest.update(code.unwrap_or_default().to_ascii_lowercase().as_bytes());
    digest.update([0]);
    digest.update(normalized.as_bytes());
    hex::encode(digest.finalize())
}

static RFC3339_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
static REQUEST_ID_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
static RESET_CLAUSE_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
static UUID_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();
static EPOCH_PATTERN: OnceLock<Option<Regex>> = OnceLock::new();

fn replace_pattern(
    value: String,
    cache: &'static OnceLock<Option<Regex>>,
    pattern: &str,
    replacement: &str,
) -> String {
    let Some(regex) = cache.get_or_init(|| Regex::new(pattern).ok()) else {
        return value;
    };
    regex.replace_all(&value, replacement).into_owned()
}

fn stderr_suffix(stderr: &[u8]) -> &[u8] {
    let start = stderr.len().saturating_sub(MAX_STDERR_CLASSIFIER_BYTES);
    &stderr[start..]
}

fn trim_ascii(mut value: &[u8]) -> &[u8] {
    while value.first().is_some_and(u8::is_ascii_whitespace) {
        value = &value[1..];
    }
    while value.last().is_some_and(u8::is_ascii_whitespace) {
        value = &value[..value.len() - 1];
    }
    value
}

fn take_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn tail_chars(value: &str, max_chars: usize) -> String {
    let total = value.chars().count();
    value
        .chars()
        .skip(total.saturating_sub(max_chars))
        .collect()
}
