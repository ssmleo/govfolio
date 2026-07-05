//! Outbound Stripe seam (goal 050, design §6.4): a trait so billing logic is
//! mock-tested end-to-end with ZERO Stripe credentials on hosts; the live
//! HTTP implementation is config-gated behind `STRIPE_SECRET_KEY` (Secret
//! Manager in deploy; never in code). The inbound half — webhook signature
//! verify + event parse — is `govfolio_core::stripe`, shared with the api
//! crate's `/v1/stripe/webhook`.

use async_trait::async_trait;

/// Meter event name for API usage (modern Stripe metering: billing meters +
/// meter events attributed to a customer). TODO(founder): must match the
/// meter configured in the Stripe dashboard when billing goes live.
pub const USAGE_METER_EVENT: &str = "govfolio_api_requests";

/// Identified UA (invariant 10 politeness applies to outbound calls too).
pub const USER_AGENT: &str = "govfolio-worker/0.1 (+https://govfolio.io)";

/// The three outbound calls the product needs. Small on purpose — Stripe is
/// mirrored, never wrapped.
#[async_trait]
pub trait StripeClient: Send + Sync {
    /// Creates a customer; returns the `cus_...` id (stored on
    /// `user_account.stripe_customer_id`).
    ///
    /// # Errors
    /// Transport or Stripe-side failure.
    async fn create_customer(&self, email: &str) -> anyhow::Result<String>;

    /// Creates a subscription checkout session for `price_id`, stamping
    /// `govfolio_tier` metadata (the webhook mirror reads it back); returns
    /// the hosted checkout URL.
    ///
    /// # Errors
    /// Transport or Stripe-side failure.
    async fn checkout_link(
        &self,
        customer_id: &str,
        price_id: &str,
        tier: &str,
    ) -> anyhow::Result<String>;

    /// Reports `quantity` usage for a customer as a meter event.
    /// `idempotency_key` is the `usage_report` ULID — resending after a crash
    /// dedups on Stripe's side, so usage is never double-billed.
    ///
    /// # Errors
    /// Transport or Stripe-side failure (the caller leaves the report
    /// unreported and retries next pass).
    async fn report_usage(
        &self,
        customer_id: &str,
        quantity: u64,
        idempotency_key: &str,
    ) -> anyhow::Result<()>;
}

/// Live client over the Stripe REST API (form-encoded, Bearer secret key).
pub struct HttpStripeClient {
    client: reqwest::Client,
    secret_key: String,
    base_url: String,
}

impl HttpStripeClient {
    /// Builds the live client when `STRIPE_SECRET_KEY` is present; `None`
    /// otherwise (callers fail closed — there is no "pretend" mode outside
    /// tests).
    ///
    /// # Errors
    /// Client construction failure.
    pub fn from_env() -> anyhow::Result<Option<Self>> {
        let Some(secret_key) = std::env::var("STRIPE_SECRET_KEY")
            .ok()
            .filter(|key| !key.is_empty())
        else {
            return Ok(None);
        };
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }
        let client = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| anyhow::anyhow!("building stripe client: {e}"))?;
        Ok(Some(Self {
            client,
            secret_key,
            base_url: "https://api.stripe.com".to_owned(),
        }))
    }

    async fn post_form(
        &self,
        path: &str,
        form: &[(String, String)],
        idempotency_key: Option<&str>,
    ) -> anyhow::Result<serde_json::Value> {
        let mut request = self
            .client
            .post(format!("{}{path}", self.base_url))
            .bearer_auth(&self.secret_key)
            .form(form);
        if let Some(key) = idempotency_key {
            request = request.header("Idempotency-Key", key);
        }
        let response = request
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("stripe POST {path}: {e}"))?;
        let status = response.status();
        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| anyhow::anyhow!("stripe POST {path}: non-JSON response: {e}"))?;
        if !status.is_success() {
            // Stripe error bodies are structured; keep the message, never
            // log secrets (none are in the response).
            anyhow::bail!(
                "stripe POST {path} returned {status}: {}",
                body["error"]["message"].as_str().unwrap_or("unknown error")
            );
        }
        Ok(body)
    }
}

#[async_trait]
impl StripeClient for HttpStripeClient {
    async fn create_customer(&self, email: &str) -> anyhow::Result<String> {
        let body = self
            .post_form(
                "/v1/customers",
                &[("email".to_owned(), email.to_owned())],
                None,
            )
            .await?;
        body["id"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("stripe customer response lacks id"))
    }

    async fn checkout_link(
        &self,
        customer_id: &str,
        price_id: &str,
        tier: &str,
    ) -> anyhow::Result<String> {
        let form = vec![
            ("mode".to_owned(), "subscription".to_owned()),
            ("customer".to_owned(), customer_id.to_owned()),
            ("line_items[0][price]".to_owned(), price_id.to_owned()),
            ("line_items[0][quantity]".to_owned(), "1".to_owned()),
            (
                "subscription_data[metadata][govfolio_tier]".to_owned(),
                tier.to_owned(),
            ),
        ];
        let body = self.post_form("/v1/checkout/sessions", &form, None).await?;
        body["url"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| anyhow::anyhow!("stripe checkout response lacks url"))
    }

    async fn report_usage(
        &self,
        customer_id: &str,
        quantity: u64,
        idempotency_key: &str,
    ) -> anyhow::Result<()> {
        let form = vec![
            ("event_name".to_owned(), USAGE_METER_EVENT.to_owned()),
            (
                "payload[stripe_customer_id]".to_owned(),
                customer_id.to_owned(),
            ),
            ("payload[value]".to_owned(), quantity.to_string()),
            // Meter-event identifier: Stripe dedups on it, making resends
            // after a crash-between-send-and-stamp harmless.
            ("identifier".to_owned(), idempotency_key.to_owned()),
        ];
        self.post_form("/v1/billing/meter_events", &form, Some(idempotency_key))
            .await?;
        Ok(())
    }
}
