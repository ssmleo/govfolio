use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use thiserror::Error;
use ulid::Ulid;

pub const INTEGRATOR_ACTOR: &str = "integrator";
const APPLY_ADVISORY_LOCK_KEY: i64 = 5_139_812_679_124;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CoveragePhase {
    Stub,
    Scouted,
    Surveyed,
    Sampled,
    Specced,
    Built,
    Live,
    Blocked,
}

impl CoveragePhase {
    pub const ALL: [Self; 8] = [
        Self::Stub,
        Self::Scouted,
        Self::Surveyed,
        Self::Sampled,
        Self::Specced,
        Self::Built,
        Self::Live,
        Self::Blocked,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Stub => "stub",
            Self::Scouted => "scouted",
            Self::Surveyed => "surveyed",
            Self::Sampled => "sampled",
            Self::Specced => "specced",
            Self::Built => "built",
            Self::Live => "live",
            Self::Blocked => "blocked",
        }
    }

    #[must_use]
    pub const fn allows(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Stub, Self::Scouted)
                | (Self::Scouted, Self::Surveyed)
                | (Self::Surveyed, Self::Sampled)
                | (Self::Sampled, Self::Specced)
                | (Self::Specced, Self::Built)
                | (Self::Built, Self::Live)
                | (
                    Self::Stub
                        | Self::Scouted
                        | Self::Surveyed
                        | Self::Sampled
                        | Self::Specced
                        | Self::Built,
                    Self::Blocked,
                )
        )
    }
}

