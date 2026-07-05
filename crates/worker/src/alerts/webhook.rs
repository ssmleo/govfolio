//! Webhook channel: HMAC-SHA256-signed JSON POSTs (design §6.4). The trait
//! seam sits UNDER the signing, so signature construction is production code
//! exercised by the mock-transport tests.

use async_trait::async_trait;
use hmac::{Hmac, Mac as _};
use sha2::Sha256;

use crate::alerts::SendError;

/// Signature header carried on every webhook POST.
pub const SIGNATURE_HEADER: &str = "x-govfolio-signature";

/// Identified UA (invariant 10 politeness applies to outbound calls too).
pub const USER_AGENT: &str = "govfolio-worker/0.1 (+https://govfolio.io)";

/// Request timeout.
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// One fully prepared webhook POST: exact body bytes + their signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebhookRequest {
    /// POST target (from the rule's channel config).
    pub url: String,
    /// The exact JSON body — the signature covers these bytes.
    pub body: String,
    /// `sha256=<hex>` HMAC over `body` with the rule's per-channel secret.
    pub signature: String,
}

/// `sha256=<hex>` HMAC-SHA256 of `body` under `secret` — the value receivers
/// recompute to authenticate the POST.
#[must_use]
pub fn signature_hex(secret: &str, body: &str) -> String {
    // HMAC accepts keys of ANY length (RFC 2104: long keys are hashed,
    // short keys padded) — the else arm cannot be reached.
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        unreachable!("HMAC accepts any key length")
    };
    mac.update(body.as_bytes());
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

/// Transport seam: production POSTs over HTTPS, tests capture requests.
#[async_trait]
pub trait WebhookTransport: Send + Sync {
    /// Delivers one signed request.
    ///
    /// # Errors
    /// [`SendError`] with retry classification.
    async fn post(&self, request: &WebhookRequest) -> anyhow::Result<()>;
}

/// Production transport: reqwest + rustls(ring), identified UA, hard timeout.
pub struct HttpWebhookTransport {
    client: reqwest::Client,
}

impl HttpWebhookTransport {
    /// Builds the client (installs the ring crypto provider if none is set —
    /// same pattern as the pipeline's extraction client).
    ///
    /// # Errors
    /// Client construction failure.
    pub fn new() -> anyhow::Result<Self> {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(TIMEOUT)
            .build()
            .map_err(|e| anyhow::anyhow!("building webhook client: {e}"))?;
        Ok(Self { client })
    }
}

#[async_trait]
impl WebhookTransport for HttpWebhookTransport {
    async fn post(&self, request: &WebhookRequest) -> anyhow::Result<()> {
        let response = self
            .client
            .post(&request.url)
            .header("content-type", "application/json")
            .header(SIGNATURE_HEADER, &request.signature)
            .body(request.body.clone())
            .send()
            .await
            .map_err(|e| SendError {
                retryable: true,
                message: format!("webhook POST failed: {e}"),
            })?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        Err(SendError {
            retryable: matches!(status.as_u16(), 408 | 429) || status.is_server_error(),
            message: format!("webhook endpoint returned {status}"),
        }
        .into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alerts_signature_matches_the_rfc_test_vector() {
        // RFC 4231-adjacent known vector (HMAC-SHA256, key "key").
        assert_eq!(
            signature_hex("key", "The quick brown fox jumps over the lazy dog"),
            "sha256=f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"
        );
    }

    #[test]
    fn alerts_signature_depends_on_secret_and_body() {
        let base = signature_hex("s1", "body");
        assert_ne!(base, signature_hex("s2", "body"));
        assert_ne!(base, signature_hex("s1", "body2"));
    }
}
