//! Strong `ETag`s everywhere (design §6.1): one middleware hashes every
//! successful GET body (sha256, quoted hex) and answers `If-None-Match` with
//! `304 Not Modified` — list and detail endpoints alike, no per-handler
//! bookkeeping. Bodies are our own bounded JSON pages, so buffering them to
//! hash is safe.

use axum::body::{Body, to_bytes};
use axum::extract::Request;
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse as _, Response};
use sha2::{Digest as _, Sha256};

use crate::error::ApiError;

/// Wraps GET handling: hashes 200 bodies into a strong `ETag` and serves
/// `304 Not Modified` (empty body, same `ETag`) when `If-None-Match` hits.
/// Non-GET methods and non-200 responses pass through untouched.
pub async fn etag(request: Request, next: Next) -> Response {
    let is_get = request.method() == Method::GET;
    let if_none_match = request
        .headers()
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let response = next.run(request).await;
    if !is_get || response.status() != StatusCode::OK {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    let bytes = match to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return ApiError::from(anyhow::anyhow!("buffering response body for ETag: {err}"))
                .into_response();
        }
    };
    let tag = format!("\"{}\"", hex_lower(&Sha256::digest(&bytes)));
    let Ok(header_value) = HeaderValue::from_str(&tag) else {
        // 66 ASCII characters — structurally a valid header value.
        return ApiError::from(anyhow::anyhow!("ETag {tag:?} is not a valid header value"))
            .into_response();
    };
    if if_none_match.is_some_and(|candidates| matches(&candidates, &tag)) {
        let mut not_modified = StatusCode::NOT_MODIFIED.into_response();
        not_modified
            .headers_mut()
            .insert(header::ETAG, header_value);
        return not_modified;
    }
    parts.headers.insert(header::ETAG, header_value);
    Response::from_parts(parts, Body::from(bytes))
}

/// Strong comparison of an `If-None-Match` header against the computed tag:
/// the wildcard, or any member of the comma-separated validator list.
fn matches(if_none_match: &str, tag: &str) -> bool {
    if_none_match.trim() == "*"
        || if_none_match
            .split(',')
            .any(|candidate| candidate.trim() == tag)
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

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn hex_lower_renders_sha256_as_64_hex_chars() {
        let hex = hex_lower(&Sha256::digest(b"govfolio"));
        assert_eq!(hex.len(), 64);
        assert!(hex.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')));
    }

    #[test]
    fn if_none_match_semantics() {
        let tag = "\"abc\"";
        assert!(matches("\"abc\"", tag));
        assert!(matches("*", tag));
        assert!(matches(" \"zzz\" , \"abc\"", tag), "validator lists");
        assert!(!matches("\"zzz\"", tag));
        assert!(!matches("abc", tag), "unquoted never strong-matches");
    }
}
