//! `GET /v1/records` — the canonical Gold listing (design §6.1). ULID cursor
//! pagination; filtering is the ONE shared grammar (`core::query::RecordFilter`,
//! design §6.3) — the same evaluator behind alert matching.

use axum::Json;
use axum::extract::State;
use const_format::concatcp;
use serde::Deserialize;
use utoipa::IntoParams;

use govfolio_core::query::RecordFilter;

use crate::AppState;
use crate::dto::{RecordPage, RecordRow, build_page, validate_page_params};
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiQuery;
use crate::routes::RECORD_COLUMNS;

/// The full listing statement: the grammar owns `$1..=$10`; the cursor and
/// limit follow at `$11`/`$12`.
const LIST_SQL: &str = concatcp!(
    RECORD_COLUMNS,
    "where ",
    RecordFilter::SQL_WHERE,
    " and ($11::text is null or id > $11) order by id limit $12"
);

/// Pagination parameters of `GET /v1/records`; the filter parameters are the
/// shared [`RecordFilter`] grammar.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListRecordsParams {
    /// Pagination cursor: the `id` of the last record on the previous page
    /// (from `next_cursor`). Pages begin strictly after it.
    #[param(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub cursor: Option<String>,
    /// Page size, `1..=200`; defaults to 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
}

/// Lists disclosure records in ascending ULID order.
///
/// # Errors
/// `400` on a malformed cursor, limit or filter; `500` on backend failure —
/// all in the consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/records",
    tag = "records",
    params(ListRecordsParams, RecordFilter),
    responses(
        (status = 200, description = "One page of disclosure records; every record carries verification_state", body = RecordPage),
        (status = 400, description = "Malformed cursor, limit or filter", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_records(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<ListRecordsParams>,
    ApiQuery(filter): ApiQuery<RecordFilter>,
) -> Result<Json<RecordPage>, ApiError> {
    let (cursor, limit) = validate_page_params(params.cursor.as_deref(), params.limit)?;
    let rows: Vec<RecordRow> = filter
        .bind_query_as(sqlx::query_as(LIST_SQL))?
        .bind(&cursor)
        .bind(limit + 1)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(build_page(rows, limit)?))
}
