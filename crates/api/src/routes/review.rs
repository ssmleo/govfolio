//! Review-queue admin endpoints (design §7.1–7.2, goal 041a): the
//! priority-ranked queue, the task detail (full target record + extraction
//! context — the LLM pre-review surface), the resolve door, and the audit
//! log.
//!
//! WRITE AUTHORITY: every resolution goes through
//! `pipeline::promote::resolve_review_task` — the ONE sanctioned write path
//! for adjudications (supersede-never-update, invariant 1). This module
//! contains no `UPDATE` of `disclosure_record`, ever; the API is a thin door.
//!
//! AUTH: none yet — accounts land in goal 050; until then `reviewer` is
//! caller-supplied free text, recorded verbatim in the audit log.
//!
//! SILVER-PAYLOAD GAP (documented follow-up): design §7.2 wants the record's
//! Silver staging row beside the extraction context. Gold rows do not store
//! their staging ordinal, and staging tables are per-regime (`stg_<regime>`),
//! so no regime-generic, `SqlSafeStr`-clean join exists today. Rather than
//! hack a us_house-only join into `/v1`, the detail serves the extraction
//! context WITHOUT the staging payload; clean linkage (e.g. expand-only
//! `stg_table`/`stg_id` columns on `disclosure_record`) is follow-up work.

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use const_format::concatcp;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use govfolio_core::domain::enums::{RecordType, VerificationState};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use pipeline::promote::{ResolveAudit, ResolveOutcome, Verdict};

use crate::AppState;
use crate::dto::{from_token, validate_page_params, value_from_columns};
use crate::error::{ApiError, ErrorBody};
use crate::extract::{ApiJson, ApiQuery};
use crate::routes::records::{RecordDetail, fetch_record_detail};

// ------------------------------------------------------------- wire shapes --

/// One `review_task` row (design §4.2).
#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewTask {
    /// Task ULID (time-sortable; the queue pagination cursor).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// What the task targets (e.g. `disclosure_record`).
    pub target_kind: String,
    /// Id of the targeted row.
    pub target_id: String,
    /// Why the task was opened (e.g. `ptr_amendment_unlinked`).
    pub reason: String,
    /// Queue rank: impact × uncertainty (design §7.2); higher reviews first.
    pub priority_score: f32,
    /// `open` | `resolved` | `dismissed`.
    pub status: String,
    /// Claimed reviewer, when assigned.
    pub assignee: Option<String>,
    /// Verdict payload once resolved.
    #[schema(value_type = Option<Object>)]
    pub resolution: Option<serde_json::Value>,
    /// When the task was opened.
    pub created_at: DateTime<Utc>,
    /// When the task was resolved.
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Target-record summary on a queue item — the reviewer's scan surface.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewTargetSummary {
    /// The targeted `disclosure_record`.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub record_id: String,
    /// Asset description exactly as filed (invariant 2).
    pub asset_description_raw: String,
    /// Canonical name of the politician the record concerns.
    pub politician_name: String,
    /// One of the four observation types.
    pub record_type: RecordType,
    /// Declared value band; bounds are decimal STRINGS (invariant 7).
    pub value: Option<ValueInterval>,
    /// Two-stage publication state (design §7.1).
    pub verification_state: VerificationState,
    /// Extractor confidence in `[0, 1]`.
    pub extraction_confidence: Option<f32>,
    /// Parser id / model+prompt version that produced the record.
    pub extracted_by: String,
}

/// One queue entry: the task plus its target-record summary (`null` when the
/// task targets something other than a disclosure record).
#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewQueueItem {
    /// The task itself.
    pub task: ReviewTask,
    /// Target-record summary for `disclosure_record` tasks.
    pub record: Option<ReviewTargetSummary>,
}

/// One page of the review queue, ranked `priority_score` desc, then
/// `created_at` asc (oldest first within a priority), then id.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewQueuePage {
    /// Queue entries in ranking order.
    pub items: Vec<ReviewQueueItem>,
    /// Pass back as `cursor` to fetch the page after this one; `null` at the
    /// end. Ranking (not id order) is preserved across pages.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub next_cursor: Option<String>,
}

