//! Filing resources (design §6.1's originally-planned `/filings/{id} (+
//! raw-doc link)`, scoped to just the document sub-resource — filing
//! metadata already rides on `/v1/records/{id}`'s `provenance.filing`).
//! Serves OUR OWN archived copy of the original document (design §7.3:
//! "official-source link + our archived copy") rather than the government's
//! own URL, which can rot, change, or (Brazil) point at a nationwide bulk
//! file instead of anything politician-specific.

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse as _, Response};
use const_format::concatcp;

use govfolio_core::query::RecordFilter;

use crate::AppState;
use crate::auth::AuthContext;
use crate::bronze;
use crate::error::{ApiError, ErrorBody};

/// The SAME visibility gate as `/v1/records/{id}` (`RecordFilter::SQL_WHERE`
/// binds `$1..=$11`; the filing id is `$12`): a filing's document is only
/// servable when at least one of its OWN disclosure records is visible under
/// the caller's tier. This is what stops a free-tier caller from bypassing
/// the 24h embargo (goal 050) by guessing a filing id directly.
const DOCUMENT_SQL: &str = concatcp!(
    "select d.storage_uri, d.mime_type \
     from filing f join raw_document d on d.id = f.raw_document_id \
     where f.id = $12 and exists ( \
       select 1 from disclosure_record where filing_id = f.id and ",
    RecordFilter::SQL_WHERE,
    ")"
);

/// Shared serving logic behind both the public (tier-gated) and admin
/// (real-time reviewer) document routes: identical query shape, identical
/// bytes, identical security headers — only the `RecordFilter` visibility
/// bound differs between callers.
async fn fetch_document(
    state: &AppState,
    filter: &RecordFilter,
    id: &str,
) -> Result<Response, ApiError> {
    let row: Option<(String, String)> = filter
        .bind_query_as(sqlx::query_as(DOCUMENT_SQL))?
        .bind(id)
        .fetch_optional(&state.pool)
        .await?;
    let Some((storage_uri, mime_type)) = row else {
        return Err(ApiError::NotFound {
            message: format!("filing {id} not found"),
        });
    };
    let bytes = bronze::read_document(&storage_uri).await?;
    let content_type = HeaderValue::from_str(&mime_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let mut response = (StatusCode::OK, bytes).into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, content_type);
    // The response is an arbitrary byte blob (PDF/HTML/JSON/etc, design §3);
    // never let a browser MIME-sniff it into something else.
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok(response)
}

/// Serves the archived original document for one filing (public, tier-gated).
///
/// # Errors
/// `404` for an unknown filing, or one not yet visible under the caller's
/// tier (the same freshness bound as every other record-serving route);
/// `503` if the document's storage backend is not implemented in this build;
/// `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/filings/{id}/document",
    tag = "filings",
    params(("id" = String, Path, description = "Filing ULID")),
    responses(
        (status = 200, description = "The archived document bytes; Content-Type reflects the sniffed mime type"),
        (status = 404, description = "Unknown filing, or not yet visible under the caller's tier", body = ErrorBody),
        (status = 503, description = "Storage backend not available for this document", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn get_filing_document(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let filter = auth.filter();
    fetch_document(&state, &filter, &id).await
}

/// Serves the archived original document for one filing, in REAL TIME
/// (admin-gated): the reviewer surface must see freshly-ingested filings the
/// moment they exist, not 24h later — mirrors `review::get_review_task`'s
/// own `RecordFilter::default()` use for exactly the same reason. Consumed
/// by the web app's same-origin reviewer document proxy, never called
/// directly from a browser (the `X-Admin-Token` never reaches client JS).
///
/// # Errors
/// Same as [`get_filing_document`], plus `401`/`403` from the admin gate.
#[utoipa::path(
    get,
    path = "/v1/admin/filings/{id}/document",
    tag = "filings",
    params(("id" = String, Path, description = "Filing ULID")),
    responses(
        (status = 200, description = "The archived document bytes; Content-Type reflects the sniffed mime type"),
        (status = 404, description = "Unknown filing", body = ErrorBody),
        (status = 401, description = "Missing or invalid X-Admin-Token", body = ErrorBody),
        (status = 503, description = "Storage backend not available for this document", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn get_admin_filing_document(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    fetch_document(&state, &RecordFilter::default(), &id).await
}
