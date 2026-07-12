use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, bail};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

pub const EXPERIMENT_SCHEMA_VERSION: u32 = 2;
pub const EVIDENCE_FORMAT_V2: &str = "govfolio-build-experiment-evidence-v2";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentManifest {
    pub schema_version: u32,
    pub experiment_id: String,
    pub lever: String,
    pub baseline: TreePin,
    pub candidate: TreePin,
    pub workload: ExperimentWorkload,
    pub acceptance: ExperimentAcceptance,
    pub target_strategy: TargetStrategy,
    pub expected_duration_seconds: u64,
    pub evidence_format: String,
    pub policy_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rerun_checkpoint: Option<ExperimentRerunCheckpoint>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentRerunCheckpoint {
    pub prior_experiment_id: String,
    pub prior_result_sha256: String,
    pub new_evidence_reason: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TreePin {
    pub commit: String,
    pub tree: String,
    pub lockfile_sha256: String,
    pub toolchain_sha256: String,
    pub linker_config_sha256: String,
    pub profile_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentWorkload {
    pub kind: WorkloadKind,
    pub cargo_args: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_path: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadKind {
    Cold,
    Warm,
    Edit,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentAcceptance {
    pub minimum_improvement_bps: u32,
    pub regression_veto_bps: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TargetStrategy {
    SeparatePrivate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParsedManifest {
    pub manifest: ExperimentManifest,
    pub sha256: String,
}

pub fn parse_manifest(bytes: &[u8]) -> anyhow::Result<ParsedManifest> {
    let manifest: ExperimentManifest =
        serde_json::from_slice(bytes).context("parse experiment manifest")?;
    validate_manifest(&manifest)?;
    Ok(ParsedManifest {
        manifest,
        sha256: sha256(bytes),
    })
}

fn validate_manifest(manifest: &ExperimentManifest) -> anyhow::Result<()> {
    if manifest.schema_version != EXPERIMENT_SCHEMA_VERSION {
        bail!("unsupported experiment schema version");
    }
    if !valid_identifier(&manifest.experiment_id) {
        bail!("experiment id must be 3-64 lowercase safe characters");
    }
    if manifest.lever.trim().is_empty() || manifest.lever.len() > 512 {
        bail!("experiment lever must be non-empty and bounded");
    }
    validate_tree_pin(&manifest.baseline)?;
    validate_tree_pin(&manifest.candidate)?;
    if manifest.workload.cargo_args.is_empty() {
        bail!("experiment workload has no Cargo arguments");
    }
    let normalized = manifest
        .workload
        .cargo_args
        .iter()
        .map(|arg| arg.replace('\\', "/").to_ascii_lowercase())
        .collect::<Vec<_>>();
    if normalized.iter().any(|arg| arg == "--") {
        bail!("experiment workloads cannot pass arguments through to a test binary");
    }
    let cargo_options = normalized.as_slice();
    let command = cargo_options
        .first()
        .map(String::as_str)
        .unwrap_or_default();
    if !matches!(command, "check" | "build" | "clippy" | "test")
        || (command == "test" && !cargo_options.iter().any(|arg| arg == "--no-run"))
        || !cargo_options
            .iter()
            .any(|arg| matches!(arg.as_str(), "--locked" | "--frozen"))
        || cargo_options.iter().any(|arg| {
            arg == "--target-dir"
                || arg.starts_with("--target-dir=")
                || arg == "--manifest-path"
                || arg.starts_with("--manifest-path=")
                || arg == "--config"
                || arg.starts_with("--config=")
                || arg == "--lockfile-path"
                || arg.starts_with("--lockfile-path=")
                || arg.starts_with("-c")
                || matches!(arg.as_str(), "--fix" | "--allow-dirty" | "--allow-staged")
                || arg == "--target"
                || arg.starts_with("--target=")
                || arg.contains("bronze")
                || arg.contains("cargo_target_dir")
        })
    {
        bail!(
            "experiment workload must be locked, host-target, checkout-local compilation-only check/build/clippy or test --no-run"
        );
    }
    match (manifest.workload.kind, &manifest.workload.edit_path) {
        (WorkloadKind::Edit, Some(path)) if safe_edit_path(path) => {}
        (WorkloadKind::Edit, _) => {
            bail!("edit workload requires one safe relative edit_path");
        }
        (WorkloadKind::Cold | WorkloadKind::Warm, None) => {}
        (WorkloadKind::Cold | WorkloadKind::Warm, Some(_)) => {
            bail!("only edit workloads may declare edit_path");
        }
    }
    if manifest.acceptance.minimum_improvement_bps > 10_000
        || manifest.acceptance.regression_veto_bps > 10_000
    {
        bail!("experiment thresholds must be between 0 and 10000 basis points");
    }
    if manifest.expected_duration_seconds == 0 || manifest.expected_duration_seconds > 3_600 {
        bail!("experiment expected duration must be within the one-hour deadline");
    }
    if manifest.evidence_format != EVIDENCE_FORMAT_V2 {
        bail!("unsupported experiment evidence format");
    }
    if !is_hex(&manifest.policy_sha256, 64) {
        bail!("policy hash must be SHA-256");
    }
    if let Some(checkpoint) = &manifest.rerun_checkpoint
        && (!valid_identifier(&checkpoint.prior_experiment_id)
            || !is_hex(&checkpoint.prior_result_sha256, 64)
            || checkpoint.new_evidence_reason.trim().is_empty()
            || checkpoint.new_evidence_reason.len() > 512)
    {
        bail!("experiment rerun checkpoint is invalid");
    }
    Ok(())
}

fn validate_tree_pin(pin: &TreePin) -> anyhow::Result<()> {
    if !(is_hex(&pin.commit, 40) || is_hex(&pin.commit, 64))
        || !(is_hex(&pin.tree, 40) || is_hex(&pin.tree, 64))
        || !is_hex(&pin.lockfile_sha256, 64)
        || !is_hex(&pin.toolchain_sha256, 64)
        || !is_hex(&pin.linker_config_sha256, 64)
        || !is_hex(&pin.profile_sha256, 64)
    {
        bail!("experiment commit, tree, or lockfile pin is malformed");
    }
    Ok(())
}

fn valid_identifier(value: &str) -> bool {
    (3..=64).contains(&value.len())
        && value.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || (index > 0 && matches!(byte, b'-' | b'_' | b'.'))
        })
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn safe_edit_path(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        })
        && path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| matches!(extension, "rs" | "toml"))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "decision", rename_all = "snake_case", deny_unknown_fields)]
pub enum ExperimentReview {
    Accept {
        schema_version: u32,
        experiment_id: String,
        manifest_sha256: String,
        exploratory_evidence_sha256: String,
        reason: String,
        auditor: String,
        reviewed_at: String,
    },
    Reject {
        schema_version: u32,
        experiment_id: String,
        manifest_sha256: String,
        exploratory_evidence_sha256: String,
        reason: String,
        auditor: String,
        reviewed_at: String,
    },
    RequestMatrixRerun {
        schema_version: u32,
        experiment_id: String,
        manifest_sha256: String,
        exploratory_evidence_sha256: String,
        reason: String,
        auditor: String,
        reviewed_at: String,
        repeated_commands: Vec<Vec<String>>,
        estimated_cost_seconds: u64,
    },
}

impl ExperimentReview {
    #[must_use]
    pub fn experiment_id(&self) -> &str {
        match self {
            Self::Accept { experiment_id, .. }
            | Self::Reject { experiment_id, .. }
            | Self::RequestMatrixRerun { experiment_id, .. } => experiment_id,
        }
    }

    #[must_use]
    pub fn manifest_sha256(&self) -> &str {
        match self {
            Self::Accept {
                manifest_sha256, ..
            }
            | Self::Reject {
                manifest_sha256, ..
            }
            | Self::RequestMatrixRerun {
                manifest_sha256, ..
            } => manifest_sha256,
        }
    }

    #[must_use]
    pub fn exploratory_evidence_sha256(&self) -> &str {
        match self {
            Self::Accept {
                exploratory_evidence_sha256,
                ..
            }
            | Self::Reject {
                exploratory_evidence_sha256,
                ..
            }
            | Self::RequestMatrixRerun {
                exploratory_evidence_sha256,
                ..
            } => exploratory_evidence_sha256,
        }
    }

    #[must_use]
    pub fn reviewed_at(&self) -> &str {
        match self {
            Self::Accept { reviewed_at, .. }
            | Self::Reject { reviewed_at, .. }
            | Self::RequestMatrixRerun { reviewed_at, .. } => reviewed_at,
        }
    }
}

pub fn parse_review(bytes: &[u8]) -> anyhow::Result<ExperimentReview> {
    let review: ExperimentReview =
        serde_json::from_slice(bytes).context("parse experiment review")?;
    let (schema_version, reason, auditor, reviewed_at) = match &review {
        ExperimentReview::Accept {
            schema_version,
            reason,
            auditor,
            reviewed_at,
            ..
        }
        | ExperimentReview::Reject {
            schema_version,
            reason,
            auditor,
            reviewed_at,
            ..
        }
        | ExperimentReview::RequestMatrixRerun {
            schema_version,
            reason,
            auditor,
            reviewed_at,
            ..
        } => (*schema_version, reason, auditor, reviewed_at),
    };
    if schema_version != EXPERIMENT_SCHEMA_VERSION
        || !valid_identifier(review.experiment_id())
        || !is_hex(review.manifest_sha256(), 64)
        || !is_hex(review.exploratory_evidence_sha256(), 64)
        || reason.trim().is_empty()
        || auditor.trim().is_empty()
        || DateTime::parse_from_rfc3339(reviewed_at).is_err()
    {
        bail!("experiment review metadata is invalid");
    }
    if let ExperimentReview::RequestMatrixRerun {
        repeated_commands,
        estimated_cost_seconds,
        ..
    } = &review
        && (repeated_commands.is_empty()
            || *estimated_cost_seconds == 0
            || *estimated_cost_seconds > 3_600
            || repeated_commands.iter().any(|command| {
                command.is_empty() || command.first().is_some_and(|arg| arg == "clean")
            }))
    {
        bail!("matrix rerun checkpoint is incomplete or unsafe");
    }
    Ok(review)
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleVariant {
    Baseline,
    Candidate,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplePhase {
    Preparation,
    Exploratory,
    Confidence,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleDisposition {
    Completed,
    Failed,
    TimedOut,
    Cancelled,
    Inconclusive,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SampleMeasurement {
    pub variant: SampleVariant,
    pub phase: SamplePhase,
    pub ordinal: u32,
    pub wall_ms: Option<u64>,
    pub disposition: SampleDisposition,
}

impl SampleMeasurement {
    #[must_use]
    pub fn completed(
        variant: SampleVariant,
        phase: SamplePhase,
        ordinal: u32,
        wall_ms: u64,
    ) -> Self {
        Self {
            variant,
            phase,
            ordinal,
            wall_ms: Some(wall_ms),
            disposition: SampleDisposition::Completed,
        }
    }

    #[must_use]
    pub fn inconclusive(variant: SampleVariant, phase: SamplePhase, ordinal: u32) -> Self {
        Self {
            variant,
            phase,
            ordinal,
            wall_ms: None,
            disposition: SampleDisposition::Inconclusive,
        }
    }
}

#[must_use]
pub fn deterministic_pair_order(experiment_id: &str, _pair_index: u32) -> [SampleVariant; 2] {
    let digest = Sha256::digest(experiment_id.as_bytes());
    let baseline_first = digest[0] & 1 == 0;
    if baseline_first {
        [SampleVariant::Baseline, SampleVariant::Candidate]
    } else {
        [SampleVariant::Candidate, SampleVariant::Baseline]
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING-KEBAB-CASE")]
pub enum ExperimentOutcome {
    Go,
    NoGo,
    Inconclusive,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExperimentEvaluation {
    pub outcome: ExperimentOutcome,
    pub baseline_median_ms: Option<u64>,
    pub candidate_median_ms: Option<u64>,
    pub improvement_bps: Option<i64>,
    pub reason: String,
}

#[expect(
    clippy::too_many_lines,
    reason = "one evaluator keeps fixed sample counts, failure semantics, medians, threshold, and veto atomic"
)]
pub fn evaluate_measurements(
    manifest: &ExperimentManifest,
    samples: &[SampleMeasurement],
) -> anyhow::Result<ExperimentEvaluation> {
    let confidence = samples
        .iter()
        .any(|sample| sample.phase == SamplePhase::Confidence);
    let expected_per_variant = if confidence {
        if manifest.workload.kind == WorkloadKind::Cold {
            bail!("cold experiments are limited to one exploratory pair");
        }
        3
    } else {
        1
    };
    let selected_phase = if confidence {
        SamplePhase::Confidence
    } else {
        SamplePhase::Exploratory
    };
    let selected = samples
        .iter()
        .filter(|sample| sample.phase == selected_phase)
        .collect::<Vec<_>>();
    let baseline = selected
        .iter()
        .filter(|sample| sample.variant == SampleVariant::Baseline)
        .copied()
        .collect::<Vec<_>>();
    let candidate = selected
        .iter()
        .filter(|sample| sample.variant == SampleVariant::Candidate)
        .copied()
        .collect::<Vec<_>>();
    if baseline.len() != expected_per_variant || candidate.len() != expected_per_variant {
        bail!("experiment evidence does not contain the required fixed sample count");
    }
    if selected_phase == SamplePhase::Exploratory && manifest.workload.kind != WorkloadKind::Cold {
        let preparation = samples
            .iter()
            .filter(|sample| sample.phase == SamplePhase::Preparation)
            .collect::<Vec<_>>();
        let preparation_is_complete = preparation.len() == 2
            && [SampleVariant::Baseline, SampleVariant::Candidate]
                .iter()
                .all(|variant| {
                    preparation.iter().any(|sample| {
                        sample.variant == *variant
                            && sample.disposition == SampleDisposition::Completed
                            && sample.wall_ms.is_some()
                    })
                });
        if !preparation_is_complete {
            return Ok(ExperimentEvaluation {
                outcome: ExperimentOutcome::Inconclusive,
                baseline_median_ms: None,
                candidate_median_ms: None,
                improvement_bps: None,
                reason: "warm/edit preparation did not complete for both variants".to_owned(),
            });
        }
    }
    if baseline
        .iter()
        .all(|sample| sample.disposition == SampleDisposition::Completed)
        && candidate
            .iter()
            .any(|sample| sample.disposition == SampleDisposition::Failed)
    {
        return Ok(ExperimentEvaluation {
            outcome: ExperimentOutcome::NoGo,
            baseline_median_ms: None,
            candidate_median_ms: None,
            improvement_bps: None,
            reason: "candidate Cargo command failed without retryable measurement evidence"
                .to_owned(),
        });
    }
    if selected.iter().any(|sample| {
        sample.disposition != SampleDisposition::Completed || sample.wall_ms.is_none()
    }) {
        return Ok(ExperimentEvaluation {
            outcome: ExperimentOutcome::Inconclusive,
            baseline_median_ms: None,
            candidate_median_ms: None,
            improvement_bps: None,
            reason: "invalid environment, interference, timeout, cancellation, or failed sample"
                .to_owned(),
        });
    }
    let baseline_median = median(
        baseline
            .iter()
            .filter_map(|sample| sample.wall_ms)
            .collect(),
    )?;
    let candidate_median = median(
        candidate
            .iter()
            .filter_map(|sample| sample.wall_ms)
            .collect(),
    )?;
    if baseline_median == 0 {
        return Ok(ExperimentEvaluation {
            outcome: ExperimentOutcome::Inconclusive,
            baseline_median_ms: Some(0),
            candidate_median_ms: Some(candidate_median),
            improvement_bps: None,
            reason: "zero baseline duration is not useful evidence".to_owned(),
        });
    }
    let improvement = i64::try_from(
        (i128::from(baseline_median) - i128::from(candidate_median)) * 10_000
            / i128::from(baseline_median),
    )
    .context("experiment improvement exceeds i64")?;
    let outcome = if improvement < -i64::from(manifest.acceptance.regression_veto_bps) {
        ExperimentOutcome::NoGo
    } else if improvement >= i64::from(manifest.acceptance.minimum_improvement_bps) {
        ExperimentOutcome::Go
    } else {
        ExperimentOutcome::NoGo
    };
    let reason = match outcome {
        ExperimentOutcome::Go => "candidate median satisfies the acceptance threshold",
        ExperimentOutcome::NoGo => "candidate has no useful signal or triggers the regression veto",
        ExperimentOutcome::Inconclusive => unreachable!(),
    };
    Ok(ExperimentEvaluation {
        outcome,
        baseline_median_ms: Some(baseline_median),
        candidate_median_ms: Some(candidate_median),
        improvement_bps: Some(improvement),
        reason: reason.to_owned(),
    })
}

fn median(mut values: Vec<u64>) -> anyhow::Result<u64> {
    if values.is_empty() {
        bail!("cannot compute an empty median");
    }
    values.sort_unstable();
    Ok(values[values.len() / 2])
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImmutableArtifact {
    pub path: PathBuf,
    pub sha256: String,
    pub size_bytes: u64,
}

pub fn write_immutable_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> anyhow::Result<ImmutableArtifact> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_immutable_bytes(path, &bytes)
}

pub fn write_immutable_bytes(path: &Path, bytes: &[u8]) -> anyhow::Result<ImmutableArtifact> {
    if let Some(parent) = path.parent() {
        create_private_dir_all(parent)?;
    }
    if path.exists() {
        let existing = std::fs::read(path)?;
        if existing != bytes {
            bail!("immutable experiment artifact differs: {}", path.display());
        }
    } else {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .context("immutable artifact has no UTF-8 file name")?;
        let temporary = path.with_file_name(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_nanos()
        ));
        let mut file = open_private_new(&temporary)?;
        file.write_all(bytes)?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        match std::fs::hard_link(&temporary, path) {
            Ok(()) => {
                std::fs::remove_file(&temporary)?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = std::fs::read(path)?;
                if existing != bytes {
                    bail!("immutable experiment artifact differs: {}", path.display());
                }
                std::fs::remove_file(&temporary)?;
            }
            Err(error) => return Err(error.into()),
        }
    }
    Ok(ImmutableArtifact {
        path: path.to_path_buf(),
        sha256: sha256(bytes),
        size_bytes: u64::try_from(bytes.len()).context("experiment artifact is too large")?,
    })
}

pub fn create_private_dir_all(path: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub fn open_private_new(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)
}

#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
