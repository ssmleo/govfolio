//! govfolio `/v1` API (plan Task 10, design §6.1): axum + sqlx handlers over
//! Gold, utoipa as the single contract source. `openapi_json()` is the exact
//! byte producer behind `packages/contracts/openapi.json` — generated,
//! committed, never hand-edited; regen drift fails CI.

pub mod auth;
pub mod dto;
pub mod error;
pub mod etag;
pub mod extract;
pub mod routes;

use std::sync::Arc;

use anyhow::Context as _;
use axum::Router;
use axum::routing::{get, post, put};
use sqlx::PgPool;
use utoipa::OpenApi;

/// Deploy-time configuration (all optional; every absence fails closed).
#[derive(Debug, Default, Clone)]
pub struct ApiConfig {
    /// Bootstrap admin token (`ADMIN_TOKEN`): gates account creation and the
    /// reviewer surface. Unset = those surfaces are disabled (401), never
    /// open.
    pub admin_token: Option<String>,
    /// Stripe webhook endpoint secret (`STRIPE_WEBHOOK_SECRET`). Unset = the
    /// webhook endpoint answers 503 (an unverifiable event is never acted
    /// on).
    pub stripe_webhook_secret: Option<String>,
    /// Anonymous per-IP per-minute backstop (`UNAUTH_REQUESTS_PER_MINUTE`,
    /// default [`ApiConfig::DEFAULT_UNAUTH_PER_MINUTE`]). Authoritative
    /// anonymous limiting lives at the CDN edge (design §6.4); this only
    /// stops a single instance from being trivially hammered.
    pub unauth_requests_per_minute: u32,
    /// Repo checkout root (`GOVFOLIO_REPO_ROOT`) for the autonomous-loop
    /// meta surface (`/v1/admin/loop`): goal queue, git activity, journal.
    /// Unset (the cloud posture) = that endpoint answers 503 Unavailable by
    /// design — never a guessed path, never faked data.
    pub repo_root: Option<std::path::PathBuf>,
}

impl ApiConfig {
    /// Generous by design: the SSR origin funnels many site visitors
    /// through one IP.
    pub const DEFAULT_UNAUTH_PER_MINUTE: u32 = 600;

    /// Reads the deploy environment (see field docs for the variables).
    #[must_use]
    pub fn from_env() -> Self {
        Self {
            admin_token: std::env::var("ADMIN_TOKEN").ok().filter(|t| !t.is_empty()),
            stripe_webhook_secret: std::env::var("STRIPE_WEBHOOK_SECRET")
                .ok()
                .filter(|s| !s.is_empty()),
            unauth_requests_per_minute: std::env::var("UNAUTH_REQUESTS_PER_MINUTE")
                .ok()
                .and_then(|raw| raw.parse().ok())
                .unwrap_or(Self::DEFAULT_UNAUTH_PER_MINUTE),
            repo_root: std::env::var("GOVFOLIO_REPO_ROOT")
                .ok()
                .filter(|p| !p.is_empty())
                .map(std::path::PathBuf::from),
        }
    }

    /// Default config with the standard anonymous ceiling (the derived
    /// `Default` would be 0 = block everything).
    #[must_use]
    pub fn new() -> Self {
        Self {
            unauth_requests_per_minute: Self::DEFAULT_UNAUTH_PER_MINUTE,
            ..Self::default()
        }
    }
}

/// Shared handler state.
#[derive(Clone)]
pub struct AppState {
    /// Read pool over the Gold schema.
    pub pool: PgPool,
    /// Deploy-time configuration.
    pub config: Arc<ApiConfig>,
    /// Anonymous backstop counters (per app instance — see `auth` docs).
    pub unauth_counters: auth::UnauthCounters,
}

/// Builds the `/v1` router over the given pool — the one app both `main` and
/// the contract tests boot.
pub fn app(pool: PgPool, config: ApiConfig) -> Router {
    let state = AppState {
        pool,
        config: Arc::new(config),
        unauth_counters: auth::UnauthCounters::default(),
    };
    // Reviewer admin surface (design §7.2), whole subtree behind the
    // bootstrap admin token: review tasks expose record context in REAL
    // TIME, so leaving it open would tunnel under the freemium delay.
    // Resolution goes through the pipeline promote path — the API never
    // mutates records directly.
    let review = Router::new()
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
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::admin_gate,
        ));
    // Read-only ops observability surface (goal 090), same admin gate as the
    // review subtree: run/backfill/cost data would leak operational detail.
    let ops = Router::new()
        .route("/v1/admin/ops/overview", get(routes::ops::overview))
        .route("/v1/admin/ops/runs", get(routes::ops::list_runs))
        .route("/v1/admin/ops/runs/summary", get(routes::ops::runs_summary))
        .route("/v1/admin/ops/backfill", get(routes::ops::backfill))
        .route("/v1/admin/ops/freezes", get(routes::ops::freezes))
        .route(
            "/v1/admin/ops/review-health",
            get(routes::ops::review_health),
        )
        .route("/v1/admin/ops/deliveries", get(routes::ops::deliveries))
        .route(
            "/v1/admin/ops/extraction-costs",
            get(routes::ops::extraction_costs),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::admin_gate,
        ));
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
        // Account bootstrap + key management (goal 050; real signup deferred).
        .route("/v1/users", post(routes::keys::create_user))
        .route(
            "/v1/keys",
            post(routes::keys::create_key).get(routes::keys::list_keys),
        )
        .route(
            "/v1/keys/{id}",
            axum::routing::delete(routes::keys::revoke_key),
        )
        // Stripe subscription mirror (signature-verified, config-gated).
        .route("/v1/stripe/webhook", post(routes::stripe::stripe_webhook))
        .merge(review)
        .merge(admin_router(&state))
        .merge(ops)
        // Strong ETags + If-None-Match → 304 on every successful GET
        // (design §6.1: ETags everywhere).
        .layer(axum::middleware::from_fn(etag::etag))
        // Outermost: key → user → tier resolution, quota + metering, the
        // anonymous backstop. Every request below carries an AuthContext.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::authenticate,
        ))
        // Liveness probe, added AFTER the two layers on purpose: axum layers
        // only wrap routes added before them, so /healthz bypasses
        // authenticate (no metering, no anonymous limiter) AND etag (no
        // ETag/304 caching on a probe).
        .route("/healthz", get(routes::ops::healthz))
        .with_state(state)
}

