//! govfolio `/v1` API (plan Task 10, design §6.1): axum + sqlx handlers over
//! Gold, utoipa as the single contract source. `openapi_json()` is the exact
//! byte producer behind `packages/contracts/openapi.json` — generated,
//! committed, never hand-edited; regen drift fails CI.

pub mod dto;
pub mod error;
pub mod extract;
pub mod routes;

use anyhow::Context as _;
use axum::Router;
use axum::routing::get;
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
        .route(
            "/v1/politicians/{id}/records",
            get(routes::politicians::politician_records),
        )
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
                       verification_state on every record."
    ),
    paths(
        routes::records::list_records,
        routes::politicians::politician_records,
    ),
    tags(
        (name = "records", description = "Canonical disclosure records (Gold)"),
        (name = "politicians", description = "Politician-scoped views"),
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
