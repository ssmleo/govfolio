//! `GoldCandidate`: the pre-insert shape of a `disclosure_record` row (design §4.2).
//! One rule, two enforcers — `validate()` mirrors the SQL CHECK constraints exactly.

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::domain::DomainError;
use crate::domain::enums::{AssetClass, Owner, RecordType, Side};
use crate::domain::value::ValueInterval;
use crate::ids::{FilingId, InstrumentId, PoliticianId, RegimeId};

/// A `disclosure_record` candidate ready for Gold promotion (design §4.2).
/// Omits what Postgres owns: `id`, `event_date` (generated), `verification_state`
/// (defaults `unverified`), `supersedes_record_id`, `created_at`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GoldCandidate {
    /// Source filing this record came from.
    pub filing_id: FilingId,
    /// Denormalized politician (hot path).
    pub politician_id: PoliticianId,
    /// Denormalized disclosure regime.
    pub regime_id: RegimeId,
    /// Resolved instrument — `None` below threshold, never guessed (invariant 3).
    pub instrument_id: Option<InstrumentId>,
    /// Asset description exactly as filed, always kept (invariant 2).
    pub asset_description_raw: String,
    /// One of the four observation types.
    pub record_type: RecordType,
    /// Asset class vocabulary.
    pub asset_class: AssetClass,
    /// Transaction direction; required when `record_type == transaction`.
    pub side: Option<Side>,
    /// Date the transaction happened; required when `record_type == transaction`.
    pub transaction_date: Option<NaiveDate>,
    /// Snapshot date; required when `record_type == holding`.
    pub as_of_date: Option<NaiveDate>,
    /// Date the interest/change was notified.
    pub notified_date: Option<NaiveDate>,
    /// Declared value band; bounds are decimal strings on the wire (invariant 7).
    pub value: Option<ValueInterval>,
    /// Whose asset (self/spouse/...).
    pub owner: Option<Owner>,
    /// Extractor confidence in `[0, 1]` (`real` in DDL; not money).
    pub extraction_confidence: Option<f32>,
    /// Parser id / model+prompt version that produced this candidate.
    pub extracted_by: String,
    /// Idempotency fingerprint; computed at promotion (plan Task 6), hence optional here.
    pub fingerprint: Option<String>,
    /// Contract-typed payload, validated per (regime, `record_type`) schema (invariant 5).
    pub details: serde_json::Value,
}

impl GoldCandidate {
    /// Mirrors the `disclosure_record` CHECK constraints — one rule, two enforcers.
    /// The third CHECK (`value_high >= value_low`) is enforced by construction:
    /// an inverted [`ValueInterval`] cannot exist.
    ///
    /// # Errors
    /// [`DomainError::TypeRequires`] when a per-type required column is missing.
    pub fn validate(&self) -> Result<(), DomainError> {
        let require = |field: &'static str, present: bool| {
            if present {
                Ok(())
            } else {
                Err(DomainError::TypeRequires {
                    record_type: self.record_type,
                    field,
                })
            }
        };
        match self.record_type {
            RecordType::Transaction => {
                require("side", self.side.is_some())?;
                require("transaction_date", self.transaction_date.is_some())?;
            }
            RecordType::Holding => require("as_of_date", self.as_of_date.is_some())?,
            RecordType::Interest | RecordType::ChangeNotification => {}
        }
        Ok(())
    }
}

// domain/gold.rs tests — the cross-regime pair from the design doc, verbatim.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;

    use super::*;
    use crate::domain::DomainError;
    use crate::domain::enums::{AssetClass, Currency, Owner, RecordType, Side};
    use crate::domain::value::ValueInterval;

    /// US House PTR row: transaction, buy, 2026-03-02, 1001–15000 USD, spouse.
    fn us_ptr_fixture() -> GoldCandidate {
        GoldCandidate {
            filing_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse().unwrap(),
            politician_id: "01BX5ZZKBKACTAV9WEVGEMMVRZ".parse().unwrap(),
            regime_id: "01BX5ZZKBKACTAV9WEVGEMMVS0".parse().unwrap(),
            instrument_id: None, // never guess (invariant 3)
            asset_description_raw: "Microsoft Corporation - Common Stock (MSFT)".to_owned(),
            record_type: RecordType::Transaction,
            asset_class: AssetClass::Equity,
            side: Some(Side::Buy),
            transaction_date: Some(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()),
            as_of_date: None,
            notified_date: None,
            value: Some(
                ValueInterval::new(dec!(1001.00), Some(dec!(15000.00)), Currency::USD).unwrap(),
            ),
            owner: Some(Owner::Spouse),
            extraction_confidence: Some(0.99),
            extracted_by: "fixture:test@0".to_owned(),
            fingerprint: None,
            details: serde_json::json!({}),
        }
    }

    /// UK register-of-interests row: interest, notified 2026-04-10, 70000–open GBP.
    fn uk_interest_fixture() -> GoldCandidate {
        GoldCandidate {
            filing_id: "01BX5ZZKBKACTAV9WEVGEMMVS1".parse().unwrap(),
            politician_id: "01BX5ZZKBKACTAV9WEVGEMMVS2".parse().unwrap(),
            regime_id: "01BX5ZZKBKACTAV9WEVGEMMVS3".parse().unwrap(),
            instrument_id: None,
            asset_description_raw: "Shareholding: XYZ Holdings Ltd (above registrable threshold)"
                .to_owned(),
            record_type: RecordType::Interest,
            asset_class: AssetClass::Equity,
            side: None,
            transaction_date: None,
            as_of_date: None,
            notified_date: Some(NaiveDate::from_ymd_opt(2026, 4, 10).unwrap()),
            value: Some(ValueInterval::new(dec!(70000.00), None, Currency::GBP).unwrap()),
            owner: None,
            extraction_confidence: Some(0.99),
            extracted_by: "fixture:test@0".to_owned(),
            fingerprint: None,
            details: serde_json::json!({}),
        }
    }

    #[test]
    fn accepts_us_ptr_transaction_and_uk_interest_rejects_sideless_transaction() {
        us_ptr_fixture().validate().unwrap(); // transaction: buy, 2026-03-02, 1001–15000 USD, spouse
        uk_interest_fixture().validate().unwrap(); // interest: notified 2026-04-10, 70000–open GBP
        let mut bad = us_ptr_fixture();
        bad.side = None;
        assert!(matches!(
            bad.validate(),
            Err(DomainError::TypeRequires { .. })
        ));
    }

    #[test]
    fn transaction_without_transaction_date_is_rejected() {
        let mut bad = us_ptr_fixture();
        bad.transaction_date = None;
        assert!(matches!(
            bad.validate(),
            Err(DomainError::TypeRequires { .. })
        ));
    }

    #[test]
    fn holding_requires_as_of_date() {
        let mut holding = us_ptr_fixture();
        holding.record_type = RecordType::Holding;
        holding.side = None;
        holding.transaction_date = None;
        assert!(matches!(
            holding.validate(),
            Err(DomainError::TypeRequires { .. })
        ));
        holding.as_of_date = Some(NaiveDate::from_ymd_opt(2026, 5, 1).unwrap());
        holding.validate().unwrap();
    }

    #[test]
    fn round_trips_through_contract_json() {
        let orig = us_ptr_fixture();
        let json = serde_json::to_value(&orig).unwrap();
        assert_eq!(json["record_type"], "transaction");
        assert_eq!(json["value"]["low"], "1001.00"); // invariant 7: decimal strings
        assert_eq!(json["owner"], "spouse");
        let back: GoldCandidate = serde_json::from_value(json).unwrap();
        assert_eq!(back, orig);
    }
}
