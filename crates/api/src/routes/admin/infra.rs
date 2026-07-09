//! `GET /v1/admin/infra` — section G, money & infra (env-gated, static v1):
//! HARD CAP display with live spend explicitly unavailable (G1, fail
//! closed), static scheduler/queue mirror + local-vs-cloud_run environment
//! badge via `K_SERVICE` (G2), terraform not surfaced v1 (G3).
//!
//! This endpoint touches NO GCP APIs and never errors locally: everything it
//! serves is either a compile-time mirror of the terraform sources or a
//! process-environment probe. When a figure cannot be observed (live spend)
//! it is `null` with an explicit reason — never a fake number.

use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::error::ErrorBody;

// ------------------------------------------------------------ static mirror --

/// One Cloud Scheduler job, mirrored statically from `infra/scheduler.tf`
/// (names, schedules, paused state, descriptions — verified against the file,
/// not GCP).
struct SchedulerMirror {
    name: &'static str,
    schedule: &'static str,
    description: &'static str,
}

/// All jobs share `time_zone = "Etc/UTC"` in `infra/scheduler.tf`.
const SCHEDULER_TIME_ZONE: &str = "Etc/UTC";

/// `infra/scheduler.tf`: three discover cadence stubs (`var.discover_tiers`)
/// plus the sentinel watch — ALL created `paused = true` (unpausing is the
/// explicit go-live act).
const SCHEDULER_MIRROR: [SchedulerMirror; 4] = [
    SchedulerMirror {
        name: "govfolio-discover-tier1",
        schedule: "*/5 * * * *",
        description: "US House/Senate transaction reports (design 5.5 tier 1)",
    },
    SchedulerMirror {
        name: "govfolio-discover-tier2",
        schedule: "0 * * * *",
        description: "Change-notification registers: UK/AU/CA (tier 2)",
    },
    SchedulerMirror {
        name: "govfolio-discover-tier3",
        schedule: "0 6 * * *",
        description: "Annual-declaration regimes: EU-P/FR/DE/... (tier 3)",
    },
    SchedulerMirror {
        name: "govfolio-sentinel-watch",
        schedule: "0 6 * * 1",
        description: "Weekly source drift defense (sentinel WATCH, goal 017).",
    },
];

/// `infra/tasks.tf`: one Cloud Tasks queue per pipeline stage
/// (`govfolio-${each.key}` over `var.pipeline_queues`).
const QUEUE_MIRROR: [&str; 5] = [
    "govfolio-discover",
    "govfolio-fetch",
    "govfolio-parse",
    "govfolio-normalize",
    "govfolio-publish",
];

/// Monthly billing HARD CAP as a decimal string (invariant 7: money is never
/// a float).
const HARD_CAP_USD: &str = "200";

/// Where the cap number comes from.
const HARD_CAP_SOURCE: &str = "goal 021 HALT resolution 2026-07-08";

// ------------------------------------------------------------- wire shapes --

/// G1 — the monthly HARD CAP and the (unavailable) live spend.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminBudget {
    /// Monthly billing ceiling in USD, decimal string (never a float).
    pub hard_cap_usd: String,
    /// Provenance of the cap figure.
    pub source: String,
    /// Live month-to-date spend in USD. Always `null` in v1 — no billing
    /// export is wired; rendering a guess would violate fail-closed.
    pub live_spend: Option<String>,
    /// Why `live_spend` is `null`.
    pub live_spend_unavailable_reason: String,
}

/// One Cloud Scheduler job as declared in terraform (static mirror, not a
/// live GCP read).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminScheduler {
    /// Job name (e.g. `govfolio-discover-tier1`).
    pub name: String,
    /// Cron schedule as declared.
    pub schedule: String,
    /// Schedule time zone.
    pub time_zone: String,
    /// Declared paused state (`true` for every job in v1 — unpausing is the
    /// explicit go-live act).
    pub paused: bool,
    /// Job description as declared.
    pub description: String,
}

/// Section G — money & infra, static v1.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminInfra {
    /// When this snapshot was computed.
    pub generated_at: DateTime<Utc>,
    /// `cloud_run` when `K_SERVICE` is present in the process environment,
    /// otherwise `local`.
    pub environment: String,
    /// HARD CAP + live spend (G1).
    pub budget: AdminBudget,
    /// Cloud Scheduler jobs as declared in terraform (G2).
    pub schedulers: Vec<AdminScheduler>,
    /// Provenance of `schedulers` — a file mirror, not a GCP read.
    pub schedulers_source: String,
    /// Cloud Tasks queue names as declared in terraform (G2).
    pub queues: Vec<String>,
    /// Provenance of `queues` — a file mirror, not a GCP read.
    pub queues_source: String,
    /// G3: terraform state is not surfaced in v1.
    pub terraform_note: String,
}

// ------------------------------------------------------------- the handler --

/// Money & infra observability (section G), static v1: HARD CAP, terraform
/// scheduler/queue mirror, environment badge. Touches no GCP APIs and never
/// errors locally.
#[utoipa::path(
    get,
    path = "/v1/admin/infra",
    tag = "admin",
    responses(
        (status = 200, description = "Money & infra snapshot (static mirror)", body = AdminInfra),
        (status = 401, description = "Missing or invalid admin token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
#[allow(clippy::unused_async)] // axum handlers must be async; this one is pure
pub async fn admin_infra() -> Json<AdminInfra> {
    let environment = if std::env::var("K_SERVICE").is_ok() {
        "cloud_run"
    } else {
        "local"
    };
    Json(AdminInfra {
        generated_at: Utc::now(),
        environment: environment.to_owned(),
        budget: AdminBudget {
            hard_cap_usd: HARD_CAP_USD.to_owned(),
            source: HARD_CAP_SOURCE.to_owned(),
            live_spend: None,
            live_spend_unavailable_reason: "no billing export is wired; live spend is not \
                                            observable from this service (fail closed — no \
                                            fake numbers)"
                .to_owned(),
        },
        schedulers: SCHEDULER_MIRROR
            .iter()
            .map(|job| AdminScheduler {
                name: job.name.to_owned(),
                schedule: job.schedule.to_owned(),
                time_zone: SCHEDULER_TIME_ZONE.to_owned(),
                paused: true,
                description: job.description.to_owned(),
            })
            .collect(),
        schedulers_source: "static mirror of infra/scheduler.tf".to_owned(),
        queues: QUEUE_MIRROR.iter().map(|&q| q.to_owned()).collect(),
        queues_source: "static mirror of infra/tasks.tf".to_owned(),
        terraform_note: "terraform state is not surfaced in v1".to_owned(),
    })
}
