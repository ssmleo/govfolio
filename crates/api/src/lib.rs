//! govfolio `/v1` API (plan Task 10, design §6.1): axum + sqlx handlers over
//! Gold, utoipa as the single contract source. `openapi_json()` is the exact
//! byte producer behind `packages/contracts/openapi.json` — generated,
//! committed, never hand-edited; regen drift fails CI.

pub mod dto;
pub mod error;
pub mod etag;
pub mod extract;
pub mod routes;

use anyhow::Context as _;
use axum::Router;
use axum::routing::{get, post, put};
use sqlx::PgPool;
use utoipa::OpenApi;

/// Shared handler state.
#[derive(Clone)]
pub struct AppState {
    /// Read pool over the Gold schema.
    pub pool: PgPool,
}

/// Builds the `/v1` router over the given pool — the one app both `main` and
/// the contract test boot.
pub fn app(pool: PgPool) -> Router {
    Router::new()
        .route("/v1/records", get(routes::records::list_records))
        .route("/v1/records/{id}", get(routes::records::get_record))
        .route(
            "/v1/politicians",
            get(routes::politicians::list_politicians),
        )
        .route(
            "/v1/politicians/{id}",
            get(routes::politicians::politician_profile),
        )
        .route(
            "/v1/politicians/{id}/records",
            get(routes::politicians::politician_records),
        )
        .route(
            "/v1/jurisdictions",
            get(routes::jurisdictions::list_jurisdictions),
        )
        .route("/v1/regimes", get(routes::regimes::list_regimes))
        .route("/v1/search", get(routes::search::search))
        .route(
            "/v1/alert-rules",
            post(routes::alert_rules::create_alert_rule).get(routes::alert_rules::list_alert_rules),
        )
        .route(
            "/v1/alert-rules/{id}",
            put(routes::alert_rules::update_alert_rule)
                .delete(routes::alert_rules::delete_alert_rule),
        )
        // Reviewer admin surface (design §7.2). Resolution goes through the
        // pipeline promote path — the API never mutates records directly.
        .route("/v1/review-tasks", get(routes::review::list_review_tasks))
        .route(
            "/v1/review-tasks/{id}",
            get(routes::review::get_review_task),
        )
        .route(
            "/v1/review-tasks/{id}/resolve",
            post(routes::review::resolve_review_task),
        )
        .route(
            "/v1/review-tasks/{id}/audit",
            get(routes::review::review_task_audit),
        )
        // Strong ETags + If-None-Match → 304 on every successful GET
        // (design §6.1: ETags everywhere).
        .layer(axum::middleware::from_fn(etag::etag))
        .with_state(AppState { pool })
}

/// The `OpenAPI` document — generated from the handlers, so the contract can
/// never drift from the code (drift in the committed copy fails CI instead).
#[derive(OpenApi)]
#[openapi(
    info(
        title = "govfolio API",
        description = "Worldwide politician financial-disclosure tracking. \
                       Cursor pagination on ULIDs; consistent error envelope; \
                       verification_state on every record. Every successful GET \
                       carries a strong ETag (sha256 of the body); requests with \
                       a matching If-None-Match receive 304 Not Modified."
    ),
    paths(
        routes::records::list_records,
        routes::records::get_record,
        routes::politicians::list_politicians,
        routes::politicians::politician_profile,
        routes::politicians::politician_records,
        routes::jurisdictions::list_jurisdictions,
        routes::regimes::list_regimes,
        routes::search::search,
        routes::alert_rules::create_alert_rule,
        routes::alert_rules::list_alert_rules,
        routes::alert_rules::update_alert_rule,
        routes::alert_rules::delete_alert_rule,
        routes::review::list_review_tasks,
        routes::review::get_review_task,
        routes::review::resolve_review_task,
        routes::review::review_task_audit,
    ),
    tags(
        (name = "records", description = "Canonical disclosure records (Gold)"),
        (name = "politicians", description = "Politician-scoped views"),
        (name = "jurisdictions", description = "Jurisdictions and disclosure \
         regimes — the transparency scorecard (design §6.1/§7.3)"),
        (name = "search", description = "Minimal substring search over \
         politicians and instruments (Postgres-backed until it hurts, §6.4)"),
        (name = "alert-rules", description = "Alert rules over the shared record \
         filter grammar (design §6.3). Auth arrives with accounts (goal 050)."),
        (name = "review", description = "Admin review queue (design §7.1–7.2): \
         priority-ranked tasks with target-record context and extraction \
         evidence; resolutions go through the pipeline promote path \
         (supersede-never-update) and every attempt is audit-logged. Auth \
         arrives with accounts (goal 050); reviewer is free text until then."),
    )
)]
pub struct ApiDoc;

/// Serializes the `OpenAPI` document: pretty-printed with recursively sorted
/// object keys, so regeneration is byte-deterministic and diffs stay clean.
///
/// # Errors
/// Serialization failure (structurally impossible for a well-formed doc).
pub fn openapi_json() -> anyhow::Result<String> {
    let mut doc = ApiDoc::openapi();
    // No license is declared yet (public legal copy is a human lane); drop the
    // empty stub utoipa mirrors from the crate metadata rather than invent one.
    doc.info.license = None;
    let value = serde_json::to_value(doc).context("serializing the OpenAPI document")?;
    let mut out = serde_json::to_string_pretty(&sort_json(value))
        .context("pretty-printing the OpenAPI document")?;
    out.push('\n');
    Ok(out)
}

/// Recursively sorts object keys (arrays keep their order — it is semantic).
/// Explicit sort, never the map backing: any crate enabling `preserve_order`
/// would silently flip insertion order otherwise.
fn sort_json(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut sorted = serde_json::Map::with_capacity(entries.len());
            for (key, val) in entries {
                sorted.insert(key, sort_json(val));
            }
            serde_json::Value::Object(sorted)
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(sort_json).collect())
        }
        other => other,
    }
}