/// Extraction-cache evidence for the task's document: which model produced
/// the cached extraction and how (`provenance` carries the cross-check
/// status when the live call recorded one). Latest entry for the document's
/// sha + the record's extractor tag; absent for deterministic (non-LLM)
/// parses.
#[derive(Debug, Serialize, ToSchema)]
pub struct ExtractionCacheInfo {
    /// Model that produced the cached extraction.
    pub model_id: String,
    /// When the extraction was cached.
    pub cached_at: DateTime<Utc>,
    /// How the entry was produced (audit surface): source, models, and
    /// `cross_checked` verdict when a second model re-extracted (design §5.3).
    #[schema(value_type = Object)]
    pub provenance: serde_json::Value,
}

/// The LLM pre-review note (design §7.2): who/what extracted the target
/// record, with what confidence, and the extraction-cache evidence when the
/// LLM seam produced it. The record's Silver staging payload is NOT included
/// yet (see the module-level silver-payload gap note).
#[derive(Debug, Serialize, ToSchema)]
pub struct ExtractionContext {
    /// Parser id / model+prompt version that produced the record.
    pub extracted_by: String,
    /// Extractor confidence in `[0, 1]`.
    pub extraction_confidence: Option<f32>,
    /// Cache evidence for LLM-extracted documents; `null` for deterministic
    /// parses.
    pub cache: Option<ExtractionCacheInfo>,
}

/// One review task with its full adjudication surface: the target record in
/// the public trust shape (provenance + supersession history) and the
/// extraction context.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReviewTaskDetail {
    /// The task itself.
    pub task: ReviewTask,
    /// Full target record (the 040a `RecordDetail` shape) for
    /// `disclosure_record` tasks.
    pub record: Option<RecordDetail>,
    /// The LLM pre-review note; present when the target record is.
    pub extraction: Option<ExtractionContext>,
}

/// Resolve request: reviewer identity plus one verdict. `regime_code` and
/// `corrected` travel only with `verdict = "edit"` (they select the details
/// contract and carry the corrected facts); supplying them with any other
/// verdict fails closed. Corrected identity fields (`filing_id`,
/// `politician_id`, `regime_id`) may be omitted — promote pins identity from
/// the original row and ignores reviewer-supplied values.
#[derive(Debug, Deserialize, ToSchema)]
pub struct ResolveRequest {
    /// Reviewer identity — free text until accounts land (goal 050).
    pub reviewer: String,
    /// `confirm` | `reject` | `edit`.
    pub verdict: String,
    /// Optional note, recorded verbatim in the audit log.
    pub note: Option<String>,
    /// Details-registry arm for an edit (e.g. `us_house`).
    pub regime_code: Option<String>,
    /// Corrected facts for an edit, in the `GoldCandidate` wire shape.
    #[schema(value_type = Option<Object>)]
    pub corrected: Option<serde_json::Value>,
}

/// What an applied resolution did (non-applied attempts surface as errors:
/// `409` for an already-resolved task, `5xx` for a failed resolution).
#[derive(Debug, Serialize, ToSchema)]
pub struct ResolveResponse {
    /// Always `applied`.
    pub outcome: String,
    /// The adjudicated record.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub record_id: String,
    /// The superseding `corrected` record an edit inserted (`null` for
    /// confirm/reject).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub superseding_record_id: Option<String>,
}

/// One resolve attempt in the audit log — exactly one row per attempt,
/// whatever came of it.
#[derive(Debug, Serialize, ToSchema, sqlx::FromRow)]
pub struct ReviewAuditEntry {
    /// Audit row ULID (time-sortable).
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub id: String,
    /// The task the attempt adjudicated.
    #[schema(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub review_task_id: String,
    /// Who attempted (free text until goal 050 auth).
    pub reviewer: String,
    /// `confirm` | `edit` | `reject`.
    pub verdict: String,
    /// `applied` | `conflict` (task already resolved) | `failed` (rolled
    /// back whole).
    pub outcome: String,
    /// Reviewer note, verbatim.
    pub note: Option<String>,
    /// Record ids the attempt touched (`[]` for non-applied attempts).
    #[schema(value_type = Vec<String>)]
    pub affected_record_ids: serde_json::Value,
    /// When the attempt happened.
    pub created_at: DateTime<Utc>,
}

