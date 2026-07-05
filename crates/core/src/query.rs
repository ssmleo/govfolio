//! The ONE record filter grammar (design §6.3): shared by `/v1/records`, the
//! UI and `alert_rule.filter` — learned once, tested once. This module is the
//! single evaluator: [`RecordFilter::SQL_WHERE`] is the only SQL rendering of
//! the grammar, and the `bind_*` helpers are the only way values reach it.
//!
//! Contract surface: the serde JSON form of [`RecordFilter`] IS the
//! `alert_rule.filter` contract — snapshot-committed at
//! `crates/core/schemas/record_filter.json` (invariant 5 discipline). The
//! schema forbids unknown keys (`additionalProperties: false`) so a typo'd
//! filter fails at the API door instead of silently matching everything;
//! serde itself stays lenient because the same struct doubles as the
//! `/v1/records` query-string extractor (which also carries `cursor`/`limit`).

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::Postgres;
use sqlx::postgres::PgArguments;

use crate::domain::enums::{AssetClass, RecordType, VerificationState};
use crate::ids::{InstrumentId, PoliticianId};

/// A conjunctive filter over `disclosure_record` (design §6.1 filter list:
/// jurisdiction, type, `asset_class`, instrument, politician, date range,
/// value bounds, `verification_state`). Every field is optional; an empty
/// filter matches everything.
///
/// Value-bound semantics: a record matches when its declared band
/// `[value_low, value_high|∞]` OVERLAPS `[value_min, value_max]`; records
/// without a declared value never match a value bound.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema, utoipa::IntoParams))]
#[cfg_attr(feature = "utoipa", into_params(parameter_in = Query))]
#[schemars(extend("additionalProperties" = false))]
pub struct RecordFilter {
    /// Only records filed under a regime of this jurisdiction
    /// (`jurisdiction.id`, e.g. `us`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jurisdiction_id: Option<String>,
    /// Only records of this type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record_type: Option<RecordType>,
    /// Only records of this asset class.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset_class: Option<AssetClass>,
    /// Only records concerning this politician.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub politician_id: Option<PoliticianId>,
    /// Only records resolved to this instrument (below-threshold matches are
    /// NULL and never match — invariant 3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instrument_id: Option<InstrumentId>,
    /// Only records with `event_date` on or after this date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_date_from: Option<NaiveDate>,
    /// Only records with `event_date` on or before this date.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_date_to: Option<NaiveDate>,
    /// Only records whose declared value band can reach this amount or more
    /// (decimal string — invariant 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_min: Option<Decimal>,
    /// Only records whose declared value band can stay at this amount or less
    /// (decimal string — invariant 7).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value_max: Option<Decimal>,
    /// Only records in this verification state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verification_state: Option<VerificationState>,
    /// INTERNAL visibility bound — the freemium 24h delay (design §6.2), the
    /// tier's ONE lever. Only records whose filing we discovered at or before
    /// this instant match (`filing.discovered_at` = our knowledge time,
    /// honestly — `published_at` is the government's clock and often absent;
    /// the promise "free is 24h behind US" is only enforceable on OUR clock).
    /// `#[serde(skip)]`: never part of the grammar contract — callers cannot
    /// request it, alert rules cannot store it; the API layer sets it from
    /// the authenticated tier via [`Self::with_max_discovered_at`].
    #[serde(skip)]
    max_discovered_at: Option<DateTime<Utc>>,
}

/// Grammar evaluation failure: a closed-vocabulary value refused to render as
/// its SQL wire token (internal misuse — the vocabularies are all strings).
#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    /// The serialized token was not a plain JSON string.
    #[error("closed-vocabulary filter value must serialize to a string token, got {got}")]
    NonStringToken {
        /// What it serialized to instead.
        got: String,
    },
    /// Serde failure while rendering the token.
    #[error("serializing wire token: {0}")]
    Serialize(#[from] serde_json::Error),
}

