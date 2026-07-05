//! Record resources (design §6.1): the canonical Gold listing (ULID cursor
//! pagination; filtering is the ONE shared grammar —
//! `core::query::RecordFilter`, design §6.3 — the same evaluator behind alert
//! matching) and the single-record detail with full provenance + supersession
//! history (the record page's trust surface, design §7.3).

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, NaiveDate, Utc};
use const_format::concatcp;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use govfolio_core::query::RecordFilter;

use crate::AppState;
use crate::dto::{DisclosureRecord, RecordPage, RecordRow, build_page, validate_page_params};
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiQuery;
use crate::routes::RECORD_COLUMNS;
use crate::routes::regimes::{REGIME_COLUMNS, Regime};

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

// ------------------------------------------------------------------ detail --

/// Filing provenance: the source filing the record came from.
#[derive(Debug, Serialize, ToSchema)]
pub struct FilingProvenance {
    /// Filing ULID.
    pub id: String,
    /// Source-native filing id when the source has one.
    pub external_id: Option<String>,
    /// Date the filing was filed.
    pub filed_date: Option<NaiveDate>,
    /// When the government made it public.
    pub published_at: Option<DateTime<Utc>>,
}

/// Raw-document provenance: our archived copy of the official document
/// (invariant 2: raw is sacred — official link + archived copy, design §7.3).
#[derive(Debug, Serialize, ToSchema)]
pub struct RawDocumentProvenance {
    /// Raw document ULID.
    pub id: String,
    /// Official source URL the document was fetched from.
    pub source_url: Option<String>,
    /// sha256 of the archived Bronze bytes (integrity + citability).
    pub sha256: String,
    /// When we fetched the document.
    pub fetched_at: DateTime<Utc>,
}

/// Full provenance of one record: filing, archived raw document, and the
/// regime it was filed under (with its methodology metadata).
#[derive(Debug, Serialize, ToSchema)]
pub struct Provenance {
    /// The source filing.
    pub filing: FilingProvenance,
    /// Our archived copy of the official document.
    pub raw_document: RawDocumentProvenance,
    /// The disclosure regime the record was filed under.
    pub regime: Regime,
}

/// One record with its trust surface (design §7.3): provenance plus the full
/// supersession history in both directions (invariant 1: corrections insert
/// superseding rows, so history is a chain of records).
#[derive(Debug, Serialize, ToSchema)]
pub struct RecordDetail {
    /// The record itself.
    pub record: DisclosureRecord,
    /// Where it came from.
    pub provenance: Provenance,
    /// Records this one (transitively) supersedes — the corrected history,
    /// ascending ULID (= insertion-time) order.
    pub supersedes: Vec<DisclosureRecord>,
    /// Corrections that (transitively) supersede this one, ascending ULID.
    pub superseded_by: Vec<DisclosureRecord>,
}

const GET_SQL: &str = concatcp!(RECORD_COLUMNS, "where id = $1");

/// Ancestor chain: follow `supersedes_record_id` FROM the record. Terminates
/// structurally — the pointer always references an earlier, immutable row
/// (supersede-never-update), so no cycles exist.
const SUPERSEDES_SQL: &str = concatcp!(
    "with recursive up as ( \
        select r.id, r.supersedes_record_id from disclosure_record r \
         where r.id = (select supersedes_record_id from disclosure_record where id = $1) \
        union all \
        select r.id, r.supersedes_record_id from disclosure_record r \
          join up on r.id = up.supersedes_record_id) ",
    RECORD_COLUMNS,
    "where id in (select id from up) order by id"
);

/// Descendant chain: records whose `supersedes_record_id` points (transitively)
/// AT the record.
const SUPERSEDED_BY_SQL: &str = concatcp!(
    "with recursive down as ( \
        select r.id from disclosure_record r where r.supersedes_record_id = $1 \
        union all \
        select r.id from disclosure_record r \
          join down on r.supersedes_record_id = down.id) ",
    RECORD_COLUMNS,
    "where id in (select id from down) order by id"
);

async fn fetch_chain(
    state: &AppState,
    sql: &'static str,
    id: &str,
) -> Result<Vec<DisclosureRecord>, ApiError> {
    let rows: Vec<RecordRow> = sqlx::query_as(sql).bind(id).fetch_all(&state.pool).await?;
    rows.into_iter().map(DisclosureRecord::try_from).collect()
}

/// Fetches one record with provenance and its supersession chain.
///
/// # Errors
/// `404` for an unknown record; `500` on backend failure — all in the
/// consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/records/{id}",
    tag = "records",
    params(("id" = String, Path, description = "Record ULID")),
    responses(
        (status = 200, description = "The record with provenance and supersession history", body = RecordDetail),
        (status = 404, description = "Unknown record", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn get_record(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<RecordDetail>, ApiError> {
    match fetch_record_detail(&state, &id).await? {
        Some(detail) => Ok(Json(detail)),
        None => Err(ApiError::NotFound {
            message: format!("record {id} not found"),
        }),
    }
}

/// Builds the full [`RecordDetail`] for one record id; `None` for an unknown
/// id. Shared with the review-task detail endpoint (goal 041a), so reviewers
/// adjudicate against EXACTLY the public trust surface.
pub(crate) async fn fetch_record_detail(
    state: &AppState,
    id: &str,
) -> Result<Option<RecordDetail>, ApiError> {
    let row: Option<RecordRow> = sqlx::query_as(GET_SQL)
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let record = DisclosureRecord::try_from(row)?;

    // FK-backed joins: a missing filing/raw_document/regime is data
    // corruption and surfaces as 500, never a silent hole.
    #[allow(clippy::type_complexity)]
    let (filing_id, external_id, filed_date, published_at, raw_id, source_url, sha256, fetched_at): (
        String,
        Option<String>,
        Option<NaiveDate>,
        Option<DateTime<Utc>>,
        String,
        Option<String>,
        String,
        DateTime<Utc>,
    ) = sqlx::query_as(
        "select f.id, f.external_id, f.filed_date, f.published_at, \
                d.id, d.source_url, d.sha256, d.fetched_at \
         from filing f join raw_document d on d.id = f.raw_document_id \
         where f.id = $1",
    )
    .bind(&record.filing_id)
    .fetch_one(&state.pool)
    .await?;
    let regime: Regime = sqlx::query_as(concatcp!(REGIME_COLUMNS, "where id = $1"))
        .bind(&record.regime_id)
        .fetch_one(&state.pool)
        .await?;

    let supersedes = fetch_chain(state, SUPERSEDES_SQL, id).await?;
    let superseded_by = fetch_chain(state, SUPERSEDED_BY_SQL, id).await?;

    Ok(Some(RecordDetail {
        record,
        provenance: Provenance {
            filing: FilingProvenance {
                id: filing_id,
                external_id,
                filed_date,
                published_at,
            },
            raw_document: RawDocumentProvenance {
                id: raw_id,
                source_url,
                sha256,
                fetched_at,
            },
            regime,
        },
        supersedes,
        superseded_by,
    }))
}
