//! Politician resources (design §6.1): the paged listing with a plain-ILIKE
//! name query, the profile (`politician` + mandates + record summary — what
//! the profile page renders, design §6.4/§7.3), and the record timeline
//! paged with the same ULID cursor rule as `/v1/records`.

use anyhow::Context as _;
use axum::Json;
use axum::extract::{Extension, Path, State};
use chrono::NaiveDate;
use const_format::concatcp;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use govfolio_core::ids::PoliticianId;
use govfolio_core::query::RecordFilter;

use crate::AppState;
use crate::auth::AuthContext;
use crate::dto::{RecordPage, RecordRow, build_page, validate_page_params};
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiQuery;
use crate::routes::like_pattern;
use crate::routes::records::LIST_SQL as RECORDS_LIST_SQL;

/// One politician (Gold `politician`, design §4.2).
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct Politician {
    /// Politician ULID (the pagination cursor).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Canonical person name (no honorific).
    pub canonical_name: String,
    /// Wikidata QID when linked.
    pub wikidata_qid: Option<String>,
}

/// One page of politicians; the same `next_cursor` rule as `/v1/records`.
#[derive(Debug, Serialize, ToSchema)]
pub struct PoliticianPage {
    /// Politicians in ascending ULID order.
    pub items: Vec<Politician>,
    /// Pass back as `cursor` to fetch the page after this one; `null` at the end.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub next_cursor: Option<String>,
}

/// One mandate (Gold `mandate`): a role held in a jurisdiction body.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct Mandate {
    /// Mandate ULID.
    pub id: String,
    /// Jurisdiction of the body (`jurisdiction.id`, e.g. `us`).
    pub jurisdiction_id: String,
    /// The body, e.g. `US House`.
    pub body: String,
    /// Role held, e.g. `Representative`.
    pub role: String,
    /// Party affiliation when known.
    pub party: Option<String>,
    /// District code as filed, e.g. `PA11`.
    pub district: Option<String>,
    /// Mandate start (roster-attested "active since at least" bound).
    pub start_date: NaiveDate,
    /// Mandate end; `null` while active.
    pub end_date: Option<NaiveDate>,
}

/// Aggregate summary of a politician's disclosure records.
#[derive(Debug, Serialize, ToSchema)]
pub struct RecordSummary {
    /// Total records concerning the politician.
    pub count: i64,
    /// Earliest `event_date` across those records.
    pub first_event_date: Option<NaiveDate>,
    /// Latest `event_date` across those records.
    pub last_event_date: Option<NaiveDate>,
}

/// A politician profile: the person, their mandates, and a record summary
/// (the profile page's above-the-fold data, design §6.4).
#[derive(Debug, Serialize, ToSchema)]
pub struct PoliticianProfile {
    /// Politician ULID.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Canonical person name (no honorific).
    pub canonical_name: String,
    /// Wikidata QID when linked.
    pub wikidata_qid: Option<String>,
    /// Mandates, most recent first.
    pub mandates: Vec<Mandate>,
    /// Disclosure-record summary (count + event-date range).
    pub records: RecordSummary,
}

/// The ONE name-match predicate (`$1` = escaped ILIKE pattern): canonical
/// name OR any as-filed alias. Plain substring ILIKE — no fuzzy matching.
/// Shared with `/v1/search` (one predicate, two doors).
pub(crate) const POLITICIAN_MATCH: &str = "(politician.canonical_name ilike $1 \
     or exists (select 1 from politician_alias a \
                 where a.politician_id = politician.id and a.alias ilike $1))";

/// The listing statement: `$1` = optional name pattern, `$2` = cursor,
/// `$3` = limit.
const LIST_SQL: &str = concatcp!(
    "select id, canonical_name, wikidata_qid from politician \
     where ($1::text is null or ",
    POLITICIAN_MATCH,
    ") and ($2::text is null or id > $2) order by id limit $3"
);

/// Query parameters of `GET /v1/politicians`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListPoliticiansParams {
    /// Case-insensitive substring over canonical name and as-filed aliases
    /// (plain ILIKE; `%`/`_` are literals).
    pub q: Option<String>,
    /// Pagination cursor: the `id` of the last politician on the previous
    /// page (from `next_cursor`). Pages begin strictly after it.
    #[param(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub cursor: Option<String>,
    /// Page size, `1..=200`; defaults to 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
}