// -------------------------------------------------------------- the queue --

/// Query parameters of `GET /v1/review-tasks`.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReviewQueueParams {
    /// Task status filter: `open` (default) | `resolved` | `dismissed`.
    pub status: Option<String>,
    /// Pagination cursor: the task id of the last item on the previous page.
    #[param(pattern = "^[0-7][0-9A-HJKMNP-TV-Z]{25}$")]
    pub cursor: Option<String>,
    /// Page size, `1..=200`; defaults to 50.
    #[param(minimum = 1, maximum = 200)]
    pub limit: Option<u32>,
}

/// Raw queue row: the task plus the LEFT-joined target-record summary.
#[derive(Debug, sqlx::FromRow)]
struct QueueRow {
    id: String,
    target_kind: String,
    target_id: String,
    reason: String,
    priority_score: f32,
    status: String,
    assignee: Option<String>,
    resolution: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
    record_id: Option<String>,
    asset_description_raw: Option<String>,
    politician_name: Option<String>,
    record_type: Option<String>,
    value_low: Option<Decimal>,
    value_high: Option<Decimal>,
    currency: Option<String>,
    verification_state: Option<String>,
    extraction_confidence: Option<f32>,
    extracted_by: Option<String>,
}

impl TryFrom<QueueRow> for ReviewQueueItem {
    type Error = ApiError;

    fn try_from(row: QueueRow) -> Result<Self, Self::Error> {
        let record = match row.record_id {
            None => None,
            Some(record_id) => {
                // The LEFT joins hit: every summary column is FK-backed, so a
                // hole here is data corruption, never a silent null.
                let take = |what: &str, value: Option<String>| {
                    value.ok_or_else(|| {
                        ApiError::from(anyhow::anyhow!(
                            "record {record_id} joined without {what} — data corruption"
                        ))
                    })
                };
                Some(ReviewTargetSummary {
                    asset_description_raw: take(
                        "asset_description_raw",
                        row.asset_description_raw,
                    )?,
                    politician_name: take("politician canonical_name", row.politician_name)?,
                    record_type: from_token(take("record_type", row.record_type)?, "record_type")?,
                    value: value_from_columns(row.value_low, row.value_high, row.currency)?,
                    verification_state: from_token(
                        take("verification_state", row.verification_state)?,
                        "verification_state",
                    )?,
                    extraction_confidence: row.extraction_confidence,
                    extracted_by: take("extracted_by", row.extracted_by)?,
                    record_id,
                })
            }
        };
        Ok(Self {
            task: ReviewTask {
                id: row.id,
                target_kind: row.target_kind,
                target_id: row.target_id,
                reason: row.reason,
                priority_score: row.priority_score,
                status: row.status,
                assignee: row.assignee,
                resolution: row.resolution,
                created_at: row.created_at,
                resolved_at: row.resolved_at,
            },
            record,
        })
    }
}

/// Task projection shared by the queue and detail statements.
const TASK_COLUMNS: &str = "rt.id, rt.target_kind, rt.target_id, rt.reason, rt.priority_score, \
     rt.status, rt.assignee, rt.resolution, rt.created_at, rt.resolved_at";

/// Queue projection: task + LEFT-joined target summary (tasks may target
/// kinds other than `disclosure_record`; those simply carry no summary).
const QUEUE_COLUMNS: &str = concatcp!(
    "select ",
    TASK_COLUMNS,
    ", r.id as record_id, r.asset_description_raw, p.canonical_name as politician_name, \
     r.record_type, r.value_low, r.value_high, r.currency, r.verification_state, \
     r.extraction_confidence, r.extracted_by \
     from review_task rt \
     left join disclosure_record r \
       on rt.target_kind = 'disclosure_record' and r.id = rt.target_id \
     left join politician p on p.id = r.politician_id "
);

