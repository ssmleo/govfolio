//! `GET /v1/records` — the canonical Gold listing (design §6.1). ULID cursor
//! pagination; filters are minimal for now and grow with the one shared
//! filter grammar (design §6.3).

use axum::Json;
use axum::extract::State;
use serde::Deserialize;
use utoipa::IntoParams;

use govfolio_core::domain::enums::{RecordType, VerificationState};

use crate::AppState;
use crate::dto::{RecordPage, RecordRow, build_page, to_token, validate_page_params};
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiQuery;
use crate::routes::record_select;

/// Query parameters of `GET /v1/records`.
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
    /// Only records of this type.
    pub record_type: Option<RecordType>,
    /// Only records in this verification state.
    pub verification_state: Option<VerificationState>,
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
    params(ListRecordsParams),
    responses(
        (status = 200, description = "One page of disclosure records; every record carries verification_state", body = RecordPage),
        (status = 400, description = "Malformed cursor, limit or filter", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_records(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<ListRecordsParams>,
) -> Result<Json<RecordPage>, ApiError> {
    let (cursor, limit) = validate_page_params(params.cursor.as_deref(), params.limit)?;
    let record_type = params.record_type.as_ref().map(to_token).transpose()?;
    let verification_state = params
        .verification_state
        .as_ref()
        .map(to_token)
        .transpose()?;
    let rows: Vec<RecordRow> = sqlx::query_as(record_select!(
        "where ($1::text is null or id > $1) \
           and ($2::text is null or record_type = $2) \
           and ($3::text is null or verification_state = $3) \
         order by id \
         limit $4"
    ))
    .bind(&cursor)
    .bind(&record_type)
    .bind(&verification_state)
    .bind(limit + 1)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(build_page(rows, limit)?))
}