/// Lists politicians in ascending ULID order, optionally name-filtered.
///
/// # Errors
/// `400` on a malformed cursor or limit; `500` on backend failure — all in
/// the consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/politicians",
    tag = "politicians",
    params(ListPoliticiansParams),
    responses(
        (status = 200, description = "One page of politicians", body = PoliticianPage),
        (status = 400, description = "Malformed cursor or limit", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_politicians(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<ListPoliticiansParams>,
) -> Result<Json<PoliticianPage>, ApiError> {
    let (cursor, limit) = validate_page_params(params.cursor.as_deref(), params.limit)?;
    let pattern = params
        .q
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .map(like_pattern);
    let mut items: Vec<Politician> = sqlx::query_as(LIST_SQL)
        .bind(&pattern)
        .bind(&cursor)
        .bind(limit + 1)
        .fetch_all(&state.pool)
        .await?;
    let page_len = usize::try_from(limit).context("limit fits usize")?;
    let has_more = items.len() > page_len;
    items.truncate(page_len);
    let next_cursor = if has_more {
        items.last().map(|politician| politician.id.clone())
    } else {
        None
    };
    Ok(Json(PoliticianPage { items, next_cursor }))
}

/// Fetches one politician's profile: person, mandates, record summary.
///
/// # Errors
/// `404` for an unknown politician; `500` on backend failure — all in the
/// consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/politicians/{id}",
    tag = "politicians",
    params(("id" = String, Path, description = "Politician ULID")),
    responses(
        (status = 200, description = "The politician's profile", body = PoliticianProfile),
        (status = 404, description = "Unknown politician", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn politician_profile(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Json<PoliticianProfile>, ApiError> {
    let politician: Option<Politician> =
        sqlx::query_as("select id, canonical_name, wikidata_qid from politician where id = $1")
            .bind(&id)
            .fetch_optional(&state.pool)
            .await?;
    let Some(politician) = politician else {
        return Err(ApiError::NotFound {
            message: format!("politician {id} not found"),
        });
    };
    let mandates: Vec<Mandate> = sqlx::query_as(
        "select id, jurisdiction_id, body, role, party, district, start_date, end_date \
         from mandate where politician_id = $1 order by start_date desc, id",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;
    // The summary is derived from records, so it flows through the shared
    // evaluator too — a free-tier profile must not count records the free
    // tier cannot see yet (the count would leak "something fresh exists").
    let scope = politician_scope(&politician.id)
        .ok_or_else(|| ApiError::from(anyhow::anyhow!("stored politician id is not a ULID")))?;
    let (count, first_event_date, last_event_date): (i64, Option<NaiveDate>, Option<NaiveDate>) =
        auth.apply(scope)
            .bind_query_as(sqlx::query_as(SUMMARY_SQL))?
            .fetch_one(&state.pool)
            .await?;
    Ok(Json(PoliticianProfile {
        id: politician.id,
        canonical_name: politician.canonical_name,
        wikidata_qid: politician.wikidata_qid,
        mandates,
        records: RecordSummary {
            count,
            first_event_date,
            last_event_date,
        },
    }))
}

/// Record summary through the ONE evaluator (`$1..=$11` = the grammar with
/// the caller's freshness bound; the politician pins via the grammar's own
/// `politician_id` slot).
const SUMMARY_SQL: &str = concatcp!(
    "select count(*), min(event_date), max(event_date) from disclosure_record where ",
    RecordFilter::SQL_WHERE
);

/// A grammar filter scoped to one politician. Stored politician ids are
/// ULIDs by construction; a path id that does not parse cannot name a
/// politician, so callers turn `None` into their 404 (fail closed — never
/// an unscoped filter).
fn politician_scope(id: &str) -> Option<RecordFilter> {
    Some(RecordFilter::default().with_politician(id.parse::<PoliticianId>().ok()?))
}

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
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
    ApiQuery(params): ApiQuery<TimelineParams>,
) -> Result<Json<RecordPage>, ApiError> {
    let (cursor, limit) = validate_page_params(params.cursor.as_deref(), params.limit)?;
    let exists: bool = sqlx::query_scalar("select exists(select 1 from politician where id = $1)")
        .bind(&id)
        .fetch_one(&state.pool)
        .await?;
    let scope = politician_scope(&id);
    let (Some(scope), true) = (scope, exists) else {
        return Err(ApiError::NotFound {
            message: format!("politician {id} not found"),
        });
    };
    // The timeline is /v1/records scoped to one politician: the SAME
    // statement, so the tier's freshness bound cannot diverge between the
    // two doors.
    let rows: Vec<RecordRow> = auth
        .apply(scope)
        .bind_query_as(sqlx::query_as(RECORDS_LIST_SQL))?
        .bind(&cursor)
        .bind(limit + 1)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(build_page(rows, limit)?))
}
