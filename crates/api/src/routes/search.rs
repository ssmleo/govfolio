//! `GET /v1/search` — minimal HONEST search (design §6.1): plain
//! case-insensitive substring matching over politician names/aliases (the
//! same one predicate `/v1/politicians?q=` uses) and instrument names/
//! tickers, in a typed result envelope. No ranking beyond id order, no fuzzy
//! matching, no search infrastructure — design §6.4 keeps Postgres behind
//! `/search` "until it hurts"; dedicated engines (e.g. Typesense) are a
//! documented, unbuilt upgrade path.

use axum::Json;
use axum::extract::State;
use const_format::concatcp;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::AppState;
use crate::error::{ApiError, ErrorBody};
use crate::extract::ApiQuery;
use crate::routes::like_pattern;
use crate::routes::politicians::{POLITICIAN_MATCH, Politician};

/// Cap per result arm — search is a jump-off point, not a listing (the paged
/// resource endpoints are).
const ARM_LIMIT: i64 = 20;

/// One instrument hit.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct InstrumentHit {
    /// Instrument ULID.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// Instrument display name.
    pub name: String,
    /// Exchange ticker when known.
    pub ticker: Option<String>,
    /// Asset class as stored.
    pub asset_class: String,
}

/// Typed search results: one arm per entity kind, each capped at 20 hits in
/// id order.
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResults {
    /// The query as evaluated (trimmed).
    pub query: String,
    /// Politicians whose canonical name or as-filed alias contains the query.
    pub politicians: Vec<Politician>,
    /// Instruments whose name or ticker contains the query.
    pub instruments: Vec<InstrumentHit>,
}

const POLITICIAN_SEARCH_SQL: &str = concatcp!(
    "select id, canonical_name, wikidata_qid from politician where ",
    POLITICIAN_MATCH,
    " order by id limit ",
    ARM_LIMIT
);

const INSTRUMENT_SEARCH_SQL: &str = concatcp!(
    "select id, name, ticker, asset_class from instrument \
     where name ilike $1 or ticker ilike $1 order by id limit ",
    ARM_LIMIT
);

/// Query parameters of `GET /v1/search`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchParams {
    /// Query text: case-insensitive substring, at least one non-blank
    /// character; `%`/`_` are literals.
    pub q: String,
}

/// Searches politicians (name/alias) and instruments (name/ticker).
///
/// # Errors
/// `400` on a missing or blank `q`; `500` on backend failure — all in the
/// consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/search",
    tag = "search",
    params(SearchParams),
    responses(
        (status = 200, description = "Typed search results", body = SearchResults),
        (status = 400, description = "Missing or blank query", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn search(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<SearchParams>,
) -> Result<Json<SearchResults>, ApiError> {
    let query = params.q.trim();
    if query.is_empty() {
        return Err(ApiError::bad_request(
            "invalid_query",
            "q must contain at least one non-blank character",
        ));
    }
    let pattern = like_pattern(query);
    let politicians: Vec<Politician> = sqlx::query_as(POLITICIAN_SEARCH_SQL)
        .bind(&pattern)
        .fetch_all(&state.pool)
        .await?;
    let instruments: Vec<InstrumentHit> = sqlx::query_as(INSTRUMENT_SEARCH_SQL)
        .bind(&pattern)
        .fetch_all(&state.pool)
        .await?;
    Ok(Json(SearchResults {
        query: query.to_owned(),
        politicians,
        instruments,
    }))
}