/// Serializes a closed-vocabulary value to its SQL CHECK wire token (the same
/// one rule the writers enforce — one rule, two enforcers).
///
/// # Errors
/// [`QueryError`] when the value does not serialize to a plain string.
pub fn wire_token<T: Serialize>(value: &T) -> Result<String, QueryError> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(QueryError::NonStringToken {
            got: other.to_string(),
        }),
    }
}

/// Binds the eleven grammar slots in order — shared by both `bind_*` helpers
/// so the slot order exists exactly once.
macro_rules! bind_slots {
    ($self:ident, $query:ident) => {
        Ok($query
            .bind($self.jurisdiction_id.clone())
            .bind($self.record_type.as_ref().map(wire_token).transpose()?)
            .bind($self.asset_class.as_ref().map(wire_token).transpose()?)
            .bind($self.politician_id.map(|id| id.to_string()))
            .bind($self.instrument_id.map(|id| id.to_string()))
            .bind($self.event_date_from)
            .bind($self.event_date_to)
            .bind($self.value_min)
            .bind($self.value_max)
            .bind(
                $self
                    .verification_state
                    .as_ref()
                    .map(wire_token)
                    .transpose()?,
            )
            .bind($self.max_discovered_at))
    };
}

impl RecordFilter {
    /// Number of bind slots [`Self::SQL_WHERE`] consumes: the grammar owns
    /// `$1..=$11`; caller-specific binds MUST start at `$12`.
    pub const BIND_SLOTS: u16 = 11;

    /// The single SQL evaluation of the grammar — a conjunction over
    /// `disclosure_record` with fixed placeholder slots `$1..=$11` (each
    /// `NULL` bind disables its clause). A compile-time `&'static str`, so
    /// sqlx's `SqlSafeStr` injection guarantee holds structurally; compose it
    /// with `const_format::concatcp!` at call sites.
    ///
    /// `$11` is the internal freshness bound (design §6.2 free-tier delay):
    /// it lives INSIDE the one evaluator so no record-serving query can
    /// forget it — a route that composes `SQL_WHERE` gets the delay for free
    /// once the api layer sets the bound from the tier.
    pub const SQL_WHERE: &'static str = "($1::text is null or exists (select 1 from disclosure_regime \
           where disclosure_regime.id = disclosure_record.regime_id \
             and disclosure_regime.jurisdiction_id = $1)) \
       and ($2::text is null or record_type = $2) \
       and ($3::text is null or asset_class = $3) \
       and ($4::text is null or politician_id = $4) \
       and ($5::text is null or instrument_id = $5) \
       and ($6::date is null or event_date >= $6) \
       and ($7::date is null or event_date <= $7) \
       and ($8::numeric is null or (value_low is not null \
            and (value_high is null or value_high >= $8))) \
       and ($9::numeric is null or (value_low is not null and value_low <= $9)) \
       and ($10::text is null or verification_state = $10) \
       and ($11::timestamptz is null or exists (select 1 from filing \
            where filing.id = disclosure_record.filing_id \
              and filing.discovered_at <= $11))";

    /// Sets the internal visibility bound (the free-tier 24h delay). `None`
    /// means real-time. API-layer only — this never round-trips through the
    /// serde contract.
    #[must_use]
    pub fn with_max_discovered_at(mut self, bound: Option<DateTime<Utc>>) -> Self {
        self.max_discovered_at = bound;
        self
    }

    /// Scopes the filter to one politician (struct-update syntax is closed
    /// off by the private visibility slot; this is the ergonomic door).
    #[must_use]
    pub fn with_politician(mut self, id: PoliticianId) -> Self {
        self.politician_id = Some(id);
        self
    }

