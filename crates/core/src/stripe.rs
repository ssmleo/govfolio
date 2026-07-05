//! Stripe webhook signature scheme (goal 050, design §6.4): pure
//! verification + minimal event parsing, no HTTP. Stripe signs
//! `"{t}.{payload}"` with the endpoint secret (HMAC-SHA256) and sends
//! `Stripe-Signature: t=<unix>,v1=<hex>[,v1=<hex>...]`; receivers recompute
//! and compare in constant time, and reject stale timestamps (replay
//! defense). Documented scheme — canned payloads signed with a test secret
//! exercise this end-to-end without any Stripe credentials on the host.
//!
//! The outbound Stripe client (create customer, checkout link, usage
//! reports) is the `worker::stripe::StripeClient` trait — config-gated,
//! mock-tested; this module is the inbound half both crates agree on.

use hmac::{Hmac, Mac as _};
use serde::Deserialize;
use sha2::Sha256;

/// Stripe's documented default tolerance for the signed timestamp.
pub const DEFAULT_TOLERANCE_SECS: i64 = 300;

/// Verification/parse failure — the webhook endpoint fails closed on any of
/// these (no unverified payload is ever acted on).
#[derive(Debug, thiserror::Error)]
pub enum StripeWebhookError {
    /// Header lacks `t=<unix seconds>`.
    #[error("Stripe-Signature header lacks a t=<timestamp> element")]
    MissingTimestamp,
    /// No `v1` signature matched the payload under the secret.
    #[error("no v1 signature matches the payload")]
    BadSignature,
    /// Timestamp outside the tolerance window (replay defense).
    #[error("signed timestamp is {age_secs}s from now (tolerance {tolerance_secs}s)")]
    StaleTimestamp {
        /// Absolute distance between the signed timestamp and now.
        age_secs: i64,
        /// The tolerance that was enforced.
        tolerance_secs: i64,
    },
    /// Verified payload is not a parseable Stripe event.
    #[error("verified payload is not a Stripe event: {0}")]
    Parse(#[from] serde_json::Error),
}

/// The slice of a Stripe event the mirror needs (everything else stays in
/// the raw payload; Stripe is the source of truth, we only mirror).
#[derive(Debug, Deserialize)]
pub struct StripeEvent {
    /// Stripe event id (`evt_...`).
    pub id: String,
    /// Event type, e.g. `customer.subscription.updated`.
    #[serde(rename = "type")]
    pub kind: String,
    /// The event's payload container.
    pub data: StripeEventData,
}

/// `data` container of a Stripe event.
#[derive(Debug, Deserialize)]
pub struct StripeEventData {
    /// The affected object (subscription, customer, ...), kept raw.
    pub object: serde_json::Value,
}

/// HMAC-SHA256 hex of `"{timestamp}.{payload}"` under `secret` — the value
/// Stripe puts in `v1`.
#[must_use]
pub fn signature_hex(secret: &str, timestamp: i64, payload: &str) -> String {
    // HMAC accepts keys of ANY length (RFC 2104) — the else arm is unreachable.
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        unreachable!("HMAC accepts any key length")
    };
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// Builds a complete `Stripe-Signature` header value — production shape,
/// used by the canned-payload tests (no Stripe account needed to prove the
/// verifier).
#[must_use]
pub fn signature_header(secret: &str, timestamp: i64, payload: &str) -> String {
    format!(
        "t={timestamp},v1={}",
        signature_hex(secret, timestamp, payload)
    )
}

/// Constant-time byte equality (length leaks; contents do not).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Verifies a `Stripe-Signature` header against the raw payload and parses
/// the event. `now_unix` is the receiver's clock (injected for testability).
///
/// # Errors
/// [`StripeWebhookError`] on a missing/stale timestamp, a signature mismatch,
/// or an unparseable (though verified) payload.
pub fn verify_and_parse(
    secret: &str,
    signature_header: &str,
    payload: &str,
    now_unix: i64,
) -> Result<StripeEvent, StripeWebhookError> {
    let mut timestamp: Option<i64> = None;
    let mut candidates: Vec<&str> = Vec::new();
    for element in signature_header.split(',') {
        match element.trim().split_once('=') {
            Some(("t", raw)) => timestamp = raw.parse().ok(),
            Some(("v1", hex)) => candidates.push(hex),
            _ => {} // unknown schemes (v0, ...) are ignored per Stripe docs
        }
    }
    let timestamp = timestamp.ok_or(StripeWebhookError::MissingTimestamp)?;
    let age_secs = (now_unix - timestamp).abs();
    if age_secs > DEFAULT_TOLERANCE_SECS {
        return Err(StripeWebhookError::StaleTimestamp {
            age_secs,
            tolerance_secs: DEFAULT_TOLERANCE_SECS,
        });
    }
    let expected = signature_hex(secret, timestamp, payload);
    if !candidates
        .iter()
        .any(|hex| ct_eq(hex.as_bytes(), expected.as_bytes()))
    {
        return Err(StripeWebhookError::BadSignature);
    }
    Ok(serde_json::from_str(payload)?)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const SECRET: &str = "whsec_test_secret";
    const PAYLOAD: &str =
        r#"{"id":"evt_1","type":"customer.subscription.updated","data":{"object":{"id":"sub_1"}}}"#;

    #[test]
    fn round_trips_a_signed_payload() {
        let header = signature_header(SECRET, 1_000_000, PAYLOAD);
        let event = verify_and_parse(SECRET, &header, PAYLOAD, 1_000_010).unwrap();
        assert_eq!(event.id, "evt_1");
        assert_eq!(event.kind, "customer.subscription.updated");
        assert_eq!(event.data.object["id"], serde_json::json!("sub_1"));
    }

    #[test]
    fn rejects_a_tampered_payload_and_a_wrong_secret() {
        let header = signature_header(SECRET, 1_000_000, PAYLOAD);
        let tampered = PAYLOAD.replace("sub_1", "sub_2");
        assert!(matches!(
            verify_and_parse(SECRET, &header, &tampered, 1_000_010),
            Err(StripeWebhookError::BadSignature)
        ));
        assert!(matches!(
            verify_and_parse("whsec_other", &header, PAYLOAD, 1_000_010),
            Err(StripeWebhookError::BadSignature)
        ));
    }

    #[test]
    fn rejects_stale_timestamps_both_directions() {
        let header = signature_header(SECRET, 1_000_000, PAYLOAD);
        assert!(matches!(
            verify_and_parse(SECRET, &header, PAYLOAD, 1_000_000 + 301),
            Err(StripeWebhookError::StaleTimestamp { .. })
        ));
        assert!(matches!(
            verify_and_parse(SECRET, &header, PAYLOAD, 1_000_000 - 301),
            Err(StripeWebhookError::StaleTimestamp { .. })
        ));
    }

    #[test]
    fn accepts_any_matching_v1_among_several() {
        // Stripe sends multiple v1 entries during secret rotation.
        let good = signature_hex(SECRET, 1_000_000, PAYLOAD);
        let header = format!("t=1000000,v1={},v1={good}", "0".repeat(64));
        assert!(verify_and_parse(SECRET, &header, PAYLOAD, 1_000_000).is_ok());
    }

    #[test]
    fn missing_timestamp_fails_closed() {
        let good = signature_hex(SECRET, 1_000_000, PAYLOAD);
        assert!(matches!(
            verify_and_parse(SECRET, &format!("v1={good}"), PAYLOAD, 1_000_000),
            Err(StripeWebhookError::MissingTimestamp)
        ));
    }
}
