//! Exact provider compatibility canaries with mechanical skill-load evidence.

use std::fs::{self, File};
use std::io::{self, BufRead as _, BufReader};
use std::path::{Component, Path, PathBuf};
use std::time::Duration as StdDuration;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::time::sleep;
use ulid::Ulid;

use crate::artifacts::{ArtifactPolicy, ArtifactStore, AttemptArtifactPolicy};
use crate::model::{AttemptSpec, CommandSpec, NormalizedResult, PromptKind, Provider, ResultClass};
use crate::process::{ProcessError, ProcessOutputPaths, ProcessRunner, cancellation_pair};
use crate::provider::{EventClassifier, ProviderAdapter, ProviderBuildError};
use crate::store::{CompatibilityRecord, ControlStore, StoreError};

pub const COMPATIBILITY_KIND: &str = "structured_exact_resume_skill_v1";
pub const COMPATIBILITY_PROOF_SCHEMA: &str = "govfolio.provider-compatibility/v1";
pub const SKILL_MARKER_SCHEMA: &str = "govfolio.skill-load/v1";
const MAX_SKILL_BYTES: u64 = 1024 * 1024;
const MAX_MARKER_BYTES: u64 = 16 * 1024;
const MAX_EVENT_SCAN_BYTES: u64 = 1024 * 1024;
const MAX_CODEX_ROLLOUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_CODEX_SESSION_ENTRIES: usize = 10_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanaryStage {
    Fresh,
    ExactResume,
}

impl CanaryStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::ExactResume => "exact_resume",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillCanarySpec {
    pub identifier: String,
    pub repository_relative_path: PathBuf,
    pub approved_sha256: String,
    pub challenge: String,
    pub marker_relative_path: PathBuf,
}

impl SkillCanarySpec {
    /// Validates and constructs one repository-approved skill challenge.
    ///
    /// # Errors
    ///
    /// Returns an error unless the skill lives below `agents/skills`, the
    /// marker lives below `.govfolio-loop`, and all proof fields are bounded.
    pub fn new(
        identifier: impl Into<String>,
        repository_relative_path: PathBuf,
        approved_sha256: impl Into<String>,
        challenge: impl Into<String>,
        marker_relative_path: PathBuf,
    ) -> Result<Self, CanaryError> {
        let spec = Self {
            identifier: identifier.into(),
            repository_relative_path,
            approved_sha256: approved_sha256.into(),
            challenge: challenge.into(),
            marker_relative_path,
        };
        spec.validate_shape()?;
        Ok(spec)
    }

    fn validate_shape(&self) -> Result<(), CanaryError> {
        if self.identifier.trim().is_empty() || self.identifier.chars().count() > 128 {
            return Err(CanaryError::InvalidSkillSpec(
                "skill identifier is empty or overlong".to_owned(),
            ));
        }
        if !safe_relative(&self.repository_relative_path)
            || !self
                .repository_relative_path
                .starts_with(Path::new("agents/skills"))
            || self
                .repository_relative_path
                .file_name()
                .and_then(|name| name.to_str())
                != Some("SKILL.md")
        {
            return Err(CanaryError::InvalidSkillSpec(
                "skill path must be a safe agents/skills/**/SKILL.md path".to_owned(),
            ));
        }
        if !valid_sha256(&self.approved_sha256) {
            return Err(CanaryError::InvalidSkillSpec(
                "approved skill hash must be lowercase SHA-256".to_owned(),
            ));
        }
        if self.challenge.trim().is_empty() || self.challenge.chars().count() > 128 {
            return Err(CanaryError::InvalidSkillSpec(
                "skill challenge is empty or overlong".to_owned(),
            ));
        }
        if !safe_relative(&self.marker_relative_path)
            || !self
                .marker_relative_path
                .starts_with(Path::new(".govfolio-loop"))
        {
            return Err(CanaryError::InvalidSkillSpec(
                "skill marker must be a safe .govfolio-loop relative path".to_owned(),
            ));
        }
        Ok(())
    }

    fn skill_path(&self, worktree: &Path) -> PathBuf {
        worktree.join(&self.repository_relative_path)
    }

    fn marker_path(&self, worktree: &Path) -> PathBuf {
        worktree.join(&self.marker_relative_path)
    }

    fn fresh_prompt(&self) -> String {
        format!(
            "Compatibility canary. Mechanically load the repository-approved skill `{}` from `{}`. Compute the SHA-256 of the exact skill file (the expected hash is intentionally not provided). Create `{}` as JSON with exactly schema=`{}`, skill_identifier=`{}`, skill_sha256=<computed lowercase SHA-256>, and challenge=`{}`. Do not merely claim that the skill was loaded. Finish this bounded turn after the marker exists.",
            self.identifier,
            normalized_path(&self.repository_relative_path),
            normalized_path(&self.marker_relative_path),
            SKILL_MARKER_SCHEMA,
            self.identifier,
            self.challenge,
        )
    }

