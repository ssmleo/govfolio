//! `GET /v1/jurisdictions` — jurisdictions with their disclosure regimes
//! joined in (design §6.1): the source a per-jurisdiction transparency
//! scorecard page renders from (design §7.3 coverage surface).

use axum::Json;
use axum::extract::State;
use const_format::concatcp;
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};
use crate::routes::regimes::{REGIME_COLUMNS, Regime};

/// One jurisdiction with its disclosure regimes — the scorecard row.
#[derive(Debug, Serialize, ToSchema)]
pub struct Jurisdiction {
    /// Stable jurisdiction id (ISO 3166-1 alpha-2 lowercase by convention).
    pub id: String,
    /// Display name.
    pub name: String,
    /// ISO 3166-1 alpha-2 where applicable.
    pub iso_code: Option<String>,
    /// `supranational` | `national` | `subnational`.
    pub level: String,
    /// Parent jurisdiction for subnational entries.
    pub parent_id: Option<String>,
    /// Disclosure regimes of this jurisdiction, in id order — the
    /// transparency-scorecard metadata (design §7.3).
    pub regimes: Vec<Regime>,
}

/// Lists all jurisdictions (id order) with their regimes joined in.
///
/// # Errors
/// `500` on backend failure, in the consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/jurisdictions",
    tag = "jurisdictions",
    responses(
        (status = 200, description = "All jurisdictions with their disclosure regimes", body = [Jurisdiction]),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_jurisdictions(
    State(state): State<AppState>,
) -> Result<Json<Vec<Jurisdiction>>, ApiError> {
    #[allow(clippy::type_complexity)]
    let rows: Vec<(String, String, Option<String>, String, Option<String>)> =
        sqlx::query_as("select id, name, iso_code, level, parent_id from jurisdiction order by id")
            .fetch_all(&state.pool)
            .await?;
    let mut regime_rows: Vec<Regime> = sqlx::query_as(concatcp!(REGIME_COLUMNS, "order by id"))
        .fetch_all(&state.pool)
        .await?;

    let jurisdictions = rows
        .into_iter()
        .map(|(id, name, iso_code, level, parent_id)| {
            // Both lists are id-ordered and small (design volumes: hundreds at
            // most); extract_if keeps each regime attached exactly once.
            let regimes = regime_rows
                .extract_if(.., |regime| regime.jurisdiction_id == id)
                .collect();
            Jurisdiction {
                id,
                name,
                iso_code,
                level,
                parent_id,
                regimes,
            }
        })
        .collect();
    Ok(Json(jurisdictions))
}