/// The admin observability subtree (admin dashboard plan, sections A–H):
/// nine read-only composite doors, one per dashboard page, behind the same
/// fail-closed `X-Admin-Token` gate as the reviewer subtree.
fn admin_router(state: &AppState) -> Router<AppState> {
    Router::new()
        .route(
            "/v1/admin/overview",
            get(routes::admin::overview::admin_overview),
        )
        .route(
            "/v1/admin/coverage",
            get(routes::admin::coverage::admin_coverage),
        )
        .route(
            "/v1/admin/backfill",
            get(routes::admin::backfill::admin_backfill),
        )
        .route(
            "/v1/admin/pipeline",
            get(routes::admin::pipeline::admin_pipeline),
        )
        .route(
            "/v1/admin/quality",
            get(routes::admin::quality::admin_quality),
        )
        .route(
            "/v1/admin/storage",
            get(routes::admin::storage::admin_storage),
        )
        .route(
            "/v1/admin/serving",
            get(routes::admin::serving::admin_serving),
        )
        .route("/v1/admin/infra", get(routes::admin::infra::admin_infra))
        .route("/v1/admin/loop", get(routes::admin::loop_meta::admin_loop))
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::admin_gate,
        ))
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
                       a matching If-None-Match receive 304 Not Modified. \
                       Authentication: `Authorization: Bearer gfk_...` API keys \
                       (create via /v1/keys; hashes only at rest). Requests \
                       without a key are served at the free tier. Tiers control \
                       record freshness (free: filings we discovered less than \
                       24 hours ago are not yet visible; pro/data: real time) \
                       and the daily request quota (counted per account; over \
                       quota is 429 quota_exceeded in the standard envelope). \
                       Anonymous traffic has a light per-IP rate backstop \
                       (429 rate_limited)."
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
        routes::keys::create_user,
        routes::keys::create_key,
        routes::keys::list_keys,
        routes::keys::revoke_key,
        routes::stripe::stripe_webhook,
        routes::review::list_review_tasks,
        routes::review::get_review_task,
        routes::review::resolve_review_task,
        routes::review::review_task_audit,
        routes::admin::overview::admin_overview,
        routes::admin::coverage::admin_coverage,
        routes::admin::backfill::admin_backfill,
        routes::admin::pipeline::admin_pipeline,
        routes::admin::quality::admin_quality,
        routes::admin::storage::admin_storage,
        routes::admin::serving::admin_serving,
        routes::admin::infra::admin_infra,
        routes::admin::loop_meta::admin_loop,
        routes::ops::healthz,
        routes::ops::overview,
        routes::ops::list_runs,
        routes::ops::runs_summary,
        routes::ops::backfill,
        routes::ops::freezes,
        routes::ops::review_health,
        routes::ops::deliveries,
        routes::ops::extraction_costs,
    ),
    tags(
        (name = "records", description = "Canonical disclosure records (Gold)"),
        (name = "politicians", description = "Politician-scoped views"),
        (name = "jurisdictions", description = "Jurisdictions and disclosure \
         regimes — the transparency scorecard (design §6.1/§7.3)"),
        (name = "search", description = "Minimal substring search over \
         politicians and instruments (Postgres-backed until it hurts, §6.4)"),
        (name = "alert-rules", description = "Alert rules over the shared record \
         filter grammar (design §6.3). Requires a pro or data tier API key; \
         rules belong to the authenticated account."),
        (name = "account", description = "Accounts and API keys (goal 050). \
         Account creation is operator-bootstrap behind X-Admin-Token until \
         self-serve signup ships; keys are hashed at rest and shown exactly \
         once."),
        (name = "billing", description = "Stripe integration seam (goal 050): \
         signature-verified webhook mirroring subscription status; usage \
         metering flows from usage_event via the billing-sync worker."),
        (name = "review", description = "Admin review queue (design §7.1–7.2), \
         X-Admin-Token gated: priority-ranked tasks with REAL-TIME record \
         context and extraction evidence; resolutions go through the pipeline \
         promote path (supersede-never-update) and every attempt is \
         audit-logged."),
        (name = "admin", description = "Admin observability dashboard \
         (X-Admin-Token gated, READ-ONLY): nine composite snapshots — \
         overview status strip, coverage & inventory, backfill & ingestion, \
         pipeline health, data quality & review ops, storage & tiers, \
         serving & product, money & infra (static v1), and autonomous-loop \
         meta (503 without GOVFOLIO_REPO_ROOT, by design). Metrics that \
         cannot be observed are explicit unavailable states, never guessed."),
        (name = "ops", description = "Read-only admin ops observability \
         (goal 090), X-Admin-Token gated: backfill progress, pipeline runs, \
         sentinel freezes/drift, review-queue health, alert delivery and LLM \
         extraction cost vs the USD 200.00 monthly HARD CAP. Poll-friendly: \
         every body carries a strong ETag. Narrower and largely superseded \
         by the `admin` tag's read surface; kept for /healthz and its own \
         index migration."),
        (name = "health", description = "Liveness probe: unauthenticated, \
         un-ETagged, carries nothing sensitive."),
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
