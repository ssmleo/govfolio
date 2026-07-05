//! `GET /v1/politicians/{id}/records` — one politician's disclosure timeline
//! (design §6.1), paged with the same ULID cursor rule as `/v1/records`.

use axum::Json;
use axum::extract::{Path, State};
use const_format::concatcp;
use serde::Deserialize;
use utoipa::IntoParams;

use crate::AppState;
use crate::dto::{RecordPage, RecordRow, build_page, validate_page_params};
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiQuery;
use crate::routes::RECORD_COLUMNS;

/// Query parameters of `GET /v1/politicians/{id}/records`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct TimelineParams {
    /// Pagination cursor: the `id` of the last record on the previous page
    /// (from `next_cursor`). Pages begin strictly after it.
    #[param(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub cursor: Option<String>,
    /// Page size, `1..=200`; defaults to 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
}

/// Lists one politician's disclosure records in ascending ULID order.
///
/// # Errors
/// `400` on a malformed cursor or limit; `404` for an unknown politician;
/// `500` on backend failure — all in the consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/politicians/{id}/records",
    tag = "politicians",
    params(
        ("id" = String, Path, description = "Politician ULID"),
        TimelineParams,
    ),
    responses(
        (status = 200, description = "One page of the politician's records; every record carries verification_state", body = RecordPage),
        (status = 400, description = "Malformed cursor or limit", body = ErrorBody),
        (status = 404, description = "Unknown politician", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn politician_records(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ApiQuery(params): ApiQuery<TimelineParams>,
) -> Result<Json<RecordPage>, ApiError> {
    let (cursor, limit) = validate_page_params(params.cursor.as_deref(), params.limit)?;
    let exists: bool = sqlx::query_scalar("select exists(select 1 from politician where id = $1)")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;
    if !exists {
        return Err(ApiError::NotFound {
            message: format!("politician {id} not found"),
        });
    }
    let rows: Vec<RecordRow> = sqlx::query_as(concatcp!(
        RECORD_COLUMNS,
        "where politician_id = $1 \
           and ($2::text is null or id > $2) \
         order by id \
         limit $3"
    ))
    .bind(&id)
    .bind(&cursor)
    .bind(limit + 1)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(build_page(rows, limit)?))
}