    /// Binds the grammar slots (`$1..=$10`, in slot order) on a `query_as`.
    /// MUST be called before any caller-specific binds.
    ///
    /// # Errors
    /// [`QueryError`] on a token that does not render as a string
    /// (structurally impossible for the closed vocabularies).
    pub fn bind_query_as<'q, O>(
        &self,
        query: sqlx::query::QueryAs<'q, Postgres, O, PgArguments>,
    ) -> Result<sqlx::query::QueryAs<'q, Postgres, O, PgArguments>, QueryError> {
        bind_slots!(self, query)
    }

    /// Binds the grammar slots (`$1..=$10`, in slot order) on a
    /// `query_scalar`. MUST be called before any caller-specific binds.
    ///
    /// # Errors
    /// [`QueryError`] on a token that does not render as a string
    /// (structurally impossible for the closed vocabularies).
    pub fn bind_query_scalar<'q, O>(
        &self,
        query: sqlx::query::QueryScalar<'q, Postgres, O, PgArguments>,
    ) -> Result<sqlx::query::QueryScalar<'q, Postgres, O, PgArguments>, QueryError> {
        bind_slots!(self, query)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn empty_filter_round_trips_as_empty_object() {
        let filter: RecordFilter = serde_json::from_value(json!({})).unwrap();
        assert_eq!(filter, RecordFilter::default());
        assert_eq!(serde_json::to_value(&filter).unwrap(), json!({}));
    }

    #[test]
    fn full_filter_round_trips_with_decimal_strings() {
        let value = json!({
            "jurisdiction_id": "us",
            "record_type": "transaction",
            "asset_class": "equity",
            "politician_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "instrument_id": "01ARZ3NDEKTSV4RRFFQ69G5FAW",
            "event_date_from": "2026-01-01",
            "event_date_to": "2026-06-30",
            "value_min": "1001.00",
            "value_max": "15000.00",
            "verification_state": "unverified",
        });
        let filter: RecordFilter = serde_json::from_value(value.clone()).unwrap();
        let back = serde_json::to_value(&filter).unwrap();
        // Money stays decimal strings on the wire (invariant 7).
        assert_eq!(back["value_min"], json!("1001.00"));
        assert_eq!(back, value);
    }

    #[test]
    fn sql_where_uses_every_grammar_slot_and_no_more() {
        for slot in 1..=RecordFilter::BIND_SLOTS {
            assert!(
                RecordFilter::SQL_WHERE.contains(&format!("${slot}")),
                "slot ${slot} missing from SQL_WHERE"
            );
        }
        assert!(!RecordFilter::SQL_WHERE.contains("$12"));
    }

    #[test]
    fn visibility_bound_is_internal_never_contract_surface() {
        // The 24h-delay slot must be invisible to the grammar's serde/schema
        // contract: callers cannot set it, alert rules cannot store it.
        let bounded =
            RecordFilter::default().with_max_discovered_at(Some(chrono::DateTime::UNIX_EPOCH));
        assert_eq!(serde_json::to_value(&bounded).unwrap(), json!({}));
        let schema = serde_json::to_value(schemars::schema_for!(RecordFilter)).unwrap();
        assert!(
            !schema.to_string().contains("max_discovered_at"),
            "internal bound leaked into the committed grammar schema"
        );
        // And a filter parsed from stored JSON is always realtime (alerts).
        let parsed: RecordFilter = serde_json::from_value(json!({})).unwrap();
        assert_eq!(parsed, RecordFilter::default());
    }

    #[test]
    fn schema_forbids_unknown_keys() {
        // The committed contract must reject typo'd filters at the API door
        // (a silently ignored key would match everything — fail closed).
        let schema = serde_json::to_value(schemars::schema_for!(RecordFilter)).unwrap();
        assert_eq!(schema["additionalProperties"], json!(false));
    }

    #[test]
    fn wire_tokens_match_the_sql_check_literals() {
        assert_eq!(wire_token(&RecordType::Transaction).unwrap(), "transaction");
        assert_eq!(wire_token(&AssetClass::RealEstate).unwrap(), "real_estate");
        assert_eq!(
            wire_token(&VerificationState::Unverified).unwrap(),
            "unverified"
        );
    }

    #[test]
    fn wire_token_rejects_non_string_shapes() {
        assert!(matches!(
            wire_token(&42),
            Err(QueryError::NonStringToken { .. })
        ));
    }
}
