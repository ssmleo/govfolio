//! Closed vocabularies of `disclosure_record` (design §4.2). Wire format is `snake_case`
//! (matching the SQL CHECK literals); Currency is ISO 4217 uppercase (`char(3)` in DDL).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// `record_type` CHECK: the four observation types, one table (design D1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    Transaction,
    Holding,
    Interest,
    ChangeNotification,
}

/// `side` CHECK: direction of a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Buy,
    Sell,
    Exchange,
}

/// `owner` CHECK: whose asset the record concerns. `self` is a Rust keyword,
/// hence the `Self_` variant with an explicit wire rename.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Owner {
    #[serde(rename = "self")]
    Self_,
    Spouse,
    Dependent,
    Joint,
    Unknown,
}

/// `verification_state` CHECK: two-stage publication states (design §4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    Unverified,
    Verified,
    Corrected,
    Disputed,
}

/// `asset_class` vocabulary (design §4.2 column comment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum AssetClass {
    Equity,
    Bond,
    Fund,
    Option,
    Crypto,
    Commodity,
    RealEstate,
    Private,
    Other,
}

/// `user_account.tier` CHECK (design §6.2 freemium table): the tier decides
/// freshness (free = 24h-delayed via the ONE visibility bound in
/// `core::query`) and the daily request quota. Default is `free`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Free,
    Pro,
    Data,
}

impl Tier {
    /// Real-time record visibility (design §6.2: the 24-hour delay is the
    /// only monetization lever — pro and data see records as we do).
    #[must_use]
    pub fn realtime(self) -> bool {
        matches!(self, Self::Pro | Self::Data)
    }
}

/// ISO 4217 currency code (`char(3)` in DDL), uppercase on the wire — `snake_case`
/// would mangle codes. Closed set for now; extend as regimes land (visible in the
/// schema snapshot).
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub enum Currency {
    EUR,
    GBP,
    USD,
    BRL,
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn wire_format_matches_sql_check_literals() {
        assert_eq!(
            serde_json::to_value(RecordType::ChangeNotification).unwrap(),
            json!("change_notification")
        );
        assert_eq!(serde_json::to_value(Side::Buy).unwrap(), json!("buy"));
        assert_eq!(serde_json::to_value(Owner::Self_).unwrap(), json!("self"));
        assert_eq!(
            serde_json::to_value(VerificationState::Unverified).unwrap(),
            json!("unverified")
        );
        assert_eq!(
            serde_json::to_value(AssetClass::RealEstate).unwrap(),
            json!("real_estate")
        );
        // ISO 4217 codes stay uppercase (snake_case would mangle "USD" into "u_s_d").
        assert_eq!(serde_json::to_value(Currency::USD).unwrap(), json!("USD"));
        assert_eq!(serde_json::to_value(Tier::Free).unwrap(), json!("free"));
        assert_eq!(serde_json::to_value(Tier::Data).unwrap(), json!("data"));
    }

    #[test]
    fn only_paid_tiers_are_realtime() {
        assert!(!Tier::Free.realtime());
        assert!(Tier::Pro.realtime());
        assert!(Tier::Data.realtime());
    }

    #[test]
    fn owner_self_round_trips() {
        let owner: Owner = serde_json::from_value(json!("self")).unwrap();
        assert_eq!(owner, Owner::Self_);
    }
}
