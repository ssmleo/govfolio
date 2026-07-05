//! `GET /v1/regimes` — the transparency-scorecard endpoint (design §6.1):
//! every disclosure regime with the methodology metadata the scorecard
//! renders (design §7.3). The [`Regime`] shape is shared with
//! `/v1/jurisdictions` (nested) and `/v1/records/{id}` provenance — one
//! schema behind every door.

use axum::Json;
use axum::extract::State;
use chrono::NaiveDate;
use const_format::concatcp;
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};

/// One disclosure regime — a jurisdiction body's disclosure rules and the
/// transparency-scorecard metadata (design §6.1/§7.3).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct Regime {
    /// Regime ULID (stable adapter constant).
    pub id: String,
    /// Jurisdiction the regime belongs to (`jurisdiction.id`, e.g. `us`).
    pub jurisdiction_id: String,
    /// Disclosing body, e.g. `US House`.
    pub body: String,
    /// `transaction_report` | `periodic_declaration` | `change_notification`
    /// | `none`.
    pub regime_type: String,
    /// Value precision the regime discloses: `exact` | `banded` |
    /// `categorical` | `none`.
    pub value_precision: String,
    /// Free-form filing cadence description.
    pub cadence: Option<String>,
    /// Statutory maximum disclosure lag in days.
    pub disclosure_lag_days: Option<i32>,
    /// Official source landing page.
    pub source_url: Option<String>,
    /// Date the regime's rules took effect.
    pub effective_from: NaiveDate,
    /// Date the rules stopped applying; `null` while current.
    pub effective_to: Option<NaiveDate>,
}

/// The shared `disclosure_regime` projection — a `const` so composed
/// statements stay compile-time `&'static str`s (`SqlSafeStr` holds
/// structurally).
pub(crate) const REGIME_COLUMNS: &str = "select id, jurisdiction_id, body, regime_type, value_precision, cadence, \
     disclosure_lag_days, source_url, effective_from, effective_to \
     from disclosure_regime ";

/// Lists all disclosure regimes — the scorecard rows — in id order.
///
/// # Errors
/// `500` on backend failure, in the consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/regimes",
    tag = "jurisdictions",
    responses(
        (status = 200, description = "All disclosure regimes with scorecard metadata", body = [Regime]),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_regimes(State(state): State<AppState>) -> Result<Json<Vec<Regime>>, ApiError> {
    let regimes: Vec<Regime> = sqlx::query_as(concatcp!(REGIME_COLUMNS, "order by id"))
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(regimes))
}