/// Ranking (design §7.2: priority = impact × uncertainty, highest first;
/// FIFO within a priority) with keyset pagination. The cursor is the last
/// task's ULID; its (`priority_score`, `created_at`) anchor arrives as
/// `$3`/`$4`, and the strictly-after predicate mirrors the ORDER BY
/// (`priority_score` desc, `created_at` asc, `id` asc — id is the total-order
/// tiebreak).
const QUEUE_SQL: &str = concatcp!(
    QUEUE_COLUMNS,
    "where rt.status = $1 \
       and ($2::text is null \
            or rt.priority_score < $3::real \
            or (rt.priority_score = $3::real \
                and (rt.created_at > $4::timestamptz \
                     or (rt.created_at = $4::timestamptz and rt.id > $2)))) \
     order by rt.priority_score desc, rt.created_at asc, rt.id asc \
     limit $5"
);

/// Lists review tasks in ranking order (design §7.2 queue).
///
/// # Errors
/// `400` on a malformed status, cursor or limit; `500` on backend failure —
/// all in the consistent error envelope.
#[utoipa::path(
    get,
    path = "/v1/review-tasks",
    tag = "review",
    params(ReviewQueueParams),
    responses(
        (status = 200, description = "One page of the review queue, ranked priority desc then age", body = ReviewQueuePage),
        (status = 400, description = "Malformed status, cursor or limit", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn list_review_tasks(
    State(state): State<AppState>,
    ApiQuery(params): ApiQuery<ReviewQueueParams>,
) -> Result<Json<ReviewQueuePage>, ApiError> {
    let (cursor, limit) = validate_page_params(params.cursor.as_deref(), params.limit)?;
    let status_filter = params.status.unwrap_or_else(|| "open".to_owned());
    if !matches!(status_filter.as_str(), "open" | "resolved" | "dismissed") {
        return Err(ApiError::bad_request(
            "invalid_status",
            format!("status must be open|resolved|dismissed, got {status_filter:?}"),
        ));
    }
    // Dereference the cursor into its ranking anchor — the cursor stays a
    // plain task ULID on the wire while the ranking keyset stays exact.
    let anchor: Option<(f32, DateTime<Utc>)> = match &cursor {
        None => None,
        Some(id) => Some(
            sqlx::query_as("select priority_score, created_at from review_task where id = $1")
                .bind(id)
                .fetch_optional(&state.pool)
                .await?
                .ok_or_else(|| {
                    ApiError::bad_request("invalid_cursor", "cursor does not name a review task")
                })?,
        ),
    };
    let rows: Vec<QueueRow> = sqlx::query_as(QUEUE_SQL)
        .bind(&status_filter)
        .bind(&cursor)
        .bind(anchor.map(|(priority, _)| priority))
        .bind(anchor.map(|(_, created)| created))
        .bind(limit + 1)
        .fetch_all(&state.pool)
        .await?;

    let page_len = usize::try_from(limit).map_err(|e| anyhow::anyhow!("limit fits usize: {e}"))?;
    let has_more = rows.len() > page_len;
    let items = rows
        .into_iter()
        .take(page_len)
        .map(ReviewQueueItem::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    let next_cursor = if has_more {
        items.last().map(|item| item.task.id.clone())
    } else {
        None
    };
    Ok(Json(ReviewQueuePage { items, next_cursor }))
}

// ------------------------------------------------------------- the detail --

/// Raw `review_task` row for the detail/resolve doors.
#[derive(Debug, sqlx::FromRow)]
struct TaskRow {
    id: String,
    target_kind: String,
    target_id: String,
    reason: String,
    priority_score: f32,
    status: String,
    assignee: Option<String>,
    resolution: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
}

impl From<TaskRow> for ReviewTask {
    fn from(row: TaskRow) -> Self {
        Self {
            id: row.id,
            target_kind: row.target_kind,
            target_id: row.target_id,
            reason: row.reason,
            priority_score: row.priority_score,
            status: row.status,
            assignee: row.assignee,
            resolution: row.resolution,
            created_at: row.created_at,
            resolved_at: row.resolved_at,
        }
    }
}

const TASK_SQL: &str = concatcp!(
    "select ",
    TASK_COLUMNS,
    " from review_task rt where rt.id = $1"
);

async fn fetch_task(state: &AppState, id: &str) -> Result<Option<TaskRow>, ApiError> {
    Ok(sqlx::query_as(TASK_SQL)
        .bind(id)
        .fetch_optional(&state.pool)
        .await?)
}

/// Fetches one review task with its full adjudication surface.
///
/// # Errors
/// `404` for an unknown task; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/review-tasks/{id}",
    tag = "review",
    params(("id" = String, Path, description = "Review task ULID")),
    responses(
        (status = 200, description = "The task with its target record and extraction context", body = ReviewTaskDetail),
        (status = 404, description = "Unknown task", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn get_review_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ReviewTaskDetail>, ApiError> {
    let Some(task_row) = fetch_task(&state, &id).await? else {
        return Err(ApiError::NotFound {
            message: format!("review task {id} not found"),
        });
    };
    let task = ReviewTask::from(task_row);

    let mut record = None;
    let mut extraction = None;
    if task.target_kind == "disclosure_record" {
        // Real-time visibility: the review surface sits behind the admin
        // gate (lib.rs), and reviewers must see records the moment they
        // exist — the freemium delay is a public-tier concern.
        record = fetch_record_detail(
            &state,
            &govfolio_core::query::RecordFilter::default(),
            &task.target_id,
        )
        .await?;
        if let Some(detail) = &record {
            // Cache evidence keyed by the Bronze sha + the record's extractor
            // tag (design §5.3 cache address). Multiple model_ids mean a
            // model-version bump: the latest entry is the one current
            // extractions answer from.
            let cache: Option<(String, DateTime<Utc>, serde_json::Value)> = sqlx::query_as(
                "select model_id, created_at, provenance from extraction_cache \
                 where document_sha256 = $1 and extractor_tag = $2 \
                 order by created_at desc limit 1",
            )
            .bind(&detail.provenance.raw_document.sha256)
            .bind(&detail.record.extracted_by)
            .fetch_optional(&state.pool)
            .await?;
            extraction = Some(ExtractionContext {
                extracted_by: detail.record.extracted_by.clone(),
                extraction_confidence: detail.record.extraction_confidence,
                cache: cache.map(|(model_id, cached_at, provenance)| ExtractionCacheInfo {
                    model_id,
                    cached_at,
                    provenance,
                }),
            });
        }
    }
    Ok(Json(ReviewTaskDetail {
        task,
        record,
        extraction,
    }))
}

// ------------------------------------------------------------ the resolve --

/// The nil-ULID placeholder promote replaces with the original row's pinned
/// identity — reviewer-supplied identity is ignored by design.
const NIL_ULID: &str = "00000000000000000000000000";

/// Deserializes the corrected facts; absent identity keys default to the nil
/// placeholder rather than forcing the client to invent ULIDs that would be
/// ignored anyway.
fn parse_corrected(mut corrected: serde_json::Value) -> Result<GoldCandidate, ApiError> {
    let Some(map) = corrected.as_object_mut() else {
        return Err(ApiError::bad_request(
            "invalid_corrected",
            "corrected must be an object in the GoldCandidate wire shape",
        ));
    };
    for key in ["filing_id", "politician_id", "regime_id"] {
        map.entry(key)
            .or_insert_with(|| serde_json::Value::String(NIL_ULID.to_owned()));
    }
    serde_json::from_value(corrected)
        .map_err(|e| ApiError::bad_request("invalid_corrected", format!("corrected: {e}")))
}

/// Maps the request body onto the promote verdict — pure translation, zero
/// semantics of its own.
fn parse_verdict(body: &ResolveRequest) -> Result<Verdict, ApiError> {
    let edit_payload = body.regime_code.is_some() || body.corrected.is_some();
    match body.verdict.as_str() {
        "confirm" | "reject" if edit_payload => Err(ApiError::bad_request(
            "invalid_edit",
            format!(
                "regime_code/corrected only travel with verdict \"edit\", got {:?} — \
                 refusing to silently drop a correction",
                body.verdict
            ),
        )),
        "confirm" => Ok(Verdict::Confirm),
        "reject" => Ok(Verdict::Reject),
        "edit" => {
            let (Some(regime_code), Some(corrected)) = (&body.regime_code, &body.corrected) else {
                return Err(ApiError::bad_request(
                    "invalid_edit",
                    "verdict \"edit\" requires both regime_code and corrected",
                ));
            };
            Ok(Verdict::Edit {
                regime_code: regime_code.clone(),
                corrected: Box::new(parse_corrected(corrected.clone())?),
            })
        }
        other => Err(ApiError::bad_request(
            "invalid_verdict",
            format!("verdict must be confirm|reject|edit, got {other:?}"),
        )),
    }
}

/// Resolves one review task through the pipeline promote path (the single
/// write authority; design §7.2). Every attempt that carries a verdict is
/// audit-logged — applied, conflicting and failed alike.
///
/// # Errors
/// `400` on a malformed body; `404` for an unknown task; `409` when the task
/// is already resolved; `500` when the resolution fails closed (e.g. a
/// correction violating the details contract) — the attempt is still
/// audit-logged.
#[utoipa::path(
    post,
    path = "/v1/review-tasks/{id}/resolve",
    tag = "review",
    params(("id" = String, Path, description = "Review task ULID")),
    request_body = ResolveRequest,
    responses(
        (status = 200, description = "The verdict was applied", body = ResolveResponse),
        (status = 400, description = "Malformed reviewer, verdict or corrected payload", body = ErrorBody),
        (status = 404, description = "Unknown task", body = ErrorBody),
        (status = 409, description = "The task is already resolved", body = ErrorBody),
        (status = 500, description = "The resolution failed closed and rolled back whole", body = ErrorBody),
    ),
)]
pub async fn resolve_review_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ApiJson(body): ApiJson<ResolveRequest>,
) -> Result<Json<ResolveResponse>, ApiError> {
    if body.reviewer.trim().is_empty() {
        return Err(ApiError::bad_request(
            "invalid_reviewer",
            "reviewer must be non-empty (free text until accounts land, goal 050)",
        ));
    }
    let verdict = parse_verdict(&body)?;
    // Existence pre-check (a read, not an adjudication): 404 beats an opaque
    // promote error, and failed-attempt audit rows stay FK-valid.
    if fetch_task(&state, &id).await?.is_none() {
        return Err(ApiError::NotFound {
            message: format!("review task {id} not found"),
        });
    }
    let audit = ResolveAudit {
        reviewer: body.reviewer.clone(),
        note: body.note.clone(),
    };
    match pipeline::promote::resolve_review_task(&state.pool, &id, verdict, Some(&audit)).await {
        Ok(ResolveOutcome::Applied {
            record_id,
            superseding_record_id,
        }) => Ok(Json(ResolveResponse {
            outcome: "applied".to_owned(),
            record_id,
            superseding_record_id,
        })),
        Ok(ResolveOutcome::AlreadyResolved) => Err(ApiError::Conflict {
            code: "already_resolved",
            message: format!("review task {id} is already resolved"),
        }),
        // Fail closed: promote rolled back whole and audit-logged the
        // attempt; details stay server-side (the envelope stays generic).
        Err(error) => Err(ApiError::Internal(error)),
    }
}

// -------------------------------------------------------------- the audit --

/// The audit log of one task: every resolve attempt, in order.
///
/// # Errors
/// `404` for an unknown task; `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/review-tasks/{id}/audit",
    tag = "review",
    params(("id" = String, Path, description = "Review task ULID")),
    responses(
        (status = 200, description = "All resolve attempts, oldest first", body = [ReviewAuditEntry]),
        (status = 404, description = "Unknown task", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn review_task_audit(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<ReviewAuditEntry>>, ApiError> {
    if fetch_task(&state, &id).await?.is_none() {
        return Err(ApiError::NotFound {
            message: format!("review task {id} not found"),
        });
    }
    let entries: Vec<ReviewAuditEntry> = sqlx::query_as(
        "select id, review_task_id, reviewer, verdict, outcome, note, \
                affected_record_ids, created_at \
         from review_audit where review_task_id = $1 order by id",
    )
    .bind(&id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(entries))
}