    fn resume_prompt(&self) -> String {
        format!(
            "Exact-session compatibility resume. Do not modify files. Complete one bounded turn for challenge `{}`.",
            self.challenge
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillLoadEventEvidence {
    pub event_type: String,
    pub event_sha256: String,
    pub referenced_path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct CanaryInvocation {
    pub result: NormalizedResult,
    pub reported_model: Option<String>,
    pub evidence_ref: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub skill_load_event: Option<SkillLoadEventEvidence>,
}

#[derive(Debug, Error)]
pub enum CanaryInvocationError {
    #[error(transparent)]
    Process(#[from] ProcessError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[async_trait]
pub trait CanaryInvoker: Send + Sync {
    async fn invoke(
        &self,
        stage: CanaryStage,
        provider: Provider,
        command: &CommandSpec,
        classifier: Box<dyn EventClassifier>,
        skill: &SkillCanarySpec,
    ) -> Result<CanaryInvocation, CanaryInvocationError>;
}

#[derive(Clone, Debug)]
pub struct ProcessCanaryInvoker {
    process: ProcessRunner,
    artifacts: ArtifactStore,
    timeout: StdDuration,
}

impl ProcessCanaryInvoker {
    #[must_use]
    pub fn new(runtime_root: impl AsRef<Path>, timeout: StdDuration) -> Self {
        Self {
            process: ProcessRunner::default(),
            artifacts: ArtifactStore::new(runtime_root, ArtifactPolicy::default()),
            timeout: timeout.max(StdDuration::from_secs(1)),
        }
    }
}

#[async_trait]
impl CanaryInvoker for ProcessCanaryInvoker {
    async fn invoke(
        &self,
        stage: CanaryStage,
        provider: Provider,
        command: &CommandSpec,
        classifier: Box<dyn EventClassifier>,
        skill: &SkillCanarySpec,
    ) -> Result<CanaryInvocation, CanaryInvocationError> {
        let attempt_id = format!("compatibility-{}-{}", stage.as_str(), Ulid::new());
        let attempt = self
            .artifacts
            .begin_attempt(&attempt_id, AttemptArtifactPolicy::Persist)?
            .ok_or_else(|| io::Error::other("canary attempt was unexpectedly suppressed"))?;
        let output_paths = ProcessOutputPaths::from(&attempt);
        let (cancel, cancellation) = cancellation_pair();
        let execution = self
            .run_bounded(command, &output_paths, classifier, cancel, cancellation)
            .await?;
        self.artifacts
            .write_json(&attempt.result_path(), &execution.result)?;
        let (mut reported_model, skill_load_event) =
            inspect_structured_events(&attempt.events_path(), provider, skill)?;
        if reported_model.is_none()
            && provider == Provider::Codex
            && let Some(session_id) = execution.result.session_id.as_deref()
            && let Some(proof) = codex_rollout_model_proof(command, session_id)?
        {
            self.artifacts
                .write_json(&attempt.directory().join("model-proof.json"), &proof)?;
            reported_model = Some(proof.model);
        }
        Ok(CanaryInvocation {
            result: execution.result,
            reported_model,
            evidence_ref: attempt.directory().display().to_string(),
            stdout_bytes: execution.stdout_bytes,
            stderr_bytes: execution.stderr_bytes,
            skill_load_event,
        })
    }
}

#[derive(Debug, Serialize)]
struct CodexRolloutModelProof {
    schema: &'static str,
    session_id: String,
    model: String,
    model_provider: String,
    model_event_count: usize,
    model_events_sha256: String,
    rollout_file: String,
}

fn codex_rollout_model_proof(
    command: &CommandSpec,
    session_id: &str,
) -> io::Result<Option<CodexRolloutModelProof>> {
    if session_id.is_empty()
        || session_id.len() > 128
        || !session_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Codex returned an unsafe session identifier",
        ));
    }
    let Some(codex_home) = command_environment_path(command, "CODEX_HOME").or_else(|| {
        command_environment_path(command, "USERPROFILE")
            .or_else(|| command_environment_path(command, "HOME"))
            .map(|home| home.join(".codex"))
    }) else {
        return Ok(None);
    };
    let sessions = codex_home.join("sessions");
    if !sessions.is_dir() {
        return Ok(None);
    }
    let mut matches = Vec::new();
    let mut entries = 0_usize;
    collect_codex_rollouts(&sessions, session_id, 0, &mut entries, &mut matches)?;
    if matches.is_empty() {
        return Ok(None);
    }
    if matches.len() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Codex session identifier matched multiple rollout files",
        ));
    }
    parse_codex_rollout_model(&matches[0], session_id).map(Some)
}

fn command_environment_path(command: &CommandSpec, key: &str) -> Option<PathBuf> {
    command
        .env
        .iter()
        .find(|(candidate, value)| candidate.eq_ignore_ascii_case(key) && !value.is_empty())
        .map(|(_, value)| PathBuf::from(value))
}

fn collect_codex_rollouts(
    directory: &Path,
    session_id: &str,
    depth: usize,
    entries: &mut usize,
    matches: &mut Vec<PathBuf>,
) -> io::Result<()> {
    if depth > 4 {
        return Ok(());
    }
    let expected_suffix = format!("-{session_id}.jsonl");
    for entry in fs::read_dir(directory)? {
        *entries = entries.saturating_add(1);
        if *entries > MAX_CODEX_SESSION_ENTRIES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Codex session tree exceeded the bounded entry scan",
            ));
        }
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_codex_rollouts(&entry.path(), session_id, depth + 1, entries, matches)?;
        } else if file_type.is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .ends_with(&expected_suffix)
        {
            matches.push(entry.path());
        }
    }
    Ok(())
}

fn parse_codex_rollout_model(
    path: &Path,
    expected_session_id: &str,
) -> io::Result<CodexRolloutModelProof> {
    let mut scanned = 0_u64;
    let mut session_matches = false;
    let mut model_provider = None;
    let mut model = None;
    let mut model_event_count = 0_usize;
    let mut model_events_digest = Sha256::new();
    for line in BufReader::new(File::open(path)?).split(b'\n') {
        let line = line?;
        scanned = scanned.saturating_add(u64::try_from(line.len()).unwrap_or(u64::MAX));
        if scanned > MAX_CODEX_ROLLOUT_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Codex rollout exceeded the bounded model-proof scan",
            ));
        }
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_slice(&line).map_err(io::Error::other)?;
        match value.get("type").and_then(Value::as_str) {
            Some("session_meta") => {
                session_matches = value
                    .pointer("/payload/session_id")
                    .or_else(|| value.pointer("/payload/id"))
                    .and_then(Value::as_str)
                    == Some(expected_session_id);
                model_provider = value
                    .pointer("/payload/model_provider")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
            }
            Some("turn_context") => {
                let observed = value
                    .pointer("/payload/model")
                    .and_then(Value::as_str)
                    .filter(|value| !value.is_empty())
                    .ok_or_else(|| {
                        io::Error::new(
                            io::ErrorKind::InvalidData,
                            "Codex turn context omitted its model",
                        )
                    })?;
                if model.as_deref().is_some_and(|current| current != observed) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Codex rollout changed models within one exact session",
                    ));
                }
                model = Some(observed.to_owned());
                model_event_count = model_event_count.saturating_add(1);
                model_events_digest.update(&line);
                model_events_digest.update([0]);
            }
            _ => {}
        }
    }
    let model = model.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Codex rollout did not contain a model-bearing turn context",
        )
    })?;
    if !session_matches {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Codex rollout session metadata did not match the completed thread",
        ));
    }
    let model_provider = model_provider
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Codex rollout omitted its model provider",
            )
        })?;
    Ok(CodexRolloutModelProof {
        schema: "govfolio.codex-rollout-model/v1",
        session_id: expected_session_id.to_owned(),
        model,
        model_provider,
        model_event_count,
        model_events_sha256: hex::encode(model_events_digest.finalize()),
        rollout_file: path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
    })
}