impl fmt::Display for CoveragePhase {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for CoveragePhase {
    type Err = IntegrationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "stub" => Ok(Self::Stub),
            "scouted" => Ok(Self::Scouted),
            "surveyed" => Ok(Self::Surveyed),
            "sampled" => Ok(Self::Sampled),
            "specced" => Ok(Self::Specced),
            "built" => Ok(Self::Built),
            "live" => Ok(Self::Live),
            "blocked" => Ok(Self::Blocked),
            _ => Err(IntegrationError::Integrity(format!(
                "unknown persisted coverage phase {value:?}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProducerProvider {
    Claude,
    Codex,
}

impl ProducerProvider {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

impl FromStr for ProducerProvider {
    type Err = IntegrationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            _ => Err(IntegrationError::Integrity(format!(
                "unknown persisted producer provider {value:?}"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, JsonSchema, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IntegrationState {
    Submitted,
    Preparing,
    AwaitingCi,
    MergedUnapplied,
    Applied,
    ReworkRequired,
    Deferred,
}

impl IntegrationState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Submitted => "submitted",
            Self::Preparing => "preparing",
            Self::AwaitingCi => "awaiting_ci",
            Self::MergedUnapplied => "merged_unapplied",
            Self::Applied => "applied",
            Self::ReworkRequired => "rework_required",
            Self::Deferred => "deferred",
        }
    }

    #[must_use]
    pub const fn allows(self, to: Self) -> bool {
        matches!(
            (self, to),
            (Self::Submitted, Self::Preparing)
                | (Self::Preparing, Self::AwaitingCi)
                | (Self::AwaitingCi, Self::MergedUnapplied)
                | (Self::Preparing | Self::AwaitingCi, Self::ReworkRequired)
                | (Self::ReworkRequired, Self::Preparing | Self::Deferred)
        )
    }

    #[must_use]
    pub const fn allows_apply(self) -> bool {
        matches!(self, Self::MergedUnapplied)
    }
}

impl fmt::Display for IntegrationState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for IntegrationState {
    type Err = IntegrationError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "submitted" => Ok(Self::Submitted),
            "preparing" => Ok(Self::Preparing),
            "awaiting_ci" => Ok(Self::AwaitingCi),
            "merged_unapplied" => Ok(Self::MergedUnapplied),
            "applied" => Ok(Self::Applied),
            "rework_required" => Ok(Self::ReworkRequired),
            "deferred" => Ok(Self::Deferred),
            _ => Err(IntegrationError::Integrity(format!(
                "unknown persisted integration state {value:?}"
            ))),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ValidationEvidence {
    pub name: String,
    pub command: String,
    pub exit_code: i32,
    pub output_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ArtifactHash {
    pub path: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct RealSourceProof {
    pub fetched_at: DateTime<Utc>,
    pub source_url: String,
    pub bronze_sha256: String,
    pub ingestion_run_id: String,
    pub rows_ingested: i64,
}

impl RealSourceProof {
    fn validate(&self) -> Result<(), IntegrationError> {
        require_nonempty("real-source URL", &self.source_url)?;
        require_nonempty("ingestion run id", &self.ingestion_run_id)?;
        require_sha256("Bronze hash", &self.bronze_sha256)?;
        if self.rows_ingested <= 0 {
            return Err(IntegrationError::InvalidReceipt(
                "real-source proof must report at least one ingested row".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct IntegrationReceipt {
    pub id: String,
    pub work_key: String,
    pub jurisdiction_id: String,
    pub from_phase: CoveragePhase,
    pub to_phase: Option<CoveragePhase>,
    pub blocked_reason: Option<String>,
    pub source_sha: String,
    pub base_sha: String,
    pub branch: String,
    pub lane_id: String,
    pub lease_generation: i64,
    pub provider: ProducerProvider,
    pub model: String,
    pub attempt_id: String,
    pub validation_evidence: Vec<ValidationEvidence>,
    pub artifact_hashes: Vec<ArtifactHash>,
    pub real_source_proof: Option<RealSourceProof>,
    pub journal_summary: String,
    pub repair_of: Option<String>,
    pub repair_ordinal: Option<i16>,
}

impl IntegrationReceipt {
    /// Validate the immutable receipt contract before any database work.
    ///
    /// # Errors
    /// Returns [`IntegrationError::InvalidReceipt`] when identifiers, evidence,
    /// transition adjacency, repair lineage fields, or hashes are malformed.
    pub fn validate(&self) -> Result<(), IntegrationError> {
        Ulid::from_string(&self.id).map_err(|error| {
            IntegrationError::InvalidReceipt(format!("receipt id is not a ULID: {error}"))
        })?;
        require_nonempty("work key", &self.work_key)?;
        require_nonempty("jurisdiction id", &self.jurisdiction_id)?;
        require_nonempty("branch", &self.branch)?;
        require_nonempty("lane id", &self.lane_id)?;
        require_nonempty("model", &self.model)?;
        require_nonempty("attempt id", &self.attempt_id)?;
        require_single_line("journal summary", &self.journal_summary)?;
        require_git_sha("source SHA", &self.source_sha)?;
        require_git_sha("base SHA", &self.base_sha)?;
        if self.lease_generation < 0 {
            return Err(IntegrationError::InvalidReceipt(
                "lease generation cannot be negative".to_owned(),
            ));
        }

        if let Some(to_phase) = self.to_phase {
            if !self.from_phase.allows(to_phase) {
                return Err(IntegrationError::InvalidReceipt(format!(
                    "coverage transition {} -> {} is not adjacent",
                    self.from_phase, to_phase
                )));
            }
            if to_phase == CoveragePhase::Blocked {
                let reason = self.blocked_reason.as_deref().ok_or_else(|| {
                    IntegrationError::InvalidReceipt(
                        "blocked transition requires a reason".to_owned(),
                    )
                })?;
                require_single_line("blocked reason", reason)?;
            } else if self.blocked_reason.is_some() {
                return Err(IntegrationError::InvalidReceipt(
                    "blocked reason is only legal for a blocked transition".to_owned(),
                ));
            }
        } else if self.blocked_reason.is_some() {
            return Err(IntegrationError::InvalidReceipt(
                "no-phase receipt cannot carry a blocked reason".to_owned(),
            ));
        }

        if self.from_phase == CoveragePhase::Built && self.to_phase == Some(CoveragePhase::Live) {
            self.real_source_proof.as_ref().ok_or_else(|| {
                IntegrationError::InvalidReceipt(
                    "built -> live requires real-source proof".to_owned(),
                )
            })?;
        }
        if let Some(proof) = &self.real_source_proof {
            proof.validate()?;
        }
        if self.validation_evidence.is_empty() {
            return Err(IntegrationError::InvalidReceipt(
                "at least one validation result is required".to_owned(),
            ));
        }
        for evidence in &self.validation_evidence {
            require_nonempty("validation name", &evidence.name)?;
            require_nonempty("validation command", &evidence.command)?;
            require_sha256("validation output hash", &evidence.output_sha256)?;
            if evidence.exit_code != 0 {
                return Err(IntegrationError::InvalidReceipt(format!(
                    "validation {:?} did not pass",
                    evidence.name
                )));
            }
        }
        for artifact in &self.artifact_hashes {
            require_nonempty("artifact path", &artifact.path)?;
            require_sha256("artifact hash", &artifact.sha256)?;
        }
        match (&self.repair_of, self.repair_ordinal) {
            (None, None) => {}
            (Some(repair_of), Some(1..=2)) => {
                if repair_of == &self.id {
                    return Err(IntegrationError::InvalidReceipt(
                        "repair receipt cannot repair itself".to_owned(),
                    ));
                }
                Ulid::from_string(repair_of).map_err(|error| {
                    IntegrationError::InvalidReceipt(format!(
                        "repair receipt id is not a ULID: {error}"
                    ))
                })?;
            }
            _ => {
                return Err(IntegrationError::InvalidReceipt(
                    "repair_of and repair_ordinal 1..=2 must be supplied together".to_owned(),
                ));
            }
        }
        Ok(())
    }

    /// Hash the semantic payload used to detect same-key/different-evidence conflicts.
    ///
    /// # Errors
    /// Returns [`IntegrationError::Serialization`] if the typed payload cannot be
    /// serialized to canonical struct-order JSON.
    pub fn payload_sha256(&self) -> Result<String, IntegrationError> {
        let mut payload = self.clone();
        payload.id.clear();
        let bytes = serde_json::to_vec(&payload)?;
        Ok(hex::encode(Sha256::digest(bytes)))
    }
}

#[derive(Clone, Debug, Default, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct TransitionEvidence {
    pub candidate_base_sha: Option<String>,
    pub integration_branch: Option<String>,
    pub pr_number: Option<i64>,
    pub merge_sha: Option<String>,
    pub failure: Option<String>,
    pub details: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct TransitionRequest {
    pub receipt_id: String,
    pub expected_state: IntegrationState,
    pub expected_version: i64,
    pub to_state: IntegrationState,
    pub actor: String,
    pub evidence: TransitionEvidence,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct CheckProof {
    pub commit_sha: String,
    pub success: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct RequiredChecks {
    pub rust: CheckProof,
    pub db: CheckProof,
    pub web: CheckProof,
    pub guardrails: CheckProof,
}

#[derive(Clone, Debug, Deserialize, Eq, JsonSchema, PartialEq, Serialize)]
pub struct ApplyEvidence {
    pub actor: String,
    pub source_sha: String,
    pub merge_sha: String,
    pub origin_main_sha: String,
    pub source_is_ancestor: bool,
    pub journal_receipt_line_count: i32,
    pub required_checks: RequiredChecks,
    pub real_source_verified: bool,
}

impl ApplyEvidence {
    #[must_use]
    pub fn successful(source_sha: &str, merge_sha: &str) -> Self {
        let check = CheckProof {
            commit_sha: merge_sha.to_owned(),
            success: true,
        };
        Self {
            actor: INTEGRATOR_ACTOR.to_owned(),
            source_sha: source_sha.to_owned(),
            merge_sha: merge_sha.to_owned(),
            origin_main_sha: merge_sha.to_owned(),
            source_is_ancestor: true,
            journal_receipt_line_count: 1,
            required_checks: RequiredChecks {
                rust: check.clone(),
                db: check.clone(),
                web: check.clone(),
                guardrails: check,
            },
            real_source_verified: false,
        }
    }

    /// Validate exact source ancestry, merge identity, canonical JOURNAL evidence,
    /// and the four required checks.
    ///
    /// # Errors
    /// Returns [`IntegrationError::InvalidApplyEvidence`] when any proof does not
    /// name the exact expected source/merge or is not successful.
    pub fn validate(
        &self,
        expected_source_sha: &str,
        expected_merge_sha: &str,
    ) -> Result<(), IntegrationError> {
        if self.actor != INTEGRATOR_ACTOR {
            return Err(IntegrationError::InvalidApplyEvidence(
                "apply actor is not the singleton integrator".to_owned(),
            ));
        }
        require_git_sha("apply source SHA", &self.source_sha)?;
        require_git_sha("apply merge SHA", &self.merge_sha)?;
        require_git_sha("verified origin/main SHA", &self.origin_main_sha)?;
        if self.source_sha != expected_source_sha || !self.source_is_ancestor {
            return Err(IntegrationError::InvalidApplyEvidence(
                "exact receipt source ancestry was not proven".to_owned(),
            ));
        }
        if self.merge_sha != expected_merge_sha || self.origin_main_sha != self.merge_sha {
            return Err(IntegrationError::InvalidApplyEvidence(
                "CI/main evidence does not name the exact merge SHA".to_owned(),
            ));
        }
        if self.journal_receipt_line_count != 1 {
            return Err(IntegrationError::InvalidApplyEvidence(
                "exactly one canonical receipt JOURNAL line is required".to_owned(),
            ));
        }
        for (name, check) in [
            ("rust", &self.required_checks.rust),
            ("db", &self.required_checks.db),
            ("web", &self.required_checks.web),
            ("guardrails", &self.required_checks.guardrails),
        ] {
            if !check.success || check.commit_sha != self.merge_sha {
                return Err(IntegrationError::InvalidApplyEvidence(format!(
                    "required {name} check is not green on the exact merge SHA"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmissionResult {
    pub receipt_id: String,
    pub inserted: bool,
    pub state: IntegrationState,
    pub version: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateProjection {
    pub receipt_id: String,
    pub state: IntegrationState,
    pub version: i64,
    pub candidate_base_sha: Option<String>,
    pub integration_branch: Option<String>,
    pub pr_number: Option<i64>,
    pub merge_sha: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppliedReceipt {
    pub receipt_id: String,
    pub state_version: i64,
    pub coverage_phase: CoveragePhase,
    pub lease_released: bool,
    pub already_applied: bool,
}

#[derive(Debug, Error)]
pub enum IntegrationError {
    #[error("invalid integration receipt: {0}")]
    InvalidReceipt(String),
    #[error("invalid integration apply evidence: {0}")]
    InvalidApplyEvidence(String),
    #[error("illegal integration lifecycle transition {from} -> {to}")]
    IllegalTransition {
        from: IntegrationState,
        to: IntegrationState,
    },
    #[error("receipt {0} was not found")]
    ReceiptNotFound(String),
    #[error("jurisdiction {jurisdiction_id} lease/generation/phase does not match the receipt")]
    LeaseMismatch { jurisdiction_id: String },
    #[error("jurisdiction {jurisdiction_id} already has pending receipt {receipt_id}")]
    PendingReceipt {
        jurisdiction_id: String,
        receipt_id: String,
    },
    #[error("idempotency key already belongs to receipt {existing_id} with a different payload")]
    IdempotencyConflict { existing_id: String },
    #[error(
        "receipt {receipt_id} CAS failed: expected {expected_state}@{expected_version}, \
         found {current_state:?}@{current_version:?}"
    )]
    CasConflict {
        receipt_id: String,
        expected_state: IntegrationState,
        expected_version: i64,
        current_state: Option<IntegrationState>,
        current_version: Option<i64>,
    },
    #[error("another integrator owns the product-domain apply transaction")]
    ApplyAuthorityBusy,
    #[error("integration store is inconsistent: {0}")]
    Integrity(String),
    #[error("integration database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("integration JSON serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[derive(FromRow)]
struct ExistingReceiptRow {
    id: String,
    payload_sha256: String,
    state: String,
    version: i64,
}

#[derive(FromRow)]
struct JurisdictionLeaseRow {
    coverage_phase: String,
    claimed_by: Option<String>,
    lease_generation: i64,
    pending_integration_id: Option<String>,
}

#[derive(FromRow)]
struct PriorRepairRow {
    id: String,
    work_key: String,
    jurisdiction_id: String,
    from_phase: String,
    to_phase: Option<String>,
    lane_id: String,
    lease_generation: i64,
    repair_ordinal: Option<i16>,
    state: String,
    version: i64,
}

struct RepairHandoff {
    receipt_id: String,
    state_version: i64,
}

#[derive(FromRow)]
struct ProjectionRow {
    receipt_id: String,
    state: String,
    version: i64,
    candidate_base_sha: Option<String>,
    integration_branch: Option<String>,
    pr_number: Option<i64>,
    merge_sha: Option<String>,
    last_error: Option<String>,
}

impl ProjectionRow {
    fn into_projection(self) -> Result<StateProjection, IntegrationError> {
        Ok(StateProjection {
            receipt_id: self.receipt_id,
            state: self.state.parse()?,
            version: self.version,
            candidate_base_sha: self.candidate_base_sha,
            integration_branch: self.integration_branch,
            pr_number: self.pr_number,
            merge_sha: self.merge_sha,
            last_error: self.last_error,
        })
    }
}

#[derive(FromRow)]
struct ApplyRow {
    receipt_id: String,
    jurisdiction_id: String,
    from_phase: String,
    to_phase: Option<String>,
    blocked_reason: Option<String>,
    lane_id: String,
    receipt_generation: i64,
    source_sha: String,
    real_source_proof: Option<serde_json::Value>,
    state: String,
    state_version: i64,
    state_merge_sha: Option<String>,
    coverage_phase: String,
    claimed_by: Option<String>,
    lease_generation: i64,
    pending_integration_id: Option<String>,
}

#[derive(FromRow)]
struct ReceiptProjectionRow {
    id: String,
    work_key: String,
    jurisdiction_id: String,
    from_phase: String,
    to_phase: Option<String>,
    blocked_reason: Option<String>,
    source_sha: String,
    base_sha: String,
    source_branch: String,
    lane_id: String,
    lease_generation: i64,
    provider: String,
    model: String,
    attempt_id: String,
    validation_evidence: serde_json::Value,
    artifact_hashes: serde_json::Value,
    real_source_proof: Option<serde_json::Value>,
    journal_summary: String,
    repair_of: Option<String>,
    repair_ordinal: Option<i16>,
    state: String,
    version: i64,
    candidate_base_sha: Option<String>,
    integration_branch: Option<String>,
    pr_number: Option<i64>,
    merge_sha: Option<String>,
    last_error: Option<String>,
}

impl ReceiptProjectionRow {
    fn into_typed(self) -> Result<(IntegrationReceipt, StateProjection), IntegrationError> {
        let receipt_id = self.id.clone();
        let receipt = IntegrationReceipt {
            id: self.id,
            work_key: self.work_key,
            jurisdiction_id: self.jurisdiction_id,
            from_phase: self.from_phase.parse()?,
            to_phase: self.to_phase.as_deref().map(str::parse).transpose()?,
            blocked_reason: self.blocked_reason,
            source_sha: self.source_sha,
            base_sha: self.base_sha,
            branch: self.source_branch,
            lane_id: self.lane_id,
            lease_generation: self.lease_generation,
            provider: self.provider.parse()?,
            model: self.model,
            attempt_id: self.attempt_id,
            validation_evidence: serde_json::from_value(self.validation_evidence)?,
            artifact_hashes: serde_json::from_value(self.artifact_hashes)?,
            real_source_proof: self
                .real_source_proof
                .map(serde_json::from_value)
                .transpose()?,
            journal_summary: self.journal_summary,
            repair_of: self.repair_of,
            repair_ordinal: self.repair_ordinal,
        };
        receipt.validate().map_err(|error| {
            IntegrationError::Integrity(format!(
                "persisted receipt {receipt_id} failed its typed contract: {error}"
            ))
        })?;
        let projection = StateProjection {
            receipt_id,
            state: self.state.parse()?,
            version: self.version,
            candidate_base_sha: self.candidate_base_sha,
            integration_branch: self.integration_branch,
            pr_number: self.pr_number,
            merge_sha: self.merge_sha,
            last_error: self.last_error,
        };
        Ok((receipt, projection))
    }
}

/// Atomically submit immutable producer evidence and fence the jurisdiction as
/// integration-pending. Repeating the same semantic payload returns the first receipt.
///
/// # Errors
/// Returns a validation, lease/pending, idempotency, or database error. Evidence
/// commands are persisted as data and are never executed by this function.
pub async fn submit_receipt(
    pool: &PgPool,
    receipt: &IntegrationReceipt,
) -> Result<SubmissionResult, IntegrationError> {
    receipt.validate()?;
    let payload_sha256 = receipt.payload_sha256()?;
    let mut transaction = pool.begin().await?;
    if let Some(existing) = existing_submission(&mut transaction, receipt, &payload_sha256).await? {
        transaction.commit().await?;
        return Ok(existing);
    }
    let lease = lock_submission_lease(&mut transaction, receipt).await?;
    validate_submission_lease(&lease, receipt)?;
    let handoff = prepare_repair_handoff(&mut transaction, &lease, receipt).await?;
    insert_receipt_rows(&mut transaction, receipt, &payload_sha256).await?;
    complete_submission_handoff(&mut transaction, receipt, handoff.as_ref()).await?;
    transaction.commit().await?;
    Ok(SubmissionResult {
        receipt_id: receipt.id.clone(),
        inserted: true,
        state: IntegrationState::Submitted,
        version: 0,
    })
}

async fn existing_submission(
    transaction: &mut Transaction<'_, Postgres>,
    receipt: &IntegrationReceipt,
    payload_sha256: &str,
) -> Result<Option<SubmissionResult>, IntegrationError> {
    let existing = sqlx::query_as::<_, ExistingReceiptRow>(
        "select receipt.id, receipt.payload_sha256, state.state, state.version \
         from integration_receipt receipt \
         join integration_receipt_state state on state.receipt_id = receipt.id \
         where receipt.work_key = $1 and receipt.from_phase = $2 \
           and receipt.to_phase is not distinct from $3 and receipt.source_sha = $4",
    )
    .bind(&receipt.work_key)
    .bind(receipt.from_phase.as_str())
    .bind(receipt.to_phase.map(CoveragePhase::as_str))
    .bind(&receipt.source_sha)
    .fetch_optional(&mut **transaction)
    .await?;
    if let Some(existing) = existing {
        if existing.payload_sha256 != payload_sha256 {
            return Err(IntegrationError::IdempotencyConflict {
                existing_id: existing.id,
            });
        }
        return Ok(Some(SubmissionResult {
            receipt_id: existing.id,
            inserted: false,
            state: existing.state.parse()?,
            version: existing.version,
        }));
    }
    Ok(None)
}

async fn lock_submission_lease(
    transaction: &mut Transaction<'_, Postgres>,
    receipt: &IntegrationReceipt,
) -> Result<JurisdictionLeaseRow, IntegrationError> {
    sqlx::query_as::<_, JurisdictionLeaseRow>(
        "select coverage_phase, claimed_by, lease_generation, pending_integration_id \
         from jurisdiction where id = $1 for update",
    )
    .bind(&receipt.jurisdiction_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| IntegrationError::LeaseMismatch {
        jurisdiction_id: receipt.jurisdiction_id.clone(),
    })
}

fn validate_submission_lease(
    lease: &JurisdictionLeaseRow,
    receipt: &IntegrationReceipt,
) -> Result<(), IntegrationError> {
    if lease.coverage_phase != receipt.from_phase.as_str()
        || lease.claimed_by.as_deref() != Some(receipt.lane_id.as_str())
        || lease.lease_generation != receipt.lease_generation
    {
        return Err(IntegrationError::LeaseMismatch {
            jurisdiction_id: receipt.jurisdiction_id.clone(),
        });
    }
    Ok(())
}

async fn prepare_repair_handoff(
    transaction: &mut Transaction<'_, Postgres>,
    lease: &JurisdictionLeaseRow,
    receipt: &IntegrationReceipt,
) -> Result<Option<RepairHandoff>, IntegrationError> {
    match (
        receipt.repair_of.as_deref(),
        lease.pending_integration_id.as_deref(),
    ) {
        (None, None) => Ok(None),
        (None, Some(pending)) => Err(IntegrationError::PendingReceipt {
            jurisdiction_id: receipt.jurisdiction_id.clone(),
            receipt_id: pending.to_owned(),
        }),
        (Some(repair_of), Some(pending)) if repair_of != pending => {
            Err(IntegrationError::PendingReceipt {
                jurisdiction_id: receipt.jurisdiction_id.clone(),
                receipt_id: pending.to_owned(),
            })
        }
        (Some(_), None) => Err(IntegrationError::InvalidReceipt(
            "repair receipt must replace the current pending receipt".to_owned(),
        )),
        (Some(repair_of), Some(_)) => {
            let prior = sqlx::query_as::<_, PriorRepairRow>(
                "select receipt.id, receipt.work_key, receipt.jurisdiction_id, \
                        receipt.from_phase, receipt.to_phase, receipt.lane_id, \
                        receipt.lease_generation, receipt.repair_ordinal, \
                        state.state, state.version \
                 from integration_receipt receipt \
                 join integration_receipt_state state on state.receipt_id = receipt.id \
                 where receipt.id = $1 for update of state",
            )
            .bind(repair_of)
            .fetch_optional(&mut **transaction)
            .await?
            .ok_or_else(|| {
                IntegrationError::Integrity(format!(
                    "pending repair ancestor {repair_of} does not exist"
                ))
            })?;
            let expected_ordinal = prior
                .repair_ordinal
                .unwrap_or(0)
                .checked_add(1)
                .ok_or_else(|| {
                    IntegrationError::InvalidReceipt("repair ordinal overflow".to_owned())
                })?;
            let same_lineage = prior.id == repair_of
                && prior.work_key == receipt.work_key
                && prior.jurisdiction_id == receipt.jurisdiction_id
                && prior.from_phase == receipt.from_phase.as_str()
                && prior.to_phase.as_deref() == receipt.to_phase.map(CoveragePhase::as_str)
                && prior.lane_id == receipt.lane_id
                && prior.lease_generation == receipt.lease_generation;
            if prior.state != IntegrationState::ReworkRequired.as_str()
                || !same_lineage
                || expected_ordinal > 2
                || receipt.repair_ordinal != Some(expected_ordinal)
            {
                return Err(IntegrationError::InvalidReceipt(
                    "repair must replace rework_required with the next bounded lineage ordinal"
                        .to_owned(),
                ));
            }
            Ok(Some(RepairHandoff {
                receipt_id: prior.id,
                state_version: prior.version,
            }))
        }
    }
}

async fn insert_receipt_rows(
    transaction: &mut Transaction<'_, Postgres>,
    receipt: &IntegrationReceipt,
    payload_sha256: &str,
) -> Result<(), IntegrationError> {
    let validation_evidence = serde_json::to_value(&receipt.validation_evidence)?;
    let artifact_hashes = serde_json::to_value(&receipt.artifact_hashes)?;
    let real_source_proof = receipt
        .real_source_proof
        .as_ref()
        .map(serde_json::to_value)
        .transpose()?;
    sqlx::query(
        "insert into integration_receipt \
         (id, work_key, jurisdiction_id, from_phase, to_phase, blocked_reason, source_sha, \
          base_sha, source_branch, lane_id, lease_generation, provider, model, attempt_id, \
          validation_evidence, artifact_hashes, real_source_proof, journal_summary, \
          repair_of, repair_ordinal, payload_sha256) \
         values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21)",
    )
    .bind(&receipt.id)
    .bind(&receipt.work_key)
    .bind(&receipt.jurisdiction_id)
    .bind(receipt.from_phase.as_str())
    .bind(receipt.to_phase.map(CoveragePhase::as_str))
    .bind(&receipt.blocked_reason)
    .bind(&receipt.source_sha)
    .bind(&receipt.base_sha)
    .bind(&receipt.branch)
    .bind(&receipt.lane_id)
    .bind(receipt.lease_generation)
    .bind(receipt.provider.as_str())
    .bind(&receipt.model)
    .bind(&receipt.attempt_id)
    .bind(validation_evidence)
    .bind(artifact_hashes)
    .bind(real_source_proof)
    .bind(&receipt.journal_summary)
    .bind(&receipt.repair_of)
    .bind(receipt.repair_ordinal)
    .bind(payload_sha256)
    .execute(&mut **transaction)
    .await?;
    sqlx::query(
        "insert into integration_receipt_state (receipt_id, state, version) \
         values ($1, 'submitted', 0)",
    )
    .bind(&receipt.id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn complete_submission_handoff(
    transaction: &mut Transaction<'_, Postgres>,
    receipt: &IntegrationReceipt,
    repair_handoff: Option<&RepairHandoff>,
) -> Result<(), IntegrationError> {
    let previous_pending = repair_handoff.map(|handoff| handoff.receipt_id.as_str());
    if let Some(handoff) = repair_handoff {
        let deferred_version = sqlx::query_scalar::<_, i64>(
            "update integration_receipt_state set state = 'deferred', version = version + 1, \
             updated_at = now() where receipt_id = $1 and state = 'rework_required' \
             and version = $2 returning version",
        )
        .bind(&handoff.receipt_id)
        .bind(handoff.state_version)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| IntegrationError::CasConflict {
            receipt_id: handoff.receipt_id.clone(),
            expected_state: IntegrationState::ReworkRequired,
            expected_version: handoff.state_version,
            current_state: None,
            current_version: None,
        })?;
        sqlx::query(
            "insert into integration_event \
             (receipt_id, from_state, to_state, version, actor, evidence) \
             values ($1, 'rework_required', 'deferred', $2, $3, $4)",
        )
        .bind(&handoff.receipt_id)
        .bind(deferred_version)
        .bind(INTEGRATOR_ACTOR)
        .bind(serde_json::json!({ "repair_receipt_id": receipt.id }))
        .execute(&mut **transaction)
        .await?;
    }
    sqlx::query(
        "insert into integration_event \
         (receipt_id, from_state, to_state, version, actor, evidence) \
         values ($1, null, 'submitted', 0, $2, $3)",
    )
    .bind(&receipt.id)
    .bind(&receipt.lane_id)
    .bind(serde_json::json!({
        "attempt_id": receipt.attempt_id,
        "provider": receipt.provider.as_str(),
        "source_sha": receipt.source_sha,
    }))
    .execute(&mut **transaction)
    .await?;
    let pending = sqlx::query(
        "update jurisdiction set pending_integration_id = $1 \
         where id = $2 and claimed_by = $3 and lease_generation = $4 \
           and coverage_phase = $5 and pending_integration_id is not distinct from $6",
    )
    .bind(&receipt.id)
    .bind(&receipt.jurisdiction_id)
    .bind(&receipt.lane_id)
    .bind(receipt.lease_generation)
    .bind(receipt.from_phase.as_str())
    .bind(previous_pending)
    .execute(&mut **transaction)
    .await?;
    if pending.rows_affected() != 1 {
        return Err(IntegrationError::LeaseMismatch {
            jurisdiction_id: receipt.jurisdiction_id.clone(),
        });
    }
    Ok(())
}

/// Read one lifecycle projection without acquiring an integration lock.
///
/// # Errors
/// Returns [`IntegrationError::ReceiptNotFound`] or a database/typed-state error.
pub async fn receipt_status(
    pool: &PgPool,
    receipt_id: &str,
) -> Result<StateProjection, IntegrationError> {
    sqlx::query_as::<_, ProjectionRow>(
        "select receipt_id, state, version, candidate_base_sha, integration_branch, \
                pr_number, merge_sha, last_error \
         from integration_receipt_state where receipt_id = $1",
    )
    .bind(receipt_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| IntegrationError::ReceiptNotFound(receipt_id.to_owned()))?
    .into_projection()
}

/// Return the oldest jurisdiction-pending nonterminal receipt for the singleton
/// integrator. Persisted validation commands remain inert strings.
///
/// # Errors
/// Returns a database, JSON-contract, or persisted-enum integrity error.
pub async fn next_actionable_receipt(
    pool: &PgPool,
) -> Result<Option<(IntegrationReceipt, StateProjection)>, IntegrationError> {
    let row = sqlx::query_as::<_, ReceiptProjectionRow>(
        "select receipt.id, receipt.work_key, receipt.jurisdiction_id, receipt.from_phase, \
                receipt.to_phase, receipt.blocked_reason, receipt.source_sha, receipt.base_sha, \
                receipt.source_branch, receipt.lane_id, receipt.lease_generation, \
                receipt.provider, receipt.model, receipt.attempt_id, \
                receipt.validation_evidence, receipt.artifact_hashes, receipt.real_source_proof, \
                receipt.journal_summary, receipt.repair_of, receipt.repair_ordinal, \
                state.state, state.version, state.candidate_base_sha, state.integration_branch, \
                state.pr_number, state.merge_sha, state.last_error \
         from integration_receipt receipt \
         join integration_receipt_state state on state.receipt_id = receipt.id \
         join jurisdiction on jurisdiction.id = receipt.jurisdiction_id \
                              and jurisdiction.pending_integration_id = receipt.id \
         where state.state not in ('applied','deferred','rework_required') \
         order by receipt.submitted_at, receipt.id limit 1",
    )
    .fetch_optional(pool)
    .await?;
    row.map(ReceiptProjectionRow::into_typed).transpose()
}

/// List every jurisdiction-pending nonterminal receipt in deterministic queue order.
///
/// # Errors
/// Returns a database, JSON-contract, or persisted-enum integrity error.
pub async fn pending_receipts(
    pool: &PgPool,
) -> Result<Vec<(IntegrationReceipt, StateProjection)>, IntegrationError> {
    let rows = sqlx::query_as::<_, ReceiptProjectionRow>(
        "select receipt.id, receipt.work_key, receipt.jurisdiction_id, receipt.from_phase, \
                receipt.to_phase, receipt.blocked_reason, receipt.source_sha, receipt.base_sha, \
                receipt.source_branch, receipt.lane_id, receipt.lease_generation, \
                receipt.provider, receipt.model, receipt.attempt_id, \
                receipt.validation_evidence, receipt.artifact_hashes, receipt.real_source_proof, \
                receipt.journal_summary, receipt.repair_of, receipt.repair_ordinal, \
                state.state, state.version, state.candidate_base_sha, state.integration_branch, \
                state.pr_number, state.merge_sha, state.last_error \
         from integration_receipt receipt \
         join integration_receipt_state state on state.receipt_id = receipt.id \
         join jurisdiction on jurisdiction.id = receipt.jurisdiction_id \
                              and jurisdiction.pending_integration_id = receipt.id \
         where state.state not in ('applied','deferred') \
         order by receipt.submitted_at, receipt.id",
    )
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(ReceiptProjectionRow::into_typed)
        .collect()
}

/// Advance the mutable receipt projection by one CAS-guarded lifecycle edge and append
/// the matching immutable event in the same transaction.
///
/// # Errors
/// Returns an illegal-transition, evidence, stale-CAS, missing-receipt, or database error.
pub async fn transition_receipt(
    pool: &PgPool,
    request: &TransitionRequest,
) -> Result<StateProjection, IntegrationError> {
    validate_transition_request(request)?;
    let mut transaction = pool.begin().await?;
    let row = sqlx::query_as::<_, ProjectionRow>(
        "update integration_receipt_state set \
           state = $1, version = version + 1, \
           candidate_base_sha = coalesce($2, candidate_base_sha), \
           integration_branch = coalesce($3, integration_branch), \
           pr_number = coalesce($4, pr_number), \
           merge_sha = coalesce($5, merge_sha), \
           last_error = case when $1 in ('rework_required','deferred') then $6 else null end, \
           updated_at = now() \
         where receipt_id = $7 and state = $8 and version = $9 \
           and ($8 <> 'preparing' or candidate_base_sha is distinct from $2) \
         returning receipt_id, state, version, candidate_base_sha, integration_branch, \
                   pr_number, merge_sha, last_error",
    )
    .bind(request.to_state.as_str())
    .bind(&request.evidence.candidate_base_sha)
    .bind(&request.evidence.integration_branch)
    .bind(request.evidence.pr_number)
    .bind(&request.evidence.merge_sha)
    .bind(&request.evidence.failure)
    .bind(&request.receipt_id)
    .bind(request.expected_state.as_str())
    .bind(request.expected_version)
    .fetch_optional(&mut *transaction)
    .await?;

    let projection = if let Some(row) = row {
        row.into_projection()?
    } else {
        let current = sqlx::query_as::<_, ProjectionRow>(
            "select receipt_id, state, version, candidate_base_sha, integration_branch, \
                    pr_number, merge_sha, last_error \
             from integration_receipt_state where receipt_id = $1",
        )
        .bind(&request.receipt_id)
        .fetch_optional(&mut *transaction)
        .await?;
        let Some(current) = current else {
            return Err(IntegrationError::ReceiptNotFound(
                request.receipt_id.clone(),
            ));
        };
        let current = current.into_projection()?;
        if current.state == request.to_state
            && current.version == request.expected_version.saturating_add(1)
            && transition_evidence_matches(&current, &request.evidence)
        {
            transaction.commit().await?;
            return Ok(current);
        }
        return Err(IntegrationError::CasConflict {
            receipt_id: request.receipt_id.clone(),
            expected_state: request.expected_state,
            expected_version: request.expected_version,
            current_state: Some(current.state),
            current_version: Some(current.version),
        });
    };

    sqlx::query(
        "insert into integration_event \
         (receipt_id, from_state, to_state, version, actor, evidence) \
         values ($1,$2,$3,$4,$5,$6)",
    )
    .bind(&request.receipt_id)
    .bind(request.expected_state.as_str())
    .bind(request.to_state.as_str())
    .bind(projection.version)
    .bind(&request.actor)
    .bind(serde_json::to_value(&request.evidence)?)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(projection)
}

fn validate_transition_request(request: &TransitionRequest) -> Result<(), IntegrationError> {
    if request.actor != INTEGRATOR_ACTOR {
        return Err(IntegrationError::InvalidApplyEvidence(
            "only the singleton integrator may mutate receipt lifecycle".to_owned(),
        ));
    }
    let moving_main_rebuild = request.expected_state == IntegrationState::Preparing
        && request.to_state == IntegrationState::Preparing;
    if request.expected_version < 0
        || (!request.expected_state.allows(request.to_state) && !moving_main_rebuild)
    {
        return Err(IntegrationError::IllegalTransition {
            from: request.expected_state,
            to: request.to_state,
        });
    }
    match request.to_state {
        IntegrationState::Preparing => {
            let candidate = request
                .evidence
                .candidate_base_sha
                .as_deref()
                .ok_or_else(|| {
                    IntegrationError::InvalidApplyEvidence(
                        "preparing transition requires candidate base SHA".to_owned(),
                    )
                })?;
            require_git_sha("candidate base SHA", candidate)?;
        }
        IntegrationState::AwaitingCi => {
            require_nonempty(
                "integration branch",
                request.evidence.integration_branch.as_deref().unwrap_or(""),
            )?;
            if request.evidence.pr_number.is_none_or(|number| number <= 0) {
                return Err(IntegrationError::InvalidApplyEvidence(
                    "awaiting_ci transition requires a positive PR number".to_owned(),
                ));
            }
        }
        IntegrationState::MergedUnapplied => {
            let merge_sha = request.evidence.merge_sha.as_deref().ok_or_else(|| {
                IntegrationError::InvalidApplyEvidence(
                    "merged_unapplied transition requires merge SHA".to_owned(),
                )
            })?;
            require_git_sha("merge SHA", merge_sha)?;
        }
        IntegrationState::ReworkRequired | IntegrationState::Deferred => {
            require_single_line(
                "failure reason",
                request.evidence.failure.as_deref().unwrap_or(""),
            )?;
        }
        IntegrationState::Submitted | IntegrationState::Applied => {
            return Err(IntegrationError::IllegalTransition {
                from: request.expected_state,
                to: request.to_state,
            });
        }
    }
    Ok(())
}

fn transition_evidence_matches(
    projection: &StateProjection,
    evidence: &TransitionEvidence,
) -> bool {
    evidence
        .candidate_base_sha
        .as_ref()
        .is_none_or(|value| projection.candidate_base_sha.as_ref() == Some(value))
        && evidence
            .integration_branch
            .as_ref()
            .is_none_or(|value| projection.integration_branch.as_ref() == Some(value))
        && evidence
            .pr_number
            .is_none_or(|value| projection.pr_number == Some(value))
        && evidence
            .merge_sha
            .as_ref()
            .is_none_or(|value| projection.merge_sha.as_ref() == Some(value))
        && evidence
            .failure
            .as_ref()
            .is_none_or(|value| projection.last_error.as_ref() == Some(value))
}

/// Apply one exact green merged receipt to the registry under the singleton product-
/// domain advisory lock. Jurisdiction mutation, pending release, state CAS, and the
/// immutable event commit together.
///
/// # Errors
/// Returns an authority, evidence, source/generation/pending, lifecycle CAS, integrity,
/// or database error. A repeated call for the already-applied exact merge is idempotent.
pub async fn apply_receipt(
    pool: &PgPool,
    receipt_id: &str,
    expected_version: i64,
    evidence: &ApplyEvidence,
) -> Result<AppliedReceipt, IntegrationError> {
    let mut transaction = pool.begin().await?;
    acquire_apply_authority(&mut transaction).await?;
    let row = load_apply_row(&mut transaction, receipt_id).await?;
    let context = validate_apply_context(&row, receipt_id, evidence)?;
    if let Some(applied) = idempotent_applied(&row, context, receipt_id, evidence)? {
        transaction.commit().await?;
        return Ok(applied);
    }
    validate_unapplied_context(&row, context, receipt_id, expected_version)?;
    let lease_released = update_jurisdiction_for_apply(
        &mut transaction,
        &row,
        context.from_phase,
        context.to_phase,
        receipt_id,
    )
    .await?;
    let new_version = finish_apply_projection(
        &mut transaction,
        &row,
        receipt_id,
        expected_version,
        evidence,
    )
    .await?;
    transaction.commit().await?;
    Ok(AppliedReceipt {
        receipt_id: receipt_id.to_owned(),
        state_version: new_version,
        coverage_phase: context.target_phase,
        lease_released,
        already_applied: false,
    })
}

#[derive(Clone, Copy)]
struct ApplyContext {
    from_phase: CoveragePhase,
    to_phase: Option<CoveragePhase>,
    current_phase: CoveragePhase,
    state: IntegrationState,
    target_phase: CoveragePhase,
}

async fn acquire_apply_authority(
    transaction: &mut Transaction<'_, Postgres>,
) -> Result<(), IntegrationError> {
    let owns_authority: bool = sqlx::query_scalar("select pg_try_advisory_xact_lock($1::bigint)")
        .bind(APPLY_ADVISORY_LOCK_KEY)
        .fetch_one(&mut **transaction)
        .await?;
    if owns_authority {
        Ok(())
    } else {
        Err(IntegrationError::ApplyAuthorityBusy)
    }
}

async fn load_apply_row(
    transaction: &mut Transaction<'_, Postgres>,
    receipt_id: &str,
) -> Result<ApplyRow, IntegrationError> {
    sqlx::query_as::<_, ApplyRow>(
        "select receipt.id as receipt_id, receipt.jurisdiction_id, receipt.from_phase, \
                receipt.to_phase, receipt.blocked_reason, receipt.lane_id, \
                receipt.lease_generation as receipt_generation, receipt.source_sha, \
                receipt.real_source_proof, state.state, state.version as state_version, \
                state.merge_sha as state_merge_sha, jurisdiction.coverage_phase, \
                jurisdiction.claimed_by, jurisdiction.lease_generation, \
                jurisdiction.pending_integration_id \
         from integration_receipt receipt \
         join integration_receipt_state state on state.receipt_id = receipt.id \
         join jurisdiction on jurisdiction.id = receipt.jurisdiction_id \
         where receipt.id = $1 for update of state, jurisdiction",
    )
    .bind(receipt_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| IntegrationError::ReceiptNotFound(receipt_id.to_owned()))
}

fn validate_apply_context(
    row: &ApplyRow,
    receipt_id: &str,
    evidence: &ApplyEvidence,
) -> Result<ApplyContext, IntegrationError> {
    let from_phase: CoveragePhase = row.from_phase.parse()?;
    let to_phase = row.to_phase.as_deref().map(str::parse).transpose()?;
    let current_phase: CoveragePhase = row.coverage_phase.parse()?;
    let state: IntegrationState = row.state.parse()?;
    let merge_sha = row.state_merge_sha.as_deref().ok_or_else(|| {
        IntegrationError::Integrity(format!("receipt {receipt_id} has no persisted merge SHA"))
    })?;
    evidence.validate(&row.source_sha, merge_sha)?;
    let target_phase = to_phase.unwrap_or(from_phase);
    let built_to_live = from_phase == CoveragePhase::Built && to_phase == Some(CoveragePhase::Live);
    if built_to_live && (row.real_source_proof.is_none() || !evidence.real_source_verified) {
        return Err(IntegrationError::InvalidApplyEvidence(
            "built -> live requires verified real fetch/ingestion evidence".to_owned(),
        ));
    }
    Ok(ApplyContext {
        from_phase,
        to_phase,
        current_phase,
        state,
        target_phase,
    })
}

fn idempotent_applied(
    row: &ApplyRow,
    context: ApplyContext,
    receipt_id: &str,
    evidence: &ApplyEvidence,
) -> Result<Option<AppliedReceipt>, IntegrationError> {
    if context.state == IntegrationState::Applied {
        if context.current_phase == context.target_phase
            && row.pending_integration_id.is_none()
            && row.state_merge_sha.as_deref() == Some(evidence.merge_sha.as_str())
        {
            return Ok(Some(AppliedReceipt {
                receipt_id: row.receipt_id.clone(),
                state_version: row.state_version,
                coverage_phase: context.current_phase,
                lease_released: matches!(
                    context.target_phase,
                    CoveragePhase::Live | CoveragePhase::Blocked
                ),
                already_applied: true,
            }));
        }
        return Err(IntegrationError::Integrity(format!(
            "applied receipt {receipt_id} does not match the registry projection"
        )));
    }
    Ok(None)
}

fn validate_unapplied_context(
    row: &ApplyRow,
    context: ApplyContext,
    receipt_id: &str,
    expected_version: i64,
) -> Result<(), IntegrationError> {
    if !context.state.allows_apply() || row.state_version != expected_version {
        return Err(IntegrationError::CasConflict {
            receipt_id: receipt_id.to_owned(),
            expected_state: IntegrationState::MergedUnapplied,
            expected_version,
            current_state: Some(context.state),
            current_version: Some(row.state_version),
        });
    }
    if context.current_phase != context.from_phase
        || row.claimed_by.as_deref() != Some(row.lane_id.as_str())
        || row.lease_generation != row.receipt_generation
        || row.pending_integration_id.as_deref() != Some(receipt_id)
    {
        return Err(IntegrationError::LeaseMismatch {
            jurisdiction_id: row.jurisdiction_id.clone(),
        });
    }
    Ok(())
}

async fn finish_apply_projection(
    transaction: &mut Transaction<'_, Postgres>,
    row: &ApplyRow,
    receipt_id: &str,
    expected_version: i64,
    evidence: &ApplyEvidence,
) -> Result<i64, IntegrationError> {
    let new_version = sqlx::query_scalar::<_, i64>(
        "update integration_receipt_state set state = 'applied', version = version + 1, \
         updated_at = now(), last_error = null \
         where receipt_id = $1 and state = 'merged_unapplied' and version = $2 \
         returning version",
    )
    .bind(receipt_id)
    .bind(expected_version)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| IntegrationError::CasConflict {
        receipt_id: receipt_id.to_owned(),
        expected_state: IntegrationState::MergedUnapplied,
        expected_version,
        current_state: Some(IntegrationState::MergedUnapplied),
        current_version: Some(row.state_version),
    })?;
    sqlx::query(
        "insert into integration_event \
         (receipt_id, from_state, to_state, version, actor, evidence) \
         values ($1, 'merged_unapplied', 'applied', $2, $3, $4)",
    )
    .bind(receipt_id)
    .bind(new_version)
    .bind(INTEGRATOR_ACTOR)
    .bind(serde_json::to_value(evidence)?)
    .execute(&mut **transaction)
    .await?;
    Ok(new_version)
}

async fn update_jurisdiction_for_apply(
    transaction: &mut Transaction<'_, Postgres>,
    row: &ApplyRow,
    from_phase: CoveragePhase,
    to_phase: Option<CoveragePhase>,
    receipt_id: &str,
) -> Result<bool, IntegrationError> {
    let (result, lease_released) = match to_phase {
        None => (
            sqlx::query(
                "update jurisdiction set pending_integration_id = null, claimed_at = now() \
                 where id = $1 and pending_integration_id = $2 and lease_generation = $3 \
                   and claimed_by = $4 and coverage_phase = $5",
            )
            .bind(&row.jurisdiction_id)
            .bind(receipt_id)
            .bind(row.receipt_generation)
            .bind(&row.lane_id)
            .bind(from_phase.as_str())
            .execute(&mut **transaction)
            .await?,
            false,
        ),
        Some(
            CoveragePhase::Scouted
            | CoveragePhase::Surveyed
            | CoveragePhase::Sampled
            | CoveragePhase::Specced
            | CoveragePhase::Built,
        ) => (
            sqlx::query(
                "update jurisdiction set coverage_phase = $6, blocked_reason = null, \
                 pending_integration_id = null, claimed_at = now() \
                 where id = $1 and pending_integration_id = $2 and lease_generation = $3 \
                   and claimed_by = $4 and coverage_phase = $5",
            )
            .bind(&row.jurisdiction_id)
            .bind(receipt_id)
            .bind(row.receipt_generation)
            .bind(&row.lane_id)
            .bind(from_phase.as_str())
            .bind(to_phase.map(CoveragePhase::as_str))
            .execute(&mut **transaction)
            .await?,
            false,
        ),
        Some(CoveragePhase::Live) => (
            sqlx::query(
                "update jurisdiction set coverage_phase = 'live', blocked_reason = null, \
                 pending_integration_id = null, claimed_by = null, claimed_at = null \
                 where id = $1 and pending_integration_id = $2 and lease_generation = $3 \
                   and claimed_by = $4 and coverage_phase = $5",
            )
            .bind(&row.jurisdiction_id)
            .bind(receipt_id)
            .bind(row.receipt_generation)
            .bind(&row.lane_id)
            .bind(from_phase.as_str())
            .execute(&mut **transaction)
            .await?,
            true,
        ),
        Some(CoveragePhase::Blocked) => (
            sqlx::query(
                "update jurisdiction set coverage_phase = 'blocked', blocked_reason = $6, \
                 pending_integration_id = null, claimed_by = null, claimed_at = null \
                 where id = $1 and pending_integration_id = $2 and lease_generation = $3 \
                   and claimed_by = $4 and coverage_phase = $5",
            )
            .bind(&row.jurisdiction_id)
            .bind(receipt_id)
            .bind(row.receipt_generation)
            .bind(&row.lane_id)
            .bind(from_phase.as_str())
            .bind(&row.blocked_reason)
            .execute(&mut **transaction)
            .await?,
            true,
        ),
        Some(CoveragePhase::Stub) => {
            return Err(IntegrationError::Integrity(
                "receipt attempted to apply the seed phase".to_owned(),
            ));
        }
    };
    if result.rows_affected() != 1 {
        return Err(IntegrationError::LeaseMismatch {
            jurisdiction_id: row.jurisdiction_id.clone(),
        });
    }
    Ok(lease_released)
}

fn require_nonempty(label: &str, value: &str) -> Result<(), IntegrationError> {
    if value.trim().is_empty() {
        Err(IntegrationError::InvalidReceipt(format!(
            "{label} cannot be empty"
        )))
    } else {
        Ok(())
    }
}

fn require_single_line(label: &str, value: &str) -> Result<(), IntegrationError> {
    require_nonempty(label, value)?;
    if value.contains('\r') || value.contains('\n') {
        Err(IntegrationError::InvalidReceipt(format!(
            "{label} must be one line"
        )))
    } else {
        Ok(())
    }
}

fn require_git_sha(label: &str, value: &str) -> Result<(), IntegrationError> {
    if matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(IntegrationError::InvalidReceipt(format!(
            "{label} must be a lowercase 40- or 64-character Git object id"
        )))
    }
}

fn require_sha256(label: &str, value: &str) -> Result<(), IntegrationError> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(IntegrationError::InvalidReceipt(format!(
            "{label} must be a lowercase SHA-256"
        )))
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod integration_receipt_tests {
    use chrono::{TimeZone as _, Utc};

    use super::*;

    fn sha(character: char) -> String {
        std::iter::repeat_n(character, 40).collect()
    }

    fn proof() -> RealSourceProof {
        RealSourceProof {
            fetched_at: Utc
                .with_ymd_and_hms(2026, 7, 11, 12, 0, 0)
                .single()
                .expect("fixture timestamp should be valid"),
            source_url: "https://example.test/disclosures".to_owned(),
            bronze_sha256: std::iter::repeat_n('b', 64).collect(),
            ingestion_run_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
            rows_ingested: 1,
        }
    }

    fn receipt(from: CoveragePhase, to: Option<CoveragePhase>) -> IntegrationReceipt {
        IntegrationReceipt {
            id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
            work_key: "jurisdiction:zz:phase".to_owned(),
            jurisdiction_id: "zz".to_owned(),
            from_phase: from,
            to_phase: to,
            blocked_reason: None,
            source_sha: sha('a'),
            base_sha: sha('b'),
            branch: "goal/fixture".to_owned(),
            lane_id: "lane-0".to_owned(),
            lease_generation: 7,
            provider: ProducerProvider::Claude,
            model: "fixture-model".to_owned(),
            attempt_id: "attempt-1".to_owned(),
            validation_evidence: vec![ValidationEvidence {
                name: "rust".to_owned(),
                command: "cargo test --workspace".to_owned(),
                exit_code: 0,
                output_sha256: std::iter::repeat_n('c', 64).collect(),
            }],
            artifact_hashes: vec![ArtifactHash {
                path: "artifact.json".to_owned(),
                sha256: std::iter::repeat_n('d', 64).collect(),
            }],
            real_source_proof: None,
            journal_summary: "fixture receipt applied".to_owned(),
            repair_of: None,
            repair_ordinal: None,
        }
    }

    #[test]
    fn coverage_phase_should_allow_only_adjacent_or_blocked_transitions() {
        let phases = CoveragePhase::ALL;
        for from in phases {
            for to in phases {
                let adjacent = matches!(
                    (from, to),
                    (CoveragePhase::Stub, CoveragePhase::Scouted)
                        | (CoveragePhase::Scouted, CoveragePhase::Surveyed)
                        | (CoveragePhase::Surveyed, CoveragePhase::Sampled)
                        | (CoveragePhase::Sampled, CoveragePhase::Specced)
                        | (CoveragePhase::Specced, CoveragePhase::Built)
                        | (CoveragePhase::Built, CoveragePhase::Live)
                );
                let block = !matches!(from, CoveragePhase::Live | CoveragePhase::Blocked)
                    && to == CoveragePhase::Blocked;
                assert_eq!(from.allows(to), adjacent || block, "{from:?} -> {to:?}");
            }
        }
    }

    #[test]
    fn built_to_live_should_require_real_source_proof() {
        let mut candidate = receipt(CoveragePhase::Built, Some(CoveragePhase::Live));
        assert!(matches!(
            candidate.validate(),
            Err(IntegrationError::InvalidReceipt(_))
        ));

        candidate.real_source_proof = Some(proof());

        assert!(candidate.validate().is_ok());
    }

    #[test]
    fn blocked_transition_should_require_a_single_reason() {
        let mut candidate = receipt(CoveragePhase::Surveyed, Some(CoveragePhase::Blocked));
        assert!(candidate.validate().is_err());
        candidate.blocked_reason = Some("source permanently unavailable".to_owned());
        assert!(candidate.validate().is_ok());
        candidate.blocked_reason = Some("bad\nreason".to_owned());
        assert!(candidate.validate().is_err());
    }

    #[test]
    fn lifecycle_should_reject_skips_and_generic_applied_transition() {
        assert!(IntegrationState::Submitted.allows(IntegrationState::Preparing));
        assert!(!IntegrationState::Submitted.allows(IntegrationState::AwaitingCi));
        assert!(IntegrationState::Preparing.allows(IntegrationState::ReworkRequired));
        assert!(IntegrationState::AwaitingCi.allows(IntegrationState::MergedUnapplied));
        assert!(!IntegrationState::MergedUnapplied.allows(IntegrationState::Applied));
        assert!(IntegrationState::MergedUnapplied.allows_apply());
    }

    #[test]
    fn preparing_self_edge_is_reserved_for_candidate_rebuild_requests() {
        assert!(!IntegrationState::Preparing.allows(IntegrationState::Preparing));
        let request = TransitionRequest {
            receipt_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
            expected_state: IntegrationState::Preparing,
            expected_version: 2,
            to_state: IntegrationState::Preparing,
            actor: INTEGRATOR_ACTOR.to_owned(),
            evidence: TransitionEvidence {
                candidate_base_sha: Some(sha('f')),
                ..TransitionEvidence::default()
            },
        };
        assert!(validate_transition_request(&request).is_ok());
    }

    #[test]
    fn apply_evidence_should_require_exact_green_merge() {
        let merge = sha('e');
        let mut evidence = ApplyEvidence::successful(&sha('a'), &merge);
        assert!(evidence.validate(&sha('a'), &merge).is_ok());
        evidence.required_checks.web.commit_sha = sha('f');
        assert!(evidence.validate(&sha('a'), &merge).is_err());
    }

    #[test]
    fn receipt_payload_hash_should_ignore_only_receipt_id() {
        let first = receipt(CoveragePhase::Stub, Some(CoveragePhase::Scouted));
        let mut same_payload = first.clone();
        same_payload.id = "01BX5ZZKBKACTAV9WEVGEMMVA1".to_owned();
        assert_eq!(
            first.payload_sha256().unwrap(),
            same_payload.payload_sha256().unwrap()
        );
        same_payload.branch = "goal/different".to_owned();
        assert_ne!(
            first.payload_sha256().unwrap(),
            same_payload.payload_sha256().unwrap()
        );
    }
}
