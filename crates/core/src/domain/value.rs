//! Money as an interval, always (design D8/§4.2): US bands, exact figures (`low == high`),
//! open-ended thresholds (`high == None`). Amounts are `rust_decimal::Decimal`,
//! serialized as decimal STRINGS — never floats (invariant 7).

use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::DomainError;
use crate::domain::enums::Currency;

/// A money interval `low ..= high` in `currency`. Exact figures are `low == high`;
/// `high == None` is an open-ended threshold (UK "over £70,000"). Construction is
/// the only door: `high < low` never exists — in memory or on the wire, because
/// the manual `Deserialize` impl funnels back through [`ValueInterval::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
pub struct ValueInterval {
    low: Decimal,
    high: Option<Decimal>,
    currency: Currency,
}

impl ValueInterval {
    /// Builds an interval, rejecting an inverted band.
    ///
    /// # Errors
    /// [`DomainError::InvertedInterval`] when `high` is `Some` and below `low`.
    pub fn new(
        low: Decimal,
        high: Option<Decimal>,
        currency: Currency,
    ) -> Result<Self, DomainError> {
        match high {
            Some(h) if h < low => Err(DomainError::InvertedInterval { low, high: h }),
            _ => Ok(Self {
                low,
                high,
                currency,
            }),
        }
    }

    /// Lower bound (or the exact amount when `low == high`).
    #[must_use]
    pub fn low(&self) -> Decimal {
        self.low
    }

    /// Upper bound; `None` means open-ended.
    #[must_use]
    pub fn high(&self) -> Option<Decimal> {
        self.high
    }

    /// ISO 4217 currency of both bounds.
    #[must_use]
    pub fn currency(&self) -> Currency {
        self.currency
    }

    /// Midpoint of the band; `None` when the interval is open-ended.
    #[must_use]
    pub fn midpoint(&self) -> Option<Decimal> {
        self.high.map(|h| (self.low + h) / Decimal::TWO)
    }
}

/// Wire shape; exists so deserialization re-runs the constructor rule.
#[derive(Deserialize)]
struct RawValueInterval {
    low: Decimal,
    high: Option<Decimal>,
    currency: Currency,
}

impl<'de> Deserialize<'de> for ValueInterval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawValueInterval::deserialize(deserializer)?;
        Self::new(raw.low, raw.high, raw.currency).map_err(serde::de::Error::custom)
    }
}

// domain/value.rs tests — money is rust_decimal, serialized as strings. Never floats.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;
    use crate::domain::enums::Currency;

    #[test]
    fn exact_value_is_low_eq_high() {
        let v = ValueInterval::new(dec!(5000.00), Some(dec!(5000.00)), Currency::EUR).unwrap();
        assert_eq!(v.low(), v.high().unwrap());
    }
    #[test]
    fn open_ended_threshold_high_is_none() {
        assert!(ValueInterval::new(dec!(70000.00), None, Currency::GBP).is_ok());
    }
    #[test]
    fn rejects_high_below_low() {
        assert!(ValueInterval::new(dec!(10.00), Some(dec!(5.00)), Currency::USD).is_err());
    }
    #[test]
    fn midpoint_of_us_band() {
        let v = ValueInterval::new(dec!(1001.00), Some(dec!(15000.00)), Currency::USD).unwrap();
        assert_eq!(v.midpoint().unwrap(), dec!(8000.50));
    }

    #[test]
    fn money_serializes_as_decimal_strings_never_floats() {
        let v = ValueInterval::new(dec!(1001.00), Some(dec!(15000.00)), Currency::USD).unwrap();
        let json = serde_json::to_value(v).unwrap();
        assert_eq!(json["low"], serde_json::json!("1001.00"));
        assert_eq!(json["high"], serde_json::json!("15000.00"));
        assert_eq!(json["currency"], serde_json::json!("USD"));
    }

    #[test]
    fn deserialization_enforces_the_constructor_rule() {
        // The constructor is the only door: JSON with high < low must not sneak past it.
        let r = serde_json::from_str::<ValueInterval>(
            r#"{"low":"10.00","high":"5.00","currency":"USD"}"#,
        );
        assert!(r.is_err());
    }
}