impl ProcessCanaryInvoker {
    async fn run_bounded(
        &self,
        command: &CommandSpec,
        output_paths: &ProcessOutputPaths,
        classifier: Box<dyn EventClassifier>,
        cancel: crate::process::ProcessCancelHandle,
        cancellation: crate::process::ProcessCancellation,
    ) -> Result<crate::process::ProcessExecution, ProcessError> {
        let execution = self
            .process
            .run(command, output_paths, classifier, cancellation);
        tokio::pin!(execution);
        tokio::select! {
            result = &mut execution => result,
            () = sleep(self.timeout) => {
                cancel.cancel();
                execution.await
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct CanaryRequest {
    pub attempt: AttemptSpec,
    pub provider_key: String,
    pub inherited_env: Vec<(String, String)>,
    pub valid_for: Duration,
    pub skill: SkillCanarySpec,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CanaryObservation {
    pub stage: CanaryStage,
    pub result: NormalizedResult,
    pub reported_model: Option<String>,
    pub evidence_ref: String,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SkillProof {
    pub identifier: String,
    pub repository_relative_path: PathBuf,
    pub approved_sha256: String,
    pub challenge: String,
    pub marker_relative_path: PathBuf,
    pub marker_sha256: Option<String>,
    pub load_event: Option<SkillLoadEventEvidence>,
    pub verified: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CompatibilityProof {
    pub schema: String,
    pub provider_key: String,
    pub provider: Provider,
    pub cli_version: String,
    pub model: String,
    pub executable: PathBuf,
    pub config_fingerprint: String,
    pub session_id: Option<String>,
    pub fresh: CanaryObservation,
    pub exact_resume: Option<CanaryObservation>,
    pub skill: SkillProof,
    pub rejection: Option<String>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanaryStatus {
    NeedsProbe,
    Proven { proof_ref: Option<String> },
    Rejected { proof_ref: Option<String> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanaryOutcome {
    Proven { proof_ref: String, cached: bool },
    Rejected { proof_ref: String, reason: String },
}

#[derive(Debug, Error)]
pub enum CanaryError {
    #[error("compatibility canary requires an explicit model")]
    MissingModel,
    #[error("compatibility provider key is empty")]
    EmptyProviderKey,
    #[error("compatibility validity must be positive")]
    InvalidValidity,
    #[error("compatibility timestamp overflow")]
    TimestampOverflow,
    #[error("compatibility attempt must use compatibility_canary prompt kind")]
    WrongPromptKind,
    #[error("invalid skill canary: {0}")]
    InvalidSkillSpec(String),
    #[error("approved skill file is too large")]
    SkillTooLarge,
    #[error("approved skill hash does not match the checked-out file")]
    SkillHashMismatch,
    #[error("skill proof marker already exists before the canary")]
    MarkerAlreadyExists,
    #[error(transparent)]
    ProviderBuild(#[from] ProviderBuildError),
    #[error(transparent)]
    Invocation(#[from] CanaryInvocationError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub struct CompatibilityCanary<'a> {
    store: &'a ControlStore,
    artifacts: &'a ArtifactStore,
}

impl<'a> CompatibilityCanary<'a> {
    #[must_use]
    pub const fn new(store: &'a ControlStore, artifacts: &'a ArtifactStore) -> Self {
        Self { store, artifacts }
    }

    /// Reads exact-fingerprint compatibility without launching a provider.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing model or control-store failure.
    pub async fn status(
        &self,
        request: &CanaryRequest,
        now: DateTime<Utc>,
    ) -> Result<CanaryStatus, CanaryError> {
        validate_request(request)?;
        let model = request
            .attempt
            .provider
            .model
            .as_deref()
            .ok_or(CanaryError::MissingModel)?;
        let record = self
            .store
            .compatibility(
                &request.provider_key,
                &request.attempt.provider.cli_version,
                model,
                &request.attempt.provider.config_fingerprint,
                COMPATIBILITY_KIND,
                now,
            )
            .await?;
        Ok(match record {
            None => CanaryStatus::NeedsProbe,
            Some(record) if record.proven => CanaryStatus::Proven {
                proof_ref: record.proof_ref,
            },
            Some(record) => CanaryStatus::Rejected {
                proof_ref: record.proof_ref,
            },
        })
    }

    /// Runs a fresh structured turn followed by one exact-session resume.
    ///
    /// # Errors
    ///
    /// Returns an error for local validation, invocation infrastructure,
    /// evidence storage, or control-store failures. Provider protocol failures
    /// are persisted and returned as [`CanaryOutcome::Rejected`].
    pub async fn prove(
        &self,
        adapter: &dyn ProviderAdapter,
        invoker: &dyn CanaryInvoker,
        request: &CanaryRequest,
        now: DateTime<Utc>,
    ) -> Result<CanaryOutcome, CanaryError> {
        validate_request(request)?;
        if let CanaryStatus::Proven { proof_ref } = self.status(request, now).await? {
            return Ok(CanaryOutcome::Proven {
                proof_ref: proof_ref.unwrap_or_else(|| "compatibility:proven".to_owned()),
                cached: true,
            });
        }

        let prepared_skill = prepare_skill(&request.attempt.worktree, &request.skill)?;
        let mut fresh_attempt = request.attempt.clone();
        fresh_attempt.prompt = request.skill.fresh_prompt();
        fresh_attempt.resume_session_id = None;
        let fresh_command = adapter.build_fresh(&fresh_attempt, &request.inherited_env)?;
        let fresh_invocation = invoker
            .invoke(
                CanaryStage::Fresh,
                adapter.provider(),
                &fresh_command,
                adapter.classifier(),
                &request.skill,
            )
            .await?;
        let fresh = observation(CanaryStage::Fresh, &fresh_invocation);
        let marker = verify_skill_marker(&prepared_skill, &request.skill).ok();
        let fresh_rejection =
            validate_fresh(&fresh_invocation, expected_model(request)?, marker.as_ref());
        if let Some(reason) = fresh_rejection {
            let proof = proof(
                request,
                fresh,
                None,
                None,
                skill_proof(request, marker, fresh_invocation.skill_load_event),
                Some(reason.clone()),
                now,
            );
            let proof_ref = self.persist_proof(request, &proof, now, false).await?;
            return Ok(CanaryOutcome::Rejected { proof_ref, reason });
        }

        let session_id =
            fresh_invocation.result.session_id.clone().ok_or_else(|| {
                CanaryError::InvalidSkillSpec("fresh session disappeared".to_owned())
            })?;
        let mut resume_attempt = request.attempt.clone();
        resume_attempt.prompt = request.skill.resume_prompt();
        resume_attempt.resume_session_id = Some(session_id.clone());
        let resume_command =
            adapter.build_resume(&resume_attempt, &session_id, &request.inherited_env)?;
        let resume_invocation = invoker
            .invoke(
                CanaryStage::ExactResume,
                adapter.provider(),
                &resume_command,
                adapter.classifier(),
                &request.skill,
            )
            .await?;
        let resume = observation(CanaryStage::ExactResume, &resume_invocation);
        let rejection = validate_resume(&resume_invocation, expected_model(request)?, &session_id);
        let proof = proof(
            request,
            fresh,
            Some(resume),
            Some(session_id.clone()),
            skill_proof(request, marker, fresh_invocation.skill_load_event),
            rejection.clone(),
            now,
        );
        let proven = rejection.is_none();
        let proof_ref = self.persist_proof(request, &proof, now, proven).await?;
        match rejection {
            Some(reason) => Ok(CanaryOutcome::Rejected { proof_ref, reason }),
            None => Ok(CanaryOutcome::Proven {
                proof_ref,
                cached: false,
            }),
        }
    }

    async fn persist_proof(
        &self,
        request: &CanaryRequest,
        proof: &CompatibilityProof,
        now: DateTime<Utc>,
        proven: bool,
    ) -> Result<String, CanaryError> {
        let valid_until = now
            .checked_add_signed(request.valid_for)
            .ok_or(CanaryError::TimestampOverflow)?;
        let bytes = serde_json::to_vec(proof)?;
        let blob = self.artifacts.write_gzip_blob(&bytes)?;
        let proof_ref = format!("sha256:{}", blob.sha256);
        self.store
            .upsert_compatibility(&CompatibilityRecord {
                provider_key: request.provider_key.clone(),
                cli_version: request.attempt.provider.cli_version.clone(),
                model: expected_model(request)?.to_owned(),
                config_fingerprint: request.attempt.provider.config_fingerprint.clone(),
                compatibility_kind: COMPATIBILITY_KIND.to_owned(),
                proven,
                proof_ref: Some(proof_ref.clone()),
                checked_at: now,
                valid_until: Some(valid_until),
            })
            .await?;
        Ok(proof_ref)
    }
}

struct PreparedSkill {
    marker_path: PathBuf,
    approved_sha256: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SkillMarker {
    schema: String,
    skill_identifier: String,
    skill_sha256: String,
    challenge: String,
}

fn prepare_skill(worktree: &Path, skill: &SkillCanarySpec) -> Result<PreparedSkill, CanaryError> {
    skill.validate_shape()?;
    let skill_path = skill.skill_path(worktree);
    let metadata = std::fs::metadata(&skill_path)?;
    if metadata.len() > MAX_SKILL_BYTES {
        return Err(CanaryError::SkillTooLarge);
    }
    let actual_sha256 = hash_bytes(&std::fs::read(skill_path)?);
    if actual_sha256 != skill.approved_sha256 {
        return Err(CanaryError::SkillHashMismatch);
    }
    let marker_path = skill.marker_path(worktree);
    if marker_path.exists() {
        return Err(CanaryError::MarkerAlreadyExists);
    }
    Ok(PreparedSkill {
        marker_path,
        approved_sha256: actual_sha256,
    })
}

fn verify_skill_marker(
    prepared: &PreparedSkill,
    skill: &SkillCanarySpec,
) -> Result<String, CanaryError> {
    let metadata = std::fs::metadata(&prepared.marker_path)?;
    if metadata.len() > MAX_MARKER_BYTES {
        return Err(CanaryError::InvalidSkillSpec(
            "skill marker is overlong".to_owned(),
        ));
    }
    let bytes = std::fs::read(&prepared.marker_path)?;
    let marker: SkillMarker = serde_json::from_slice(&bytes)?;
    let expected = SkillMarker {
        schema: SKILL_MARKER_SCHEMA.to_owned(),
        skill_identifier: skill.identifier.clone(),
        skill_sha256: prepared.approved_sha256.clone(),
        challenge: skill.challenge.clone(),
    };
    if marker.schema != expected.schema
        || marker.skill_identifier != expected.skill_identifier
        || marker.skill_sha256 != expected.skill_sha256
        || marker.challenge != expected.challenge
    {
        return Err(CanaryError::InvalidSkillSpec(
            "skill marker does not match the mechanical challenge".to_owned(),
        ));
    }
    Ok(hash_bytes(&bytes))
}

fn validate_fresh(
    invocation: &CanaryInvocation,
    expected_model: &str,
    marker_sha256: Option<&String>,
) -> Option<String> {
    validate_common(invocation, expected_model)
        .or_else(|| {
            invocation
                .result
                .session_id
                .as_deref()
                .is_none_or(|session| session.trim().is_empty())
                .then(|| "fresh canary did not capture a session/thread ID".to_owned())
        })
        .or_else(|| {
            invocation
                .skill_load_event
                .is_none()
                .then(|| "fresh canary has no structured skill-load event".to_owned())
        })
        .or_else(|| {
            marker_sha256
                .is_none()
                .then(|| "fresh canary did not create the verified skill marker".to_owned())
        })
}

fn validate_resume(
    invocation: &CanaryInvocation,
    expected_model: &str,
    exact_session_id: &str,
) -> Option<String> {
    validate_common(invocation, expected_model).or_else(|| {
        (invocation.result.session_id.as_deref() != Some(exact_session_id))
            .then(|| "resume canary returned a different session/thread ID".to_owned())
    })
}

fn validate_common(invocation: &CanaryInvocation, expected_model: &str) -> Option<String> {
    if invocation.result.class != ResultClass::Completed {
        Some(format!(
            "canary terminal class is {}",
            invocation.result.class
        ))
    } else if !invocation.result.structured_started || invocation.result.terminal_type.is_none() {
        Some("canary lacks a structured terminal event".to_owned())
    } else if invocation.result.exit_code.is_none() {
        Some("canary exit behavior was not captured".to_owned())
    } else if invocation.stdout_bytes == 0 {
        Some("canary structured stdout is empty".to_owned())
    } else if invocation.reported_model.as_deref() != Some(expected_model) {
        Some("canary reported model differs from configured model".to_owned())
    } else if invocation.evidence_ref.trim().is_empty() {
        Some("canary evidence reference is empty".to_owned())
    } else {
        None
    }
}

fn observation(stage: CanaryStage, invocation: &CanaryInvocation) -> CanaryObservation {
    CanaryObservation {
        stage,
        result: invocation.result.clone(),
        reported_model: invocation.reported_model.clone(),
        evidence_ref: invocation.evidence_ref.clone(),
        stdout_bytes: invocation.stdout_bytes,
        stderr_bytes: invocation.stderr_bytes,
    }
}

fn skill_proof(
    request: &CanaryRequest,
    marker_sha256: Option<String>,
    load_event: Option<SkillLoadEventEvidence>,
) -> SkillProof {
    SkillProof {
        identifier: request.skill.identifier.clone(),
        repository_relative_path: request.skill.repository_relative_path.clone(),
        approved_sha256: request.skill.approved_sha256.clone(),
        challenge: request.skill.challenge.clone(),
        marker_relative_path: request.skill.marker_relative_path.clone(),
        verified: marker_sha256.is_some() && load_event.is_some(),
        marker_sha256,
        load_event,
    }
}

fn proof(
    request: &CanaryRequest,
    fresh: CanaryObservation,
    exact_resume: Option<CanaryObservation>,
    session_id: Option<String>,
    skill: SkillProof,
    rejection: Option<String>,
    checked_at: DateTime<Utc>,
) -> CompatibilityProof {
    CompatibilityProof {
        schema: COMPATIBILITY_PROOF_SCHEMA.to_owned(),
        provider_key: request.provider_key.clone(),
        provider: request.attempt.provider.provider,
        cli_version: request.attempt.provider.cli_version.clone(),
        model: request.attempt.provider.model.clone().unwrap_or_default(),
        executable: request.attempt.provider.executable.clone(),
        config_fingerprint: request.attempt.provider.config_fingerprint.clone(),
        session_id,
        fresh,
        exact_resume,
        skill,
        rejection,
        checked_at,
    }
}

fn validate_request(request: &CanaryRequest) -> Result<(), CanaryError> {
    if request.provider_key.trim().is_empty() {
        return Err(CanaryError::EmptyProviderKey);
    }
    if request.valid_for <= Duration::zero() {
        return Err(CanaryError::InvalidValidity);
    }
    if request.attempt.prompt_kind != PromptKind::CompatibilityCanary {
        return Err(CanaryError::WrongPromptKind);
    }
    if request
        .attempt
        .provider
        .model
        .as_deref()
        .is_none_or(str::is_empty)
    {
        return Err(CanaryError::MissingModel);
    }
    request.skill.validate_shape()
}

fn expected_model(request: &CanaryRequest) -> Result<&str, CanaryError> {
    request
        .attempt
        .provider
        .model
        .as_deref()
        .filter(|model| !model.is_empty())
        .ok_or(CanaryError::MissingModel)
}

fn inspect_structured_events(
    path: &Path,
    provider: Provider,
    skill: &SkillCanarySpec,
) -> io::Result<(Option<String>, Option<SkillLoadEventEvidence>)> {
    let mut model = None;
    let mut skill_event = None;
    let mut scanned = 0_u64;
    let reader = BufReader::new(File::open(path)?);
    for line in reader.split(b'\n') {
        let line = line?;
        scanned = scanned.saturating_add(u64::try_from(line.len()).unwrap_or(u64::MAX));
        if scanned > MAX_EVENT_SCAN_BYTES {
            break;
        }
        let Ok(value) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if model.is_none() {
            model = structured_model(provider, &value).map(str::to_owned);
        }
        if skill_event.is_none() && is_skill_load_event(provider, &value, skill) {
            skill_event = Some(SkillLoadEventEvidence {
                event_type: value
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned(),
                event_sha256: hash_bytes(&line),
                referenced_path: skill.repository_relative_path.clone(),
            });
        }
        if model.is_some() && skill_event.is_some() {
            break;
        }
    }
    Ok((model, skill_event))
}

fn structured_model(provider: Provider, value: &Value) -> Option<&str> {
    let event_type = value.get("type").and_then(Value::as_str)?;
    match provider {
        Provider::Claude
            if event_type == "system"
                && value.get("subtype").and_then(Value::as_str) == Some("init") =>
        {
            value.get("model").and_then(Value::as_str)
        }
        Provider::Codex if matches!(event_type, "thread.started" | "turn.started") => value
            .get("model")
            .or_else(|| value.get("model_name"))
            .and_then(Value::as_str),
        _ => None,
    }
}

fn is_skill_load_event(provider: Provider, value: &Value, skill: &SkillCanarySpec) -> bool {
    match provider {
        Provider::Claude => claude_skill_event(value, skill),
        Provider::Codex => codex_skill_event(value, skill),
    }
}

fn claude_skill_event(value: &Value, skill: &SkillCanarySpec) -> bool {
    if value.get("type").and_then(Value::as_str) != Some("assistant") {
        return false;
    }
    value
        .pointer("/message/content")
        .and_then(Value::as_array)
        .is_some_and(|content| {
            content.iter().any(|item| {
                if item.get("type").and_then(Value::as_str) != Some("tool_use") {
                    return false;
                }
                let name = item.get("name").and_then(Value::as_str).unwrap_or_default();
                let input = item.get("input").unwrap_or(&Value::Null);
                (name == "Skill"
                    && input.get("skill").and_then(Value::as_str)
                        == Some(skill.identifier.as_str()))
                    || (name == "Read" && value_references_skill(input, skill))
            })
        })
}

fn codex_skill_event(value: &Value, skill: &SkillCanarySpec) -> bool {
    if value.get("type").and_then(Value::as_str) != Some("item.completed") {
        return false;
    }
    let Some(item) = value.get("item") else {
        return false;
    };
    let item_type = item.get("type").and_then(Value::as_str).unwrap_or_default();
    if matches!(item_type, "file_read" | "read_file") {
        return value_references_skill(item, skill);
    }
    if item_type == "mcp_tool_call" {
        let code = item
            .pointer("/arguments/code")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .replace('\\', "/")
            .to_ascii_lowercase();
        let path = normalized_path(&skill.repository_relative_path).to_ascii_lowercase();
        return item.get("status").and_then(Value::as_str) == Some("completed")
            && item.get("server").and_then(Value::as_str) == Some("node_repl")
            && item.get("tool").and_then(Value::as_str) == Some("js")
            && item.get("result").is_some_and(|result| !result.is_null())
            && item.get("error").is_none_or(Value::is_null)
            && code.contains("readfile(")
            && code.contains(&path);
    }
    if item_type != "command_execution"
        || item.get("status").and_then(Value::as_str) != Some("completed")
    {
        return false;
    }
    let command = item
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let path = normalized_path(&skill.repository_relative_path).to_ascii_lowercase();
    let read_verb = [
        "get-filehash",
        "get-content",
        "sha256sum",
        "shasum",
        "cat ",
        "type ",
    ]
    .iter()
    .any(|verb| command.contains(verb));
    read_verb && command.contains(&path)
}

fn value_references_skill(value: &Value, skill: &SkillCanarySpec) -> bool {
    let rendered = value.to_string().replace('\\', "/").to_ascii_lowercase();
    rendered.contains(&normalized_path(&skill.repository_relative_path).to_ascii_lowercase())
}

fn safe_relative(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hash_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use chrono::TimeZone as _;
    use tempfile::tempdir;

    use super::*;
    use crate::model::{ProviderIdentity, ResultClass};
    use crate::provider::CodexAdapter;

    #[test]
    fn codex_skill_event_accepts_completed_node_read_of_exact_skill() {
        let skill = SkillCanarySpec::new(
            "rust-tdd",
            PathBuf::from("agents/skills/rust-tdd/SKILL.md"),
            "a".repeat(64),
            "challenge",
            PathBuf::from(".govfolio-loop/marker.json"),
        )
        .expect("skill spec");
        let event = serde_json::json!({
            "type": "item.completed",
            "item": {
                "type": "mcp_tool_call",
                "server": "node_repl",
                "tool": "js",
                "arguments": {
                    "code": "await fs.readFile(nodeRepl.cwd + '/agents/skills/rust-tdd/SKILL.md')"
                },
                "result": {"content": [{"type": "text", "text": "skill bytes"}]},
                "error": null,
                "status": "completed"
            }
        });

        assert!(codex_skill_event(&event, &skill));
    }

    #[test]
    fn codex_skill_event_rejects_prose_or_uncompleted_node_claims() {
        let skill = SkillCanarySpec::new(
            "rust-tdd",
            PathBuf::from("agents/skills/rust-tdd/SKILL.md"),
            "a".repeat(64),
            "challenge",
            PathBuf::from(".govfolio-loop/marker.json"),
        )
        .expect("skill spec");
        let prose = serde_json::json!({
            "type": "item.completed",
            "item": {
                "type": "agent_message",
                "text": "I read agents/skills/rust-tdd/SKILL.md"
            }
        });
        let unfinished = serde_json::json!({
            "type": "item.started",
            "item": {
                "type": "mcp_tool_call",
                "server": "node_repl",
                "tool": "js",
                "arguments": {
                    "code": "await fs.readFile('agents/skills/rust-tdd/SKILL.md')"
                },
                "result": null,
                "error": null,
                "status": "in_progress"
            }
        });

        assert!(!codex_skill_event(&prose, &skill));
        assert!(!codex_skill_event(&unfinished, &skill));
    }

    #[test]
    fn codex_rollout_supplies_model_when_stdout_events_omit_it() {
        let temp = tempdir().expect("tempdir");
        let session_id = "019f5345-f15e-7112-925e-e0fdaa8448da";
        let directory = temp.path().join("sessions/2026/07/11");
        std::fs::create_dir_all(&directory).expect("session directory");
        let rollout = directory.join(format!("rollout-2026-07-11T19-22-00-{session_id}.jsonl"));
        std::fs::write(
            rollout,
            format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"session_id\":\"{session_id}\",\"model_provider\":\"openai\"}}}}\n{{\"type\":\"turn_context\",\"payload\":{{\"model\":\"gpt-5.6-sol\"}}}}\n"
            ),
        )
        .expect("rollout");
        let command = CommandSpec {
            program: PathBuf::from("codex"),
            args: Vec::new(),
            cwd: temp.path().to_path_buf(),
            stdin: Vec::new(),
            env: vec![("CODEX_HOME".to_owned(), temp.path().display().to_string())],
            remove_env: Vec::new(),
        };

        let proof = codex_rollout_model_proof(&command, session_id)
            .expect("proof scan")
            .expect("model proof");

        assert_eq!(proof.model, "gpt-5.6-sol");
        assert_eq!(proof.model_provider, "openai");
        assert_eq!(proof.model_event_count, 1);
        assert!(valid_sha256(&proof.model_events_sha256));
    }

    #[test]
    fn codex_rollout_rejects_model_switch_within_exact_session() {
        let temp = tempdir().expect("tempdir");
        let session_id = "019f5345-f15e-7112-925e-e0fdaa8448da";
        let directory = temp.path().join("sessions/2026/07/11");
        std::fs::create_dir_all(&directory).expect("session directory");
        let rollout = directory.join(format!("rollout-2026-07-11T19-22-00-{session_id}.jsonl"));
        std::fs::write(
            rollout,
            format!(
                "{{\"type\":\"session_meta\",\"payload\":{{\"session_id\":\"{session_id}\",\"model_provider\":\"openai\"}}}}\n{{\"type\":\"turn_context\",\"payload\":{{\"model\":\"gpt-5.6-sol\"}}}}\n{{\"type\":\"turn_context\",\"payload\":{{\"model\":\"wrong-model\"}}}}\n"
            ),
        )
        .expect("rollout");
        let command = CommandSpec {
            program: PathBuf::from("codex"),
            args: Vec::new(),
            cwd: temp.path().to_path_buf(),
            stdin: Vec::new(),
            env: vec![("CODEX_HOME".to_owned(), temp.path().display().to_string())],
            remove_env: Vec::new(),
        };

        let error = codex_rollout_model_proof(&command, session_id)
            .expect_err("model switch must fail closed");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[derive(Clone)]
    struct ScriptedInvocation {
        invocation: CanaryInvocation,
        write_marker: bool,
    }

    struct FakeInvoker {
        scripted: Mutex<VecDeque<ScriptedInvocation>>,
        calls: Mutex<Vec<(CanaryStage, CommandSpec)>>,
    }

    #[async_trait]
    impl CanaryInvoker for FakeInvoker {
        async fn invoke(
            &self,
            stage: CanaryStage,
            _provider: Provider,
            command: &CommandSpec,
            _classifier: Box<dyn EventClassifier>,
            skill: &SkillCanarySpec,
        ) -> Result<CanaryInvocation, CanaryInvocationError> {
            self.calls
                .lock()
                .expect("call lock")
                .push((stage, command.clone()));
            let scripted = self
                .scripted
                .lock()
                .expect("script lock")
                .pop_front()
                .expect("scripted invocation");
            if scripted.write_marker {
                let marker = SkillMarker {
                    schema: SKILL_MARKER_SCHEMA.to_owned(),
                    skill_identifier: skill.identifier.clone(),
                    skill_sha256: skill.approved_sha256.clone(),
                    challenge: skill.challenge.clone(),
                };
                let path = skill.marker_path(&command.cwd);
                std::fs::create_dir_all(path.parent().expect("marker parent"))?;
                std::fs::write(path, serde_json::to_vec(&marker).expect("marker JSON"))?;
            }
            Ok(scripted.invocation)
        }
    }

    #[tokio::test]
    async fn compatibility_canary_proves_skill_fresh_and_exact_resume() {
        let harness = Harness::new().await;
        let invoker = FakeInvoker::successful(&harness.request, Some(7));
        let canary = CompatibilityCanary::new(&harness.store, &harness.artifacts);

        let outcome = canary
            .prove(&CodexAdapter, &invoker, &harness.request, at(0))
            .await
            .expect("canary completes");

        assert!(matches!(
            outcome,
            CanaryOutcome::Proven { cached: false, .. }
        ));
        {
            let calls = invoker.calls.lock().expect("call lock");
            assert_eq!(calls.len(), 2);
            assert!(
                calls[1]
                    .1
                    .args
                    .windows(2)
                    .any(|pair| pair == ["resume", "thread-exact"])
            );
        }
        assert!(matches!(
            canary
                .status(&harness.request, at(1))
                .await
                .expect("status"),
            CanaryStatus::Proven { .. }
        ));
    }

    #[tokio::test]
    async fn compatibility_canary_rejects_model_mismatch_without_resume() {
        let harness = Harness::new().await;
        let mut fresh = invocation(
            &harness.request,
            "thread-exact",
            ResultClass::Completed,
            Some(0),
            true,
        );
        fresh.reported_model = Some("wrong-model".to_owned());
        let invoker = FakeInvoker::new([ScriptedInvocation {
            invocation: fresh,
            write_marker: true,
        }]);
        let canary = CompatibilityCanary::new(&harness.store, &harness.artifacts);

        let outcome = canary
            .prove(&CodexAdapter, &invoker, &harness.request, at(0))
            .await
            .expect("rejection is data");

        assert!(matches!(outcome, CanaryOutcome::Rejected { .. }));
        assert_eq!(invoker.calls.lock().expect("call lock").len(), 1);
        assert!(matches!(
            canary
                .status(&harness.request, at(1))
                .await
                .expect("status"),
            CanaryStatus::Rejected { .. }
        ));
    }

    #[tokio::test]
    async fn compatibility_canary_rejects_prose_only_skill_claim() {
        let harness = Harness::new().await;
        let mut fresh = invocation(
            &harness.request,
            "thread-exact",
            ResultClass::Completed,
            Some(0),
            true,
        );
        fresh.skill_load_event = None;
        let invoker = FakeInvoker::new([ScriptedInvocation {
            invocation: fresh,
            write_marker: true,
        }]);
        let canary = CompatibilityCanary::new(&harness.store, &harness.artifacts);

        let outcome = canary
            .prove(&CodexAdapter, &invoker, &harness.request, at(0))
            .await
            .expect("rejection is data");

        assert!(matches!(
            outcome,
            CanaryOutcome::Rejected { reason, .. }
                if reason.contains("structured skill-load event")
        ));
    }

    #[tokio::test]
    async fn compatibility_canary_rejects_corrupt_terminal_without_resume() {
        let harness = Harness::new().await;
        let fresh = invocation(
            &harness.request,
            "thread-exact",
            ResultClass::Ambiguous,
            Some(1),
            true,
        );
        let invoker = FakeInvoker::new([ScriptedInvocation {
            invocation: fresh,
            write_marker: true,
        }]);
        let canary = CompatibilityCanary::new(&harness.store, &harness.artifacts);

        let outcome = canary
            .prove(&CodexAdapter, &invoker, &harness.request, at(0))
            .await
            .expect("rejection is data");

        assert!(matches!(outcome, CanaryOutcome::Rejected { .. }));
        assert_eq!(invoker.calls.lock().expect("call lock").len(), 1);
    }

    #[tokio::test]
    async fn compatibility_canary_exact_fingerprint_upgrade_needs_new_probe() {
        let harness = Harness::new().await;
        let invoker = FakeInvoker::successful(&harness.request, Some(0));
        let canary = CompatibilityCanary::new(&harness.store, &harness.artifacts);
        canary
            .prove(&CodexAdapter, &invoker, &harness.request, at(0))
            .await
            .expect("initial proof");
        let mut upgraded = harness.request.clone();
        upgraded.attempt.provider.config_fingerprint = "config-upgraded".to_owned();

        assert_eq!(
            canary.status(&upgraded, at(1)).await.expect("status"),
            CanaryStatus::NeedsProbe
        );
    }

    #[tokio::test]
    async fn compatibility_canary_cached_proof_spends_no_invocation() {
        let harness = Harness::new().await;
        let first = FakeInvoker::successful(&harness.request, Some(0));
        let canary = CompatibilityCanary::new(&harness.store, &harness.artifacts);
        canary
            .prove(&CodexAdapter, &first, &harness.request, at(0))
            .await
            .expect("initial proof");
        let empty = FakeInvoker::new([]);

        let outcome = canary
            .prove(&CodexAdapter, &empty, &harness.request, at(1))
            .await
            .expect("cached proof");

        assert!(matches!(
            outcome,
            CanaryOutcome::Proven { cached: true, .. }
        ));
        assert!(empty.calls.lock().expect("call lock").is_empty());
    }

    impl FakeInvoker {
        fn new<const N: usize>(scripted: [ScriptedInvocation; N]) -> Self {
            Self {
                scripted: Mutex::new(VecDeque::from(scripted)),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn successful(request: &CanaryRequest, fresh_exit: Option<i32>) -> Self {
            Self::new([
                ScriptedInvocation {
                    invocation: invocation(
                        request,
                        "thread-exact",
                        ResultClass::Completed,
                        fresh_exit,
                        true,
                    ),
                    write_marker: true,
                },
                ScriptedInvocation {
                    invocation: invocation(
                        request,
                        "thread-exact",
                        ResultClass::Completed,
                        Some(0),
                        false,
                    ),
                    write_marker: false,
                },
            ])
        }
    }

    struct Harness {
        _temp: tempfile::TempDir,
        store: ControlStore,
        artifacts: ArtifactStore,
        request: CanaryRequest,
    }

    impl Harness {
        async fn new() -> Self {
            let temp = tempdir().expect("tempdir");
            let worktree = temp.path().join("worktree");
            let skill_path = worktree.join("agents/skills/rust-tdd/SKILL.md");
            std::fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("skill dir");
            std::fs::write(&skill_path, b"---\nname: rust-tdd\n---\nUse tests first.\n")
                .expect("skill file");
            let skill_hash = hash_bytes(&std::fs::read(&skill_path).expect("skill bytes"));
            let store = ControlStore::open_writer(temp.path().join("control.sqlite3"))
                .await
                .expect("store");
            let artifacts =
                ArtifactStore::new(temp.path().join("runtime"), ArtifactPolicy::default());
            let request = CanaryRequest {
                attempt: AttemptSpec {
                    id: "canary-attempt".to_owned(),
                    lane_id: "compatibility-disposable".to_owned(),
                    lane_fence: 1,
                    work_key: "compatibility".to_owned(),
                    worktree,
                    expected_branch: "compatibility/canary".to_owned(),
                    prompt: String::new(),
                    prompt_kind: PromptKind::CompatibilityCanary,
                    provider: ProviderIdentity {
                        provider: Provider::Codex,
                        executable: PathBuf::from("codex"),
                        cli_version: "codex 1.2.3".to_owned(),
                        model: Some("gpt-test".to_owned()),
                        config_fingerprint: "config-a".to_owned(),
                    },
                    resume_session_id: None,
                    preflight_signature: "preflight".to_owned(),
                    git_head_before: "abc".to_owned(),
                    journal_sha_before: "def".to_owned(),
                },
                provider_key: "codex-account".to_owned(),
                inherited_env: Vec::new(),
                valid_for: Duration::hours(24),
                skill: SkillCanarySpec::new(
                    "rust-tdd",
                    PathBuf::from("agents/skills/rust-tdd/SKILL.md"),
                    skill_hash,
                    "challenge-123",
                    PathBuf::from(".govfolio-loop/skill-proof.json"),
                )
                .expect("skill spec"),
            };
            Self {
                _temp: temp,
                store,
                artifacts,
                request,
            }
        }
    }

    fn invocation(
        request: &CanaryRequest,
        session_id: &str,
        class: ResultClass,
        exit_code: Option<i32>,
        with_skill_event: bool,
    ) -> CanaryInvocation {
        CanaryInvocation {
            result: NormalizedResult {
                class,
                terminal_type: Some("turn.completed".to_owned()),
                structured_started: true,
                session_id: Some(session_id.to_owned()),
                provider_error_code: None,
                stable_error_hash: None,
                retry_at: None,
                exit_code,
                summary: "completed".to_owned(),
            },
            reported_model: request.attempt.provider.model.clone(),
            evidence_ref: "attempts/canary".to_owned(),
            stdout_bytes: 100,
            stderr_bytes: 0,
            skill_load_event: with_skill_event.then(|| SkillLoadEventEvidence {
                event_type: "item.completed".to_owned(),
                event_sha256: "a".repeat(64),
                referenced_path: request.skill.repository_relative_path.clone(),
            }),
        }
    }

    fn at(seconds: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(1_700_000_000 + seconds, 0)
            .single()
            .expect("test timestamp")
    }
}
