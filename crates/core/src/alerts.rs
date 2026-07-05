//! Alert-rule channel contract (design §6.3): the typed shape of
//! `alert_rule.channels`. A rule carries AT MOST ONE channel per type — the
//! delivery dedup key is `(rule, event, channel-type)`, so a second channel
//! of the same type would silently collapse into one delivery; users who
//! want two webhooks create two rules.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// One delivery channel of an alert rule (`alert_rule.channels` element).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertChannel {
    /// Email delivery (instant or digest).
    Email {
        /// Destination address.
        to: String,
    },
    /// HMAC-SHA256-signed webhook (design §6.4): the dispatcher POSTs JSON
    /// and signs the exact body with `secret` in the
    /// `X-Govfolio-Signature: sha256=<hex>` header.
    Webhook {
        /// POST target URL.
        url: String,
        /// Per-rule shared secret for the HMAC signature.
        secret: String,
    },
}

impl AlertChannel {
    /// The channel-type token — the SAME closed vocabulary as the
    /// `delivery.channel` SQL CHECK and the serde `type` tag (one rule,
    /// three enforcers, one literal each).
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Email { .. } => "email",
            Self::Webhook { .. } => "webhook",
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn channels_round_trip_and_kind_matches_the_serde_tag() {
        let channels: Vec<AlertChannel> = serde_json::from_value(json!([
            { "type": "email", "to": "a@example.org" },
            { "type": "webhook", "url": "https://example.org/h", "secret": "s" },
        ]))
        .unwrap();
        for channel in &channels {
            let tag = serde_json::to_value(channel).unwrap()["type"]
                .as_str()
                .unwrap()
                .to_owned();
            assert_eq!(channel.kind(), tag, "kind() must equal the wire tag");
        }
    }

    #[test]
    fn unknown_channel_type_is_rejected() {
        assert!(
            serde_json::from_value::<AlertChannel>(json!({ "type": "sms", "to": "x" })).is_err()
        );
    }
}
