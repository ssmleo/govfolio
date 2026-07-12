use std::fs::File;
use std::io::{self, BufRead as _, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use anyhow::{Context, anyhow, bail};
use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::Row;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::oneshot;
use tokio::time::{MissedTickBehavior, interval, sleep};
use ulid::Ulid;

use crate::artifacts::{ArtifactPolicy, ArtifactStore, AttemptArtifactPolicy, atomic_write_new};
use crate::canary::{
    COMPATIBILITY_KIND, CanaryOutcome, CanaryRequest, CompatibilityCanary, ProcessCanaryInvoker,
    SkillCanarySpec,
};
use crate::config::LoopConfig;
use crate::failover::{FailoverAction, FailoverBudget};
use crate::host::{
    NativeCodexResolver, NativeResolverInputs, NativeSmokeRequest, SystemHostCommandRunner,
    SystemNativeExecutableProbe, persist_native_identity, run_native_smoke,
};
use crate::integration::{
    CommandIntegrationBackend, FinalizeOutcome, IntegrationEngine, PrepareOutcome, ReceiptCandidate,
};
use crate::model::{
    AttemptSpec, NormalizedResult, PromptKind, Provider, ProviderIdentity, ResultClass,
    SuppressionReason, TickOutcome,
};
use crate::policy::{PolicyEngine, RetryAction, SystemClock};
use crate::preflight::{
    AuthorityProbe, CompilerProbe, DataProbe, DiskProbe, FactoryProbe, GitProbe, PreflightReport,
    PreflightSuite, Probe, ProbeOutcome, ProviderCliProbe, RuntimeSeparationProbe,
    SkillContractProbe,
};
use crate::process::{ProcessOutputPaths, ProcessRunner, cancellation_pair};
use crate::provider::{ClaudeAdapter, CodexAdapter, ProviderAdapter};
use crate::store::{
    ControlStore, FailureObservation, FingerprintGate, LaneFence, LaneRuntimeContext, ProviderGate,
    ReceiptMirror, StoreError, SupervisorFence, SystemGate,
};

const OWNER_TTL: Duration = Duration::seconds(90);
const HEARTBEAT_INTERVAL: StdDuration = StdDuration::from_secs(20);
const HALF_OPEN_TTL: Duration = Duration::minutes(30);
const ROOT_ENVELOPE_BEGIN: &str = "--- GOVFOLIO_DISPATCH_V1 ---";
const ROOT_ENVELOPE_END: &str = "--- END GOVFOLIO_DISPATCH_V1 ---";
const MAX_ROOT_RECEIPT_SCAN_BYTES: u64 = 64 * 1024 * 1024;
type DomainLeaseRow = (
    String,
    String,
    String,
    chrono::DateTime<Utc>,
    i64,
    Option<String>,
);

/// Entry point used by the pre-built `govfolio-loop` binary.
pub fn cli_main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().cloned().unwrap_or_else(|| "help".to_owned());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("build supervisor runtime")?;
    runtime.block_on(async move {
        match command.as_str() {
            "run" => run(false).await,
            "once" => run(true).await,
            "status" => status().await,
            "doctor" => doctor().await,
            "backup" => backup().await,
            "submit-receipt" => submit_receipt(required_arg(&args, 1, "receipt JSON path")?).await,
            "receipt-status" => receipt_status(required_arg(&args, 1, "receipt id")?).await,
            "integrate" => integrate_command(false).await,
            "integrate-once" => integrate_command(true).await,
            "recover-lane" => recover_lane(required_arg(&args, 1, "lane id")?).await,
            "probe-native-codex" => probe_native_codex().await,
            "canary" => {
                let provider = required_arg(&args, 1, "provider (codex|claude)")?;
                let skill = args
                    .get(2)
                    .map_or("agents/skills/rust-tdd/SKILL.md", String::as_str);
                compatibility_canary(provider, skill).await
            }
            "help" | "--help" | "-h" => {
                print_help();
                Ok(())
            }
            unknown => bail!("unknown command {unknown:?}; use govfolio-loop help"),
        }
    })
}

fn print_help() {
    println!(
        "govfolio-loop run|once|status|doctor|backup|submit-receipt <json>|receipt-status <id>|integrate|recover-lane <lane-id>|probe-native-codex|canary <codex|claude> [skill]"
    );
}

fn required_arg<'a>(args: &'a [String], index: usize, label: &str) -> anyhow::Result<&'a str> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| anyhow!("missing {label}"))
}

async fn run(once: bool) -> anyhow::Result<()> {
    let configs = configured_lanes()?;
    let primary = configs
        .first()
        .ok_or_else(|| anyhow!("no configured supervisor lanes"))?;
    primary
        .primary
        .paths
        .ensure()
        .context("create runtime state layout")?;
    let store = Arc::new(
        ControlStore::open_writer(&primary.primary.paths.control_db)
            .await
            .context("open fenced control-store writer")?,
    );
    let integration_config = primary.primary.clone();
    let prepared = prepare_lanes(configs, &store).await?;
    let owner_id = format!("{}-{}", std::process::id(), Ulid::new());
    let now = Utc::now();
    let supervisor = store
        .acquire_supervisor(&owner_id, now, OWNER_TTL)
        .await
        .context("acquire host supervisor fence")?;
    store
        .renew_supervisor(&supervisor, std::process::id(), now, OWNER_TTL)
        .await?;
    let run_id = Ulid::new().to_string();
    let mut tasks = tokio::task::JoinSet::new();
    let launch = LaneLaunchContext {
        store: Arc::clone(&store),
        supervisor: supervisor.clone(),
        owner_id,
        run_id,
        once,
        acquired_at: now,
    };
    for lane in prepared {
        if let Err(error) = launch_lane(&mut tasks, lane, &launch).await {
            tasks.abort_all();
            while tasks.join_next().await.is_some() {}
            let _release = store.release_supervisor(&supervisor, Utc::now()).await;
            return Err(error);
        }
    }

    let integration_store = Arc::clone(&store);
    tasks.spawn(
        async move { integrate_forever(&integration_config, &integration_store, once).await },
    );

    let mut first_error = None;
    while let Some(joined) = tasks.join_next().await {
        match joined {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                first_error = Some(error);
                tasks.abort_all();
            }
            Err(error) => {
                first_error = Some(anyhow!("lane task failed: {error}"));
                tasks.abort_all();
            }
        }
    }
    store
        .release_supervisor(&supervisor, Utc::now())
        .await
        .context("release supervisor fence")?;
    first_error.map_or(Ok(()), Err)
}

struct LaneLaunchContext {
    store: Arc<ControlStore>,
    supervisor: SupervisorFence,
    owner_id: String,
    run_id: String,
    once: bool,
    acquired_at: chrono::DateTime<Utc>,
}

#[derive(Clone, Debug)]
struct ConfiguredLane {
    primary: LoopConfig,
    fallback: Option<LoopConfig>,
}

#[derive(Clone, Debug)]
struct PreparedLane {
    config: LoopConfig,
    provider: ProviderIdentity,
    fallback: Option<ProviderIdentity>,
}

async fn prepare_lanes(
    configured: Vec<ConfiguredLane>,
    store: &ControlStore,
) -> anyhow::Result<Vec<PreparedLane>> {
    let mut prepared = Vec::with_capacity(configured.len());
    for lane in configured {
        let primary = proven_identity(&lane.primary, store).await;
        let fallback = match lane.fallback {
            Some(config) => Some(proven_identity(&config, store).await),
            None => None,
        };
        match primary {
            Ok(provider) => {
                let fallback = fallback.and_then(|result| match result {
                    Ok(identity) => Some(identity),
                    Err(error) => {
                        eprintln!(
                            "govfolio-loop: alternate provider disabled for {}: {error:#}",
                            lane.primary.lane_id
                        );
                        None
                    }
                });
                prepared.push(PreparedLane {
                    config: lane.primary,
                    provider,
                    fallback,
                });
            }
            Err(primary_error) => match fallback {
                Some(Ok(provider)) => {
                    eprintln!(
                        "govfolio-loop: preferred provider unavailable for {}; starting proven alternate: {primary_error:#}",
                        lane.primary.lane_id
                    );
                    prepared.push(PreparedLane {
                        config: lane.primary,
                        provider,
                        fallback: None,
                    });
                }
                Some(Err(fallback_error)) => {
                    return Err(primary_error.context(format!(
                        "fallback provider is also unavailable: {fallback_error:#}"
                    )));
                }
                None => return Err(primary_error),
            },
        }
    }
    Ok(prepared)
}

async fn launch_lane(
    tasks: &mut tokio::task::JoinSet<anyhow::Result<()>>,
    prepared: PreparedLane,
    launch: &LaneLaunchContext,
) -> anyhow::Result<()> {
    let config = prepared.config;
    let provider = prepared.provider;
    let fallback = prepared.fallback;
    let lane = launch
        .store
        .acquire_lane(
            &config.lane_id,
            &launch.owner_id,
            &launch.supervisor,
            launch.acquired_at,
            OWNER_TTL,
        )
        .await
        .with_context(|| format!("acquire lane {}", config.lane_id))?;
    launch
        .store
        .update_lane_context(
            &lane,
            &LaneRuntimeContext {
                role: config.role.clone(),
                worktree: config.worktree.clone(),
                expected_branch: config.expected_branch.clone(),
                provider_key: Some(provider_key(&provider)),
                pid: None,
            },
            Utc::now(),
        )
        .await?;
    let artifacts = ArtifactStore::new(&config.paths.root, ArtifactPolicy::default());
    let task_store = Arc::clone(&launch.store);
    let task_supervisor = launch.supervisor.clone();
    let task_run_id = launch.run_id.clone();
    let once = launch.once;
    tasks.spawn(async move {
        let result = owned_loop(
            &config,
            &task_store,
            &task_supervisor,
            &lane,
            &provider,
            fallback.as_ref(),
            &artifacts,
            &task_run_id,
            once,
        )
        .await;
        let lane_is_recovery = match &result {
            Ok(TickOutcome::RecoveryRequired { .. }) => true,
            Err(error) => error.to_string().contains("recovery_required"),
            _ => false,
        };
        if !lane_is_recovery {
            task_store.release_lane(&lane, Utc::now()).await?;
        }
        result.map(|_| ())
    });
    Ok(())
}

fn configured_lanes() -> anyhow::Result<Vec<ConfiguredLane>> {
    let primary = LoopConfig::from_env()?;
    let count = std::env::var("GOVFOLIO_FACTORY_LANES")
        .ok()
        .map(|value| value.parse::<usize>())
        .transpose()
        .context("parse GOVFOLIO_FACTORY_LANES")?
        .unwrap_or(0);
    let mut lanes = Vec::with_capacity(count.saturating_add(1));
    lanes.push(ConfiguredLane {
        fallback: configured_fallback(&primary)?,
        primary: primary.clone(),
    });
    for index in 1..=count {
        let prefix = format!("GOVFOLIO_FACTORY_{index}");
        let worktree = std::env::var_os(format!("{prefix}_WORKTREE"))
            .map(PathBuf::from)
            .ok_or_else(|| anyhow!("{prefix}_WORKTREE is required"))?;
        let lane_id = std::env::var(format!("{prefix}_LANE_ID"))
            .unwrap_or_else(|_| format!("factory-{index}"));
        let mut lane = primary.clone();
        lane.worktree.clone_from(&worktree);
        lane.lane_id.clone_from(&lane_id);
        "factory".clone_into(&mut lane.role);
        lane.expected_branch =
            std::env::var(format!("{prefix}_BRANCH")).unwrap_or_else(|_| format!("loop/{lane_id}"));
        lane.provider = Provider::Claude;
        lane.provider_executable = std::env::var_os("GOVFOLIO_CLAUDE_BIN")
            .map_or_else(|| PathBuf::from("claude"), PathBuf::from);
        lane.model = std::env::var(format!("{prefix}_MODEL"))
            .ok()
            .filter(|value| !value.is_empty());
        lane.prompt_file =
            factory_prompt_file(&worktree, std::env::var_os(format!("{prefix}_PROMPT")));
        lanes.push(ConfiguredLane {
            primary: lane,
            fallback: None,
        });
    }
    Ok(lanes)
}

fn factory_prompt_file(worktree: &Path, configured: Option<std::ffi::OsString>) -> PathBuf {
    configured.map_or_else(
        || worktree.join("agents").join("PROMPT-FACTORY-LANE.md"),
        PathBuf::from,
    )
}

fn configured_fallback(primary: &LoopConfig) -> anyhow::Result<Option<LoopConfig>> {
    let requested = std::env::var("GOVFOLIO_LOOP_FALLBACK_PROVIDER").unwrap_or_else(|_| {
        match primary.provider {
            Provider::Codex => "claude".to_owned(),
            Provider::Claude => "codex".to_owned(),
        }
    });
    if requested.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    let provider = requested.parse::<Provider>().map_err(anyhow::Error::msg)?;
    if provider == primary.provider {
        bail!("lane fallback provider must differ from the primary provider");
    }
    let mut fallback = primary.clone();
    fallback.provider = provider;
    fallback.provider_executable = match provider {
        Provider::Claude => std::env::var_os("GOVFOLIO_CLAUDE_BIN")
            .map_or_else(|| PathBuf::from("claude"), PathBuf::from),
        Provider::Codex => std::env::var_os("GOVFOLIO_CODEX_BIN")
            .map_or_else(|| PathBuf::from("codex"), PathBuf::from),
    };
    fallback.model = match provider {
        Provider::Claude => std::env::var("GOVFOLIO_CLAUDE_MODEL").ok(),
        Provider::Codex => std::env::var("GOVFOLIO_CODEX_MODEL").ok(),
    }
    .filter(|value| !value.is_empty());
    Ok(Some(fallback))
}

#[allow(clippy::too_many_arguments)]
async fn owned_loop(
    config: &LoopConfig,
    store: &ControlStore,
    supervisor: &SupervisorFence,
    lane: &LaneFence,
    provider: &ProviderIdentity,
    fallback: Option<&ProviderIdentity>,
    artifacts: &ArtifactStore,
    run_id: &str,
    once: bool,
) -> anyhow::Result<TickOutcome> {
    let mut displayed_provider = provider_key(provider);
    loop {
        let (active, alternate) =
            select_lane_provider(store, provider, fallback, Utc::now()).await?;
        let active_key = provider_key(active);
        if active_key != displayed_provider {
            store
                .update_lane_context(
                    lane,
                    &LaneRuntimeContext {
                        role: config.role.clone(),
                        worktree: config.worktree.clone(),
                        expected_branch: config.expected_branch.clone(),
                        provider_key: Some(active_key.clone()),
                        pid: None,
                    },
                    Utc::now(),
                )
                .await?;
            displayed_provider = active_key;
        }
        let outcome = tick(config, store, supervisor, lane, active, artifacts).await?;
        let outcome = maybe_failover(
            config, store, supervisor, lane, active, alternate, artifacts, outcome,
        )
        .await?;
        append_event(artifacts, run_id, &outcome)?;
        render_outcome(&outcome);
        if once
            || matches!(
                &outcome,
                TickOutcome::RecoveryRequired { .. }
                    | TickOutcome::Failed {
                        class: ResultClass::OperatorStop,
                        ..
                    }
            )
        {
            return Ok(outcome);
        }
        tokio::select! {
            () = sleep(config.poll_interval) => {}
            signal = tokio::signal::ctrl_c() => {
                signal.context("wait for operator stop")?;
                return Ok(TickOutcome::Suppressed {
                    reason: SuppressionReason::AttemptBudget,
                    retry_at: None,
                });
            }
        }
    }
}

async fn select_lane_provider<'a>(
    store: &ControlStore,
    preferred: &'a ProviderIdentity,
    fallback: Option<&'a ProviderIdentity>,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<(&'a ProviderIdentity, Option<&'a ProviderIdentity>)> {
    let gate = store
        .provider_gate(&provider_key(preferred), &preferred.config_fingerprint, now)
        .await?;
    if matches!(gate, ProviderGate::Closed | ProviderGate::HalfOpenAvailable) {
        return Ok((preferred, fallback));
    }
    Ok(fallback.map_or((preferred, None), |fallback| (fallback, None)))
}

async fn proven_identity(
    config: &LoopConfig,
    store: &ControlStore,
) -> anyhow::Result<ProviderIdentity> {
    let provider = runtime_provider_identity(config).await?;
    let model = provider
        .model
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("{} requires an explicit model", provider.provider))?;
    let key = provider_key(&provider);
    let record = store
        .compatibility(
            &key,
            &provider.cli_version,
            model,
            &provider.config_fingerprint,
            COMPATIBILITY_KIND,
            Utc::now(),
        )
        .await?;
    if record.is_some_and(|record| record.proven) {
        Ok(provider)
    } else {
        bail!(
            "provider {key} has no current structured exact-resume and skill-load proof; run govfolio-loop canary {}",
            provider.provider
        )
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "the bounded takeover retains the exact supervisor and lane fences"
)]
async fn maybe_failover(
    config: &LoopConfig,
    store: &ControlStore,
    supervisor: &SupervisorFence,
    lane: &LaneFence,
    primary: &ProviderIdentity,
    fallback: Option<&ProviderIdentity>,
    artifacts: &ArtifactStore,
    outcome: TickOutcome,
) -> anyhow::Result<TickOutcome> {
    let TickOutcome::Failed {
        ref attempt_id,
        class,
    } = outcome
    else {
        return Ok(outcome);
    };
    let Some(fallback) = fallback else {
        return Ok(outcome);
    };
    let budget = FailoverBudget::new(primary.provider);
    match budget.decide(class, fallback.provider, true) {
        FailoverAction::Stay => Ok(outcome),
        FailoverAction::FenceRecovery => {
            store
                .mark_lane_recovery_required(lane, class.as_str(), Utc::now())
                .await?;
            Ok(TickOutcome::RecoveryRequired {
                lane_id: lane.lane_id.clone(),
                reason: format!("{class} requires reconciliation before failover"),
            })
        }
        FailoverAction::FreshAlternate => {
            let fallback_key = provider_key(fallback);
            store
                .update_lane_context(
                    lane,
                    &LaneRuntimeContext {
                        role: config.role.clone(),
                        worktree: config.worktree.clone(),
                        expected_branch: config.expected_branch.clone(),
                        provider_key: Some(fallback_key),
                        pid: None,
                    },
                    Utc::now(),
                )
                .await?;
            let alternate = alternate_tick(
                config, store, supervisor, lane, fallback, artifacts, attempt_id,
            )
            .await?;
            if matches!(alternate, TickOutcome::Completed(_)) {
                store
                    .update_lane_context(
                        lane,
                        &LaneRuntimeContext {
                            role: config.role.clone(),
                            worktree: config.worktree.clone(),
                            expected_branch: config.expected_branch.clone(),
                            provider_key: Some(provider_key(primary)),
                            pid: None,
                        },
                        Utc::now(),
                    )
                    .await?;
                return Ok(alternate);
            }
            if matches!(alternate, TickOutcome::RecoveryRequired { .. }) {
                return Ok(alternate);
            }
            store
                .mark_lane_recovery_required(
                    lane,
                    "alternate provider recovery did not complete",
                    Utc::now(),
                )
                .await?;
            Ok(TickOutcome::RecoveryRequired {
                lane_id: lane.lane_id.clone(),
                reason: "bounded alternate-provider recovery did not complete".to_owned(),
            })
        }
    }
}

async fn alternate_tick(
    config: &LoopConfig,
    store: &ControlStore,
    supervisor: &SupervisorFence,
    lane: &LaneFence,
    provider: &ProviderIdentity,
    artifacts: &ArtifactStore,
    original_attempt_id: &str,
) -> anyhow::Result<TickOutcome> {
    let now = Utc::now();
    let key = provider_key(provider);
    if let Some(outcome) = gate_system(store, &key, now).await? {
        return Ok(outcome);
    }
    if let Some(outcome) = gate_provider(store, provider, &key, now).await? {
        return Ok(outcome);
    }
    let preflight = preflight_suite(config, provider).run(now).await;
    if let Some(outcome) = suppress_preflight(store, lane, &key, &preflight, now).await? {
        return Ok(outcome);
    }
    let original = store.attempt_spec(original_attempt_id).await?;
    let attempt = AttemptSpec {
        id: Ulid::new().to_string(),
        lane_id: lane.lane_id.clone(),
        lane_fence: lane.fence,
        work_key: original.work_key,
        worktree: config.worktree.clone(),
        expected_branch: config.expected_branch.clone(),
        prompt: recovery_prompt(&original.prompt, lane.fence),
        required_root_receipt: original.required_root_receipt,
        required_root_reads: original.required_root_reads,
        prompt_kind: PromptKind::Recovery,
        provider: provider.clone(),
        resume_session_id: None,
        preflight_signature: preflight.signature,
        git_head_before: git_text(&config.worktree, &["rev-parse", "HEAD"])?,
        journal_sha_before: file_sha(&config.worktree.join("agents").join("JOURNAL.md"))?,
    };
    store
        .reserve_alternate_attempt(lane, original_attempt_id, &attempt, now)
        .await?;
    let context = TickContext {
        config,
        store,
        supervisor,
        lane,
        provider,
        artifacts,
    };
    let execution = execute_attempt(&context, &attempt).await?;
    finalize_attempt(&context, &key, attempt, execution).await
}

fn recovery_prompt(original: &str, fence: i64) -> String {
    format!(
        "{original}\n\n# Cross-provider recovery\n\nFresh recovery under fence {fence}. After satisfying the governed root receipt boundary above, reconcile authoritative Git, registry, Bronze, and receipt state before continuing. Never trust provider session history."
    )
}

async fn tick(
    config: &LoopConfig,
    store: &ControlStore,
    supervisor: &SupervisorFence,
    lane: &LaneFence,
    provider: &ProviderIdentity,
    artifacts: &ArtifactStore,
) -> anyhow::Result<TickOutcome> {
    let context = TickContext {
        config,
        store,
        supervisor,
        lane,
        provider,
        artifacts,
    };
    let now = Utc::now();
    let provider_key = provider_key(provider);
    if let Some(outcome) = gate_system(store, &provider_key, now).await? {
        return Ok(outcome);
    }
    if let Some(outcome) = gate_provider(store, provider, &provider_key, now).await? {
        return Ok(outcome);
    }

    let preflight = preflight_suite(config, provider).run(now).await;
    if let Some(outcome) = suppress_preflight(store, lane, &provider_key, &preflight, now).await? {
        return Ok(outcome);
    }
    store.mark_system_diagnostics_passed(now).await?;
    let attempt = match reserve_attempt(&context, &provider_key, preflight.signature, now).await? {
        AttemptReservation::Ready(attempt) => attempt,
        AttemptReservation::Suppressed(outcome) => return Ok(outcome),
    };
    let execution = execute_attempt(&context, &attempt).await?;
    finalize_attempt(&context, &provider_key, *attempt, execution).await
}

struct TickContext<'a> {
    config: &'a LoopConfig,
    store: &'a ControlStore,
    supervisor: &'a SupervisorFence,
    lane: &'a LaneFence,
    provider: &'a ProviderIdentity,
    artifacts: &'a ArtifactStore,
}

enum AttemptReservation {
    Ready(Box<AttemptSpec>),
    Suppressed(TickOutcome),
}

#[derive(Debug)]
struct GovernedRootPrompt {
    text: String,
    receipt: String,
    reads: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RootDispatchEnvelope {
    contract_sha256: String,
    role: String,
    skills: Vec<RootDispatchSkill>,
}

#[derive(Debug, Deserialize)]
struct RootDispatchSkill {
    id: String,
    codex_name: String,
    canonical_path: String,
}

fn governed_root_prompt(
    config: &LoopConfig,
    task_prompt: &str,
) -> anyhow::Result<GovernedRootPrompt> {
    let resolver = config
        .worktree
        .join("scripts")
        .join("agents")
        .join("resolve-codex-dispatch.mjs");
    let mut command = Command::new("node");
    command
        .arg(&resolver)
        .arg("--repo-root")
        .arg(&config.worktree)
        .arg("--role")
        .arg("orchestrator")
        .current_dir(&config.worktree);
    if config.role == "factory" {
        command.arg("--trigger").arg("trigger:parallel-work");
    }
    let output = command
        .output()
        .with_context(|| format!("render governed root dispatch for lane {}", config.lane_id))?;
    if !output.status.success() {
        bail!(
            "root dispatch resolver failed for lane {}: {}",
            config.lane_id,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let envelope_text = String::from_utf8(output.stdout)
        .context("root dispatch resolver returned non-UTF-8 output")?;
    compose_root_prompt(&envelope_text, task_prompt)
}

fn compose_root_prompt(
    envelope_text: &str,
    task_prompt: &str,
) -> anyhow::Result<GovernedRootPrompt> {
    let envelope = parse_root_dispatch_envelope(envelope_text)?;
    if envelope.role != "orchestrator" {
        bail!("root dispatch resolver returned a non-orchestrator role");
    }
    if envelope.contract_sha256.len() != 64
        || !envelope
            .contract_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || envelope.skills.is_empty()
        || envelope.skills.iter().any(|skill| {
            skill.id.is_empty() || skill.codex_name.is_empty() || skill.canonical_path.is_empty()
        })
    {
        bail!("root dispatch resolver returned an invalid contract or skill set");
    }
    let receipt = format!(
        "SKILLS_LOADED role={} contract={} skills={}",
        envelope.role,
        envelope.contract_sha256,
        envelope
            .skills
            .iter()
            .map(|skill| skill.id.as_str())
            .collect::<Vec<_>>()
            .join(",")
    );
    let mut reads = vec![
        "AGENTS.md".to_owned(),
        "CLAUDE.md".to_owned(),
        "agents/roles/orchestrator.md".to_owned(),
        "agents/skill-routing.json".to_owned(),
    ];
    for skill in &envelope.skills {
        reads.push(format!(".agents/skills/{}/SKILL.md", skill.codex_name));
        reads.push(format!("{}/SKILL.md", skill.canonical_path));
    }
    let envelope_text = envelope_text.trim();
    let text = format!(
        "{envelope_text}\n\n# Supervisor-enforced root dispatch\n\
Before reading the coordinator workflow, epoch/queue, journal, goal bodies, or doing any task work, verify the unmodified envelope above. Read only AGENTS.md, tracked CLAUDE.md, agents/roles/orchestrator.md, agents/skill-routing.json, and every listed bridge and canonical SKILL.md; verify their hashes, then emit this exact standalone line:\n\n{receipt}\n\nThe supervisor rejects a completed turn unless that exact receipt appears in the structured event stream. After the receipt, follow the coordinator task below. Use only the prebuilt executables named by GOVFOLIO_AUTHORITY_BIN, GOVFOLIO_LOOP_BIN, GOVFOLIO_EPOCH_GATE_BIN, and GOVFOLIO_LEASE_BIN; do not rebuild or search for them.\n\n# Coordinator task\n\n{task_prompt}"
    );
    Ok(GovernedRootPrompt {
        text,
        receipt,
        reads,
    })
}

fn parse_root_dispatch_envelope(text: &str) -> anyhow::Result<RootDispatchEnvelope> {
    let trimmed = text.trim();
    let body = trimmed
        .strip_prefix(ROOT_ENVELOPE_BEGIN)
        .and_then(|value| {
            value
                .strip_prefix("\r\n")
                .or_else(|| value.strip_prefix('\n'))
        })
        .and_then(|value| value.strip_suffix(ROOT_ENVELOPE_END))
        .map(str::trim)
        .ok_or_else(|| anyhow!("root dispatch resolver returned malformed envelope markers"))?;
    serde_json::from_str(body).context("parse governed root dispatch envelope")
}

struct AttemptExecution {
    result: NormalizedResult,
    exemplar: Option<String>,
}

async fn reserve_attempt(
    context: &TickContext<'_>,
    provider_key: &str,
    preflight_signature: String,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<AttemptReservation> {
    let task_prompt = std::fs::read_to_string(&context.config.prompt_file)
        .with_context(|| format!("read prompt {}", context.config.prompt_file.display()))?;
    let root_prompt = governed_root_prompt(context.config, &task_prompt)?;
    let prompt = root_prompt.text;
    let head_before = git_text(&context.config.worktree, &["rev-parse", "HEAD"])?;
    let journal_before = file_sha(&context.config.worktree.join("agents").join("JOURNAL.md"))?;
    let work_key = work_key(&context.config.lane_id, &head_before, &prompt);
    if let Some(outcome) = suppress_previous_failure(context, provider_key, &work_key, now).await? {
        return Ok(AttemptReservation::Suppressed(outcome));
    }
    let attempt = AttemptSpec {
        id: Ulid::new().to_string(),
        lane_id: context.config.lane_id.clone(),
        lane_fence: context.lane.fence,
        work_key,
        worktree: context.config.worktree.clone(),
        expected_branch: context.config.expected_branch.clone(),
        prompt,
        required_root_receipt: Some(root_prompt.receipt),
        required_root_reads: root_prompt.reads,
        prompt_kind: PromptKind::Normal,
        provider: context.provider.clone(),
        resume_session_id: None,
        preflight_signature,
        git_head_before: head_before,
        journal_sha_before: journal_before,
    };
    match context.store.reserve_initial_attempt(&attempt, now).await {
        Ok(()) => Ok(AttemptReservation::Ready(Box::new(attempt))),
        Err(StoreError::AttemptBudgetExhausted(_)) => {
            context
                .store
                .record_suppression(
                    SuppressionReason::AttemptBudget,
                    provider_key,
                    None,
                    None,
                    now,
                )
                .await?;
            Ok(AttemptReservation::Suppressed(TickOutcome::Suppressed {
                reason: SuppressionReason::AttemptBudget,
                retry_at: None,
            }))
        }
        Err(error) => Err(error.into()),
    }
}

async fn suppress_previous_failure(
    context: &TickContext<'_>,
    provider_key: &str,
    work_key: &str,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<Option<TickOutcome>> {
    let Some(previous) =
        previous_failure_fingerprint(context.store, work_key, provider_key).await?
    else {
        return Ok(None);
    };
    let FingerprintGate::Open { retry_at } = context.store.fingerprint_gate(&previous, now).await?
    else {
        return Ok(None);
    };
    context
        .store
        .record_suppression(
            SuppressionReason::FailureFingerprint,
            provider_key,
            Some(&previous),
            Some(retry_at),
            now,
        )
        .await?;
    Ok(Some(TickOutcome::Suppressed {
        reason: SuppressionReason::FailureFingerprint,
        retry_at: Some(retry_at),
    }))
}

async fn execute_attempt(
    context: &TickContext<'_>,
    attempt: &AttemptSpec,
) -> anyhow::Result<AttemptExecution> {
    let attempt_artifacts = context
        .artifacts
        .begin_attempt(&attempt.id, AttemptArtifactPolicy::Persist)?
        .ok_or_else(|| anyhow!("persisted attempt unexpectedly suppressed"))?;
    context
        .artifacts
        .write_json(&attempt_artifacts.attempt_path(), attempt)?;
    context
        .store
        .start_attempt(context.lane, &attempt.id, Utc::now())
        .await?;
    let adapter = adapter_for(context.provider.provider);
    let inherited_environment = provider_runtime_environment(context.config);
    let command = adapter.build_fresh(attempt, &inherited_environment)?;
    let output = ProcessOutputPaths::from(&attempt_artifacts);
    let (cancel, cancellation) = cancellation_pair();
    let (pid_sender, mut pid_receiver) = oneshot::channel();
    let runner = ProcessRunner::default();
    let process = runner.run_with_pid(
        &command,
        &output,
        adapter.classifier(),
        cancellation,
        pid_sender,
    );
    tokio::pin!(process);
    let mut heartbeat = interval(HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut stopped = false;
    let mut child_pid = None;
    let mut pid_channel_open = true;
    let execution = loop {
        tokio::select! {
            result = &mut process => break result?,
            pid = &mut pid_receiver, if pid_channel_open => {
                pid_channel_open = false;
                if let Ok(pid) = pid {
                    child_pid = Some(pid);
                    renew_owners(context, child_pid).await?;
                }
            }
            _ = heartbeat.tick() => renew_owners(context, child_pid).await?,
            signal = tokio::signal::ctrl_c(), if !stopped => {
                signal.context("wait for operator stop")?;
                stopped = true;
                cancel.cancel();
            }
        }
    };
    renew_owners(context, None).await?;
    let mut result = execution.result;
    apply_root_receipt_postcondition(attempt, &output.events, &mut result)?;
    apply_postconditions(context.config, attempt, &mut result)?;
    context
        .artifacts
        .write_json(&attempt_artifacts.result_path(), &result)?;
    let exemplar = persist_failure_exemplar(context.artifacts, &output, &result)?;
    Ok(AttemptExecution { result, exemplar })
}

fn provider_runtime_environment(config: &LoopConfig) -> Vec<(String, String)> {
    let mut environment = std::env::vars().collect::<Vec<_>>();
    let loop_binary = config.authority_bin.with_file_name(if cfg!(windows) {
        "govfolio-loop.exe"
    } else {
        "govfolio-loop"
    });
    for (key, value) in [
        (
            "GOVFOLIO_AUTHORITY_BIN",
            config.authority_bin.to_string_lossy().into_owned(),
        ),
        (
            "GOVFOLIO_LOOP_BIN",
            loop_binary.to_string_lossy().into_owned(),
        ),
        (
            "GOVFOLIO_EPOCH_GATE_BIN",
            config.epoch_gate_bin.to_string_lossy().into_owned(),
        ),
        (
            "GOVFOLIO_LEASE_BIN",
            config.lease_bin.to_string_lossy().into_owned(),
        ),
        ("GOVFOLIO_EPOCH", config.epoch.clone()),
    ] {
        environment.retain(|(candidate, _)| !candidate.eq_ignore_ascii_case(key));
        environment.push((key.to_owned(), value));
    }
    environment
}

fn apply_root_receipt_postcondition(
    attempt: &AttemptSpec,
    events_path: &Path,
    result: &mut NormalizedResult,
) -> anyhow::Result<()> {
    let Some(expected) = attempt.required_root_receipt.as_deref() else {
        return Ok(());
    };
    if result.class != ResultClass::Completed {
        return Ok(());
    }
    if structured_root_receipt(
        events_path,
        attempt.provider.provider,
        expected,
        &attempt.required_root_reads,
    )? {
        return Ok(());
    }
    result.class = ResultClass::PostconditionFailed;
    result.stable_error_hash = Some(hex::encode(Sha256::digest(
        b"required governed root skill receipt missing, mismatched, or late",
    )));
    "completed provider turn violated the ordered governed root SKILLS_LOADED boundary"
        .clone_into(&mut result.summary);
    Ok(())
}

fn structured_root_receipt(
    path: &Path,
    provider: Provider,
    expected: &str,
    allowed_reads: &[String],
) -> io::Result<bool> {
    let mut scanned = 0_u64;
    for line in BufReader::new(File::open(path)?).split(b'\n') {
        let line = line?;
        scanned = scanned.saturating_add(u64::try_from(line.len()).unwrap_or(u64::MAX));
        if scanned > MAX_ROOT_RECEIPT_SCAN_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "root receipt event stream exceeded bounded scan",
            ));
        }
        if line.is_empty() {
            continue;
        }
        let value: Value = serde_json::from_slice(&line).map_err(io::Error::other)?;
        match root_receipt_event(provider, &value, expected, allowed_reads) {
            RootReceiptEvent::Found => return Ok(true),
            RootReceiptEvent::Allowed => {}
            RootReceiptEvent::ForbiddenTool => return Ok(false),
        }
    }
    Ok(false)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootReceiptEvent {
    Found,
    Allowed,
    ForbiddenTool,
}

fn root_receipt_event(
    provider: Provider,
    value: &Value,
    expected: &str,
    allowed_reads: &[String],
) -> RootReceiptEvent {
    match provider {
        Provider::Codex
            if value.get("type").and_then(Value::as_str) == Some("item.completed")
                && value.pointer("/item/type").and_then(Value::as_str) == Some("agent_message") =>
        {
            if value
                .pointer("/item/text")
                .and_then(Value::as_str)
                .is_some_and(|text| text.lines().any(|line| line == expected))
            {
                RootReceiptEvent::Found
            } else {
                RootReceiptEvent::Allowed
            }
        }
        Provider::Codex
            if matches!(
                value.get("type").and_then(Value::as_str),
                Some("item.started" | "item.completed")
            ) =>
        {
            match value.pointer("/item/type").and_then(Value::as_str) {
                Some("agent_message" | "reasoning") => RootReceiptEvent::Allowed,
                Some("command_execution") => {
                    classify_pre_receipt_tool(value.pointer("/item/command"), false, allowed_reads)
                }
                Some("mcp_tool_call") => classify_pre_receipt_tool(
                    value.pointer("/item/arguments"),
                    false,
                    allowed_reads,
                ),
                _ => RootReceiptEvent::ForbiddenTool,
            }
        }
        Provider::Claude if value.get("type").and_then(Value::as_str) == Some("assistant") => {
            let Some(content) = value.pointer("/message/content").and_then(Value::as_array) else {
                return RootReceiptEvent::Allowed;
            };
            for item in content {
                match item.get("type").and_then(Value::as_str) {
                    Some("text")
                        if item
                            .get("text")
                            .and_then(Value::as_str)
                            .is_some_and(|text| text.lines().any(|line| line == expected)) =>
                    {
                        return RootReceiptEvent::Found;
                    }
                    Some("tool_use") => {
                        let classification = classify_pre_receipt_tool(
                            item.get("input"),
                            item.get("name").and_then(Value::as_str) == Some("Read"),
                            allowed_reads,
                        );
                        if classification == RootReceiptEvent::ForbiddenTool {
                            return classification;
                        }
                    }
                    _ => {}
                }
            }
            RootReceiptEvent::Allowed
        }
        _ => RootReceiptEvent::Allowed,
    }
}

fn classify_pre_receipt_tool(
    input: Option<&Value>,
    dedicated_read_tool: bool,
    allowed_reads: &[String],
) -> RootReceiptEvent {
    let mut strings = Vec::new();
    if let Some(input) = input {
        collect_json_strings(input, &mut strings);
    }
    let normalized_text = strings
        .iter()
        .map(|value| value.replace('\\', "/").to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    let forbidden_action = [
        "appendfile",
        "apply_patch",
        "cargo ",
        "cargo.exe",
        "git add",
        "git commit",
        "govfolio-loop",
        "jurisdiction-lease",
        "pnpm ",
        "remove-item",
        "set-content",
        "write_file",
        "writefile",
    ]
    .iter()
    .any(|forbidden| normalized_text.contains(forbidden));
    let recognized_read = dedicated_read_tool
        || [
            "get-content",
            "get-filehash",
            "hash-object",
            "cat-file",
            "git cat-file",
            "git hash-object",
            "git ls-files",
            "git ls-tree",
            "git show",
            "ls-files",
            "ls-tree",
            "read_file",
            "readfile",
            "sha256",
            "type ",
        ]
        .iter()
        .any(|operation| normalized_text.contains(operation));
    let referenced_paths = strings
        .iter()
        .flat_map(|value| referenced_repo_paths(value))
        .collect::<Vec<_>>();
    let normalized_allowed = allowed_reads
        .iter()
        .map(|value| normalize_repo_path(value))
        .collect::<Vec<_>>();
    let exact_governed_paths = !referenced_paths.is_empty()
        && referenced_paths
            .iter()
            .all(|path| normalized_allowed.contains(path));
    if recognized_read && exact_governed_paths && !forbidden_action {
        RootReceiptEvent::Allowed
    } else {
        RootReceiptEvent::ForbiddenTool
    }
}

fn referenced_repo_paths(value: &str) -> Vec<String> {
    let normalized = value.replace('\\', "/").to_ascii_lowercase();
    let mut paths = Vec::new();
    for marker in [".agents/", "agents/", "claude.md", "agents.md"] {
        let mut offset = 0;
        while let Some(relative) = normalized[offset..].find(marker) {
            let start = offset + relative;
            if marker == "agents/"
                && start > 0
                && normalized.as_bytes().get(start - 1) == Some(&b'.')
            {
                offset = start + marker.len();
                continue;
            }
            let end = normalized[start..]
                .find(|character: char| !is_repo_path_character(character))
                .map_or(normalized.len(), |length| start + length);
            paths.push(normalize_repo_path(&normalized[start..end]));
            offset = end.max(start + marker.len());
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn normalize_repo_path(value: &str) -> String {
    value
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_ascii_lowercase()
}

fn is_repo_path_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '/' | '.' | '_' | '-' | '@')
}

fn collect_json_strings<'a>(value: &'a Value, output: &mut Vec<&'a str>) {
    match value {
        Value::String(text) => output.push(text),
        Value::Array(values) => {
            for value in values {
                collect_json_strings(value, output);
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                collect_json_strings(value, output);
            }
        }
        _ => {}
    }
}

async fn renew_owners(context: &TickContext<'_>, pid: Option<u32>) -> anyhow::Result<()> {
    context
        .store
        .renew_supervisor(
            context.supervisor,
            std::process::id(),
            Utc::now(),
            OWNER_TTL,
        )
        .await?;
    context
        .store
        .renew_lane(context.lane, pid, Utc::now(), OWNER_TTL)
        .await?;
    Ok(())
}

fn persist_failure_exemplar(
    artifacts: &ArtifactStore,
    output: &ProcessOutputPaths,
    result: &NormalizedResult,
) -> anyhow::Result<Option<String>> {
    if !result.class.is_failure() {
        return Ok(None);
    }
    let mut evidence = std::fs::read(&output.events).unwrap_or_default();
    evidence.extend_from_slice(&std::fs::read(&output.stderr).unwrap_or_default());
    if evidence.is_empty() {
        Ok(None)
    } else {
        Ok(Some(artifacts.write_gzip_blob(&evidence)?.sha256))
    }
}

async fn finalize_attempt(
    context: &TickContext<'_>,
    provider_key: &str,
    attempt: AttemptSpec,
    execution: AttemptExecution,
) -> anyhow::Result<TickOutcome> {
    let result = execution.result;
    if matches!(
        result.class,
        ResultClass::Completed | ResultClass::OperatorStop
    ) {
        context
            .store
            .finish_attempt(context.lane, &attempt.id, &result, Utc::now())
            .await?;
        if result.class == ResultClass::Completed {
            context
                .store
                .close_provider_circuit(provider_key, &context.provider.config_fingerprint)
                .await?;
            return Ok(TickOutcome::Completed(attempt.id));
        }
        return Ok(TickOutcome::Failed {
            attempt_id: attempt.id,
            class: result.class,
        });
    }
    let fingerprint = failure_fingerprint(&attempt, &result);
    let decision = PolicyEngine::new(SystemClock).decide(&result, 1, &fingerprint);
    context
        .store
        .record_failure(FailureObservation {
            attempt_id: &attempt.id,
            provider_key,
            config_fingerprint: &context.provider.config_fingerprint,
            fingerprint: &fingerprint,
            exemplar_ref: execution.exemplar.as_deref(),
            result: &result,
            decision: &decision,
            occurred_at: Utc::now(),
        })
        .await?;
    if matches!(
        decision.action,
        RetryAction::Recover | RetryAction::Reconcile
    ) {
        context
            .store
            .mark_lane_recovery_required(context.lane, result.class.as_str(), Utc::now())
            .await?;
    }
    match decision.action {
        RetryAction::Recover | RetryAction::Reconcile => Ok(TickOutcome::RecoveryRequired {
            lane_id: context.config.lane_id.clone(),
            reason: result.summary,
        }),
        _ => Ok(TickOutcome::Failed {
            attempt_id: attempt.id,
            class: result.class,
        }),
    }
}

async fn gate_system(
    store: &ControlStore,
    provider_key: &str,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<Option<TickOutcome>> {
    match store.system_gate(now).await? {
        SystemGate::Closed => Ok(None),
        SystemGate::Paused { retry_at, .. } => {
            store
                .record_suppression(
                    SuppressionReason::SystemPause,
                    provider_key,
                    None,
                    Some(retry_at),
                    now,
                )
                .await?;
            Ok(Some(TickOutcome::Suppressed {
                reason: SuppressionReason::SystemPause,
                retry_at: Some(retry_at),
            }))
        }
    }
}

async fn gate_provider(
    store: &ControlStore,
    provider: &ProviderIdentity,
    provider_key: &str,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<Option<TickOutcome>> {
    let gate = store
        .provider_gate(provider_key, &provider.config_fingerprint, now)
        .await?;
    let retry_at = match gate {
        ProviderGate::Closed => return Ok(None),
        ProviderGate::HalfOpenAvailable => {
            if store
                .try_acquire_half_open(
                    provider_key,
                    &format!("probe-{}", std::process::id()),
                    &provider.config_fingerprint,
                    now,
                    HALF_OPEN_TTL,
                )
                .await?
            {
                return Ok(None);
            }
            None
        }
        ProviderGate::Open { retry_at } => retry_at,
        ProviderGate::HalfOpenOwned { until } => Some(until),
        ProviderGate::DisabledUntilFingerprintChanges => None,
    };
    store
        .record_suppression(
            SuppressionReason::ProviderCircuit,
            provider_key,
            None,
            retry_at,
            now,
        )
        .await?;
    Ok(Some(TickOutcome::Suppressed {
        reason: SuppressionReason::ProviderCircuit,
        retry_at,
    }))
}

async fn suppress_preflight(
    store: &ControlStore,
    lane: &LaneFence,
    provider_key: &str,
    report: &PreflightReport,
    now: chrono::DateTime<Utc>,
) -> anyhow::Result<Option<TickOutcome>> {
    let Some(outcome) = report.terminal_outcome() else {
        return Ok(None);
    };
    let (reason, retry_at) = match outcome {
        ProbeOutcome::Pass { .. } => return Ok(None),
        ProbeOutcome::Wait { retry_at, .. } => (SuppressionReason::PreflightWait, Some(*retry_at)),
        ProbeOutcome::Recover { reason } => {
            store.mark_lane_recovery_required(lane, reason, now).await?;
            return Ok(Some(TickOutcome::RecoveryRequired {
                lane_id: lane.lane_id.clone(),
                reason: reason.clone(),
            }));
        }
        ProbeOutcome::Block { .. } => (SuppressionReason::PreflightWait, None),
    };
    store
        .record_suppression(reason.clone(), provider_key, None, retry_at, now)
        .await?;
    Ok(Some(TickOutcome::Suppressed { reason, retry_at }))
}

fn preflight_suite(config: &LoopConfig, provider: &ProviderIdentity) -> PreflightSuite {
    let mut probes: Vec<Arc<dyn Probe>> = vec![
        Arc::new(GitProbe {
            worktree: config.worktree.clone(),
            expected_branch: config.expected_branch.clone(),
            allow_dirty: false,
        }),
        Arc::new(AuthorityProbe {
            binary: config.authority_bin.clone(),
            repo: config.worktree.clone(),
        }),
        Arc::new(SkillContractProbe {
            node: PathBuf::from("node"),
            worktree: config.worktree.clone(),
        }),
        Arc::new(ProviderCliProbe {
            identity: provider.clone(),
            worktree: config.worktree.clone(),
        }),
        Arc::new(CompilerProbe {
            rustc: PathBuf::from("rustc"),
            cache_dir: compiler_cache_dir(config),
        }),
        Arc::new(RuntimeSeparationProbe {
            bronze_root: config.bronze_root.clone(),
            protected_paths: vec![
                config.worktree.clone(),
                config.authority_bin.clone(),
                config.epoch_gate_bin.clone(),
                config.lease_bin.clone(),
            ],
        }),
        Arc::new(DataProbe {
            database_url: config.database_url.clone(),
            bronze_root: config.bronze_root.clone(),
        }),
    ];
    if config.role == "factory" {
        probes.push(Arc::new(FactoryProbe {
            epoch_gate: config.epoch_gate_bin.clone(),
            lease_bin: config.lease_bin.clone(),
            worktree: config.worktree.clone(),
            epoch: config.epoch.clone(),
            lane_id: config.lane_id.clone(),
        }));
    }
    probes.push(Arc::new(DiskProbe {
        runtime_root: config.paths.root.clone(),
    }));
    PreflightSuite::new(probes)
}

fn compiler_cache_dir(config: &LoopConfig) -> PathBuf {
    config
        .paths
        .root
        .join("canaries")
        .join("rustc")
        .join(hex::encode(Sha256::digest(config.lane_id.as_bytes())))
}

fn provider_identity(config: &LoopConfig) -> anyhow::Result<ProviderIdentity> {
    let output = Command::new(&config.provider_executable)
        .arg("--version")
        .current_dir(&config.worktree)
        .output()
        .with_context(|| format!("probe {} version", config.provider))?;
    if !output.status.success() {
        bail!(
            "{} --version failed: {}",
            config.provider,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let version = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if version.is_empty() {
        bail!("{} --version returned empty output", config.provider);
    }
    let mut hasher = Sha256::new();
    hasher.update(config.provider.as_str().as_bytes());
    hasher.update(
        config
            .provider_executable
            .as_os_str()
            .to_string_lossy()
            .as_bytes(),
    );
    hasher.update(version.as_bytes());
    hasher.update(config.model.as_deref().unwrap_or_default().as_bytes());
    Ok(ProviderIdentity {
        provider: config.provider,
        executable: config.provider_executable.clone(),
        cli_version: version,
        model: config.model.clone(),
        config_fingerprint: hex::encode(hasher.finalize()),
    })
}

async fn runtime_provider_identity(config: &LoopConfig) -> anyhow::Result<ProviderIdentity> {
    if config.provider != Provider::Codex {
        return provider_identity(config);
    }
    let resolver =
        NativeCodexResolver::new(SystemNativeExecutableProbe::new(StdDuration::from_secs(15)));
    let native = resolver
        .resolve(&NativeResolverInputs::from_environment()?)
        .await?;
    let mut native_config = config.clone();
    native_config.provider_executable.clone_from(&native.path);
    let base = provider_identity(&native_config)?;
    Ok(native.provider_identity(config.model.clone(), &base.config_fingerprint))
}

fn adapter_for(provider: Provider) -> Box<dyn ProviderAdapter> {
    match provider {
        Provider::Claude => Box::new(ClaudeAdapter::default()),
        Provider::Codex => Box::new(CodexAdapter),
    }
}

fn provider_key(identity: &ProviderIdentity) -> String {
    identity.model.as_ref().map_or_else(
        || identity.provider.to_string(),
        |model| format!("{}/{model}", identity.provider),
    )
}

fn work_key(lane_id: &str, head: &str, prompt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(lane_id.as_bytes());
    hasher.update(head.as_bytes());
    hasher.update(prompt.as_bytes());
    hex::encode(hasher.finalize())
}

fn failure_fingerprint(attempt: &AttemptSpec, result: &NormalizedResult) -> String {
    let mut hasher = Sha256::new();
    hasher.update(attempt.provider.provider.as_str().as_bytes());
    hasher.update(
        attempt
            .provider
            .model
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(attempt.provider.cli_version.as_bytes());
    hasher.update(result.class.as_str().as_bytes());
    hasher.update(
        result
            .stable_error_hash
            .as_deref()
            .unwrap_or_default()
            .as_bytes(),
    );
    hasher.update(attempt.worktree.as_os_str().to_string_lossy().as_bytes());
    hasher.update(attempt.preflight_signature.as_bytes());
    hex::encode(hasher.finalize())
}

async fn previous_failure_fingerprint(
    store: &ControlStore,
    work_key: &str,
    provider_key: &str,
) -> anyhow::Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT failure_fingerprint FROM attempt WHERE work_key = ?1 AND provider_key = ?2 \
         AND failure_fingerprint IS NOT NULL ORDER BY attempt_ordinal DESC LIMIT 1",
    )
    .bind(work_key)
    .bind(provider_key)
    .fetch_optional(store.pool())
    .await?)
}

fn apply_postconditions(
    config: &LoopConfig,
    attempt: &AttemptSpec,
    result: &mut NormalizedResult,
) -> anyhow::Result<()> {
    let head_after = git_text(&config.worktree, &["rev-parse", "HEAD"])?;
    let journal_after = file_sha(&config.worktree.join("agents").join("JOURNAL.md"))?;
    let status = git_text(
        &config.worktree,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let journal_changed = journal_after != attempt.journal_sha_before;
    let failed_tool_changed_git = result.class != ResultClass::Completed
        && (head_after != attempt.git_head_before || !status.is_empty());
    let successful_tool_left_dirty = result.class == ResultClass::Completed && !status.is_empty();
    if journal_changed || failed_tool_changed_git || successful_tool_left_dirty {
        result.class = ResultClass::PostconditionFailed;
        result.stable_error_hash = Some(hex::encode(Sha256::digest(
            b"repository postcondition failed",
        )));
        "provider left an unauthorized JOURNAL edit or dirty/interrupted Git state"
            .clone_into(&mut result.summary);
    }
    Ok(())
}

fn git_text(worktree: &Path, args: &[&str]) -> anyhow::Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(worktree)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn file_sha(path: &Path) -> anyhow::Result<String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(hex::encode(Sha256::digest(bytes))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("missing".to_owned()),
        Err(error) => Err(error).with_context(|| format!("read {}", path.display())),
    }
}

fn append_event<T: Serialize>(
    artifacts: &ArtifactStore,
    run_id: &str,
    event: &T,
) -> anyhow::Result<()> {
    let mut bytes = serde_json::to_vec(event)?;
    bytes.push(b'\n');
    artifacts.append_supervisor_event(run_id, &bytes)?;
    Ok(())
}

fn render_outcome(outcome: &TickOutcome) {
    match outcome {
        TickOutcome::Completed(attempt) => println!("completed attempt {attempt}"),
        TickOutcome::Failed { attempt_id, class } => {
            println!("attempt {attempt_id}: {class}");
        }
        TickOutcome::Suppressed { reason, retry_at } => {
            println!("suppressed: {reason:?} retry_at={retry_at:?}");
        }
        TickOutcome::RecoveryRequired { lane_id, reason } => {
            println!("lane {lane_id} recovery_required: {reason}");
        }
    }
}

async fn probe_native_codex() -> anyhow::Result<()> {
    let config = LoopConfig::from_env()?;
    config.paths.ensure()?;
    let store = ControlStore::open_writer(&config.paths.control_db).await?;
    let resolver =
        NativeCodexResolver::new(SystemNativeExecutableProbe::new(StdDuration::from_secs(15)));
    let inputs = NativeResolverInputs::from_environment()?;
    let identity = match resolver.resolve(&inputs).await {
        Ok(identity) => identity,
        Err(error) => {
            if let Some(proof) = error.unsupported_proof(Utc::now()) {
                let path = persist_native_unsupported(&config, &proof)?;
                bail!(
                    "native Codex is specifically unsupported; proof={} error={error}",
                    path.display()
                );
            }
            return Err(error.into());
        }
    };
    persist_native_identity(&store, &identity, Utc::now(), Duration::days(7)).await?;
    let runner = SystemHostCommandRunner::new(StdDuration::from_mins(1));
    let request = NativeSmokeRequest {
        repo: config.repo.clone(),
        scratch_root: config.paths.root.join("native-smoke"),
        git_executable: PathBuf::from("git"),
        rustc_executable: PathBuf::from("rustc"),
        codex: identity.clone(),
    };
    let report = match run_native_smoke(&runner, &request).await {
        Ok(report) => report,
        Err(error) => {
            if let Some(proof) = error.unsupported_proof(&identity, Utc::now()) {
                let path = persist_native_unsupported(&config, &proof)?;
                bail!(
                    "native Codex smoke is specifically unsupported; proof={} error={error}",
                    path.display()
                );
            }
            return Err(error.into());
        }
    };
    println!("{}", serde_json::to_string_pretty(&identity)?);
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn persist_native_unsupported(
    config: &LoopConfig,
    proof: &crate::host::NativeUnsupportedProof,
) -> anyhow::Result<PathBuf> {
    let directory = config.paths.root.join("native-unsupported");
    std::fs::create_dir_all(&directory)?;
    let path = directory.join(format!("{}.json", Ulid::new()));
    atomic_write_new(&path, &serde_json::to_vec(proof)?)?;
    Ok(path)
}

async fn compatibility_canary(provider_text: &str, skill_text: &str) -> anyhow::Result<()> {
    let provider: Provider = provider_text.parse().map_err(anyhow::Error::msg)?;
    let mut config = LoopConfig::from_env()?;
    config.paths.ensure()?;
    config.provider = provider;
    config.model = match provider {
        Provider::Codex => std::env::var("GOVFOLIO_CODEX_MODEL")
            .ok()
            .or(config.model.clone()),
        Provider::Claude => std::env::var("GOVFOLIO_CLAUDE_MODEL").ok(),
    };
    if config.model.as_deref().is_none_or(str::is_empty) {
        bail!("compatibility canary requires an explicit model for {provider}");
    }
    config.provider_executable = match provider {
        Provider::Codex => {
            let resolver = NativeCodexResolver::new(SystemNativeExecutableProbe::new(
                StdDuration::from_secs(15),
            ));
            let identity = resolver
                .resolve(&NativeResolverInputs::from_environment()?)
                .await?;
            let store = ControlStore::open_writer(&config.paths.control_db).await?;
            persist_native_identity(&store, &identity, Utc::now(), Duration::days(7)).await?;
            drop(store);
            run_native_smoke(
                &SystemHostCommandRunner::new(StdDuration::from_mins(1)),
                &NativeSmokeRequest {
                    repo: config.repo.clone(),
                    scratch_root: config.paths.root.join("native-smoke"),
                    git_executable: PathBuf::from("git"),
                    rustc_executable: PathBuf::from("rustc"),
                    codex: identity.clone(),
                },
            )
            .await?;
            identity.path
        }
        Provider::Claude => std::env::var_os("GOVFOLIO_CLAUDE_BIN")
            .map_or_else(|| PathBuf::from("claude"), PathBuf::from),
    };

    let canary_root = config.paths.root.join("canary-worktrees");
    std::fs::create_dir_all(&canary_root)?;
    let worktree = canary_root.join(Ulid::new().to_string());
    let main_sha = fetch_origin_main(&config.repo)?;
    add_disposable_worktree(&config.repo, &worktree, &main_sha)?;
    let result =
        run_compatibility_canary(&config, provider, skill_text, &worktree, &main_sha).await;
    let cleanup = remove_disposable_worktree(&config.repo, &canary_root, &worktree);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("canary cleanup also failed: {cleanup:#}")))
        }
    }
}

async fn run_compatibility_canary(
    config: &LoopConfig,
    provider: Provider,
    skill_text: &str,
    worktree: &Path,
    main_sha: &str,
) -> anyhow::Result<()> {
    let contract = SkillContractProbe {
        node: PathBuf::from("node"),
        worktree: worktree.to_path_buf(),
    }
    .check(Utc::now())
    .await;
    if !contract.is_pass() {
        bail!(
            "compatibility canary blocked by project skill contract: {}",
            serde_json::to_string(&contract)?
        );
    }
    let mut identity_config = config.clone();
    identity_config.worktree = worktree.to_path_buf();
    let mut identity = provider_identity(&identity_config)?;
    if provider == Provider::Codex {
        let resolver =
            NativeCodexResolver::new(SystemNativeExecutableProbe::new(StdDuration::from_secs(15)));
        let native = resolver
            .resolve(&NativeResolverInputs::from_environment()?)
            .await?;
        identity = native.provider_identity(config.model.clone(), &identity.config_fingerprint);
    }
    let skill_relative = PathBuf::from(skill_text);
    let skill_bytes = std::fs::read(worktree.join(&skill_relative))
        .with_context(|| format!("read canary skill {skill_text}"))?;
    let challenge = Ulid::new().to_string();
    let skill = SkillCanarySpec::new(
        skill_text,
        skill_relative,
        hex::encode(Sha256::digest(skill_bytes)),
        &challenge,
        PathBuf::from(".govfolio-loop").join(format!("skill-{challenge}.json")),
    )?;
    let attempt = AttemptSpec {
        id: Ulid::new().to_string(),
        lane_id: format!("canary-{provider}"),
        lane_fence: 1,
        work_key: format!("compatibility-{provider}-{}", identity.config_fingerprint),
        worktree: worktree.to_path_buf(),
        expected_branch: "detached-canary".to_owned(),
        prompt: String::new(),
        required_root_receipt: None,
        required_root_reads: Vec::new(),
        prompt_kind: PromptKind::CompatibilityCanary,
        provider: identity.clone(),
        resume_session_id: None,
        preflight_signature: main_sha.to_owned(),
        git_head_before: main_sha.to_owned(),
        journal_sha_before: file_sha(&worktree.join("agents").join("JOURNAL.md"))?,
    };
    let request = CanaryRequest {
        attempt,
        provider_key: provider_key(&identity),
        inherited_env: std::env::vars().collect(),
        valid_for: Duration::days(7),
        skill,
    };
    let store = ControlStore::open_writer(&config.paths.control_db).await?;
    let artifacts = ArtifactStore::new(&config.paths.root, ArtifactPolicy::default());
    let canary = CompatibilityCanary::new(&store, &artifacts);
    let invoker = ProcessCanaryInvoker::new(&config.paths.root, StdDuration::from_mins(10));
    let adapter = adapter_for(provider);
    match canary
        .prove(adapter.as_ref(), &invoker, &request, Utc::now())
        .await?
    {
        CanaryOutcome::Proven { proof_ref, cached } => {
            println!("provider={provider} compatibility=proven cached={cached} proof={proof_ref}");
            Ok(())
        }
        CanaryOutcome::Rejected { proof_ref, reason } => {
            bail!("provider={provider} compatibility=rejected proof={proof_ref} reason={reason}")
        }
    }
}

fn fetch_origin_main(repo: &Path) -> anyhow::Result<String> {
    git_checked(repo, &["fetch", "origin", "main"])?;
    git_text(repo, &["rev-parse", "origin/main"])
}

fn add_disposable_worktree(repo: &Path, worktree: &Path, main_sha: &str) -> anyhow::Result<()> {
    let path = worktree.to_string_lossy().into_owned();
    git_checked(repo, &["worktree", "add", "--detach", &path, main_sha])?;
    Ok(())
}

fn remove_disposable_worktree(repo: &Path, parent: &Path, worktree: &Path) -> anyhow::Result<()> {
    if worktree.parent() != Some(parent) || !worktree.starts_with(parent) {
        bail!("refusing to remove an unvalidated canary worktree");
    }
    let path = worktree.to_string_lossy().into_owned();
    git_checked(repo, &["worktree", "remove", "--force", &path])?;
    Ok(())
}

fn git_checked(worktree: &Path, args: &[&str]) -> anyhow::Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(worktree)
        .output()
        .with_context(|| format!("run git {}", args.join(" ")))?;
    if output.status.success() {
        Ok(())
    } else {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
    }
}

async fn doctor() -> anyhow::Result<()> {
    let config = LoopConfig::from_env()?;
    config.paths.ensure()?;
    let identity = runtime_provider_identity(&config).await?;
    let report = preflight_suite(&config, &identity).run(Utc::now()).await;
    println!("{}", serde_json::to_string_pretty(&report)?);
    if report.all_pass() {
        Ok(())
    } else {
        bail!("zero-spend preflight did not pass")
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "the read-only monitor renders one bounded snapshot of every control plane"
)]
async fn status() -> anyhow::Result<()> {
    let config = LoopConfig::from_env()?;
    let store = ControlStore::open_monitor(&config.paths.control_db).await?;
    let supervisor = sqlx::query(
        "SELECT owner_id, fence, status, pid, heartbeat_at_ms, lease_until_ms \
         FROM supervisor_lease WHERE singleton = 1",
    )
    .fetch_optional(store.pool())
    .await?;
    if let Some(row) = supervisor {
        println!(
            "supervisor owner={} fence={} status={} pid={:?} heartbeat_ms={} lease_until_ms={}",
            row.try_get::<String, _>(0)?,
            row.try_get::<i64, _>(1)?,
            row.try_get::<String, _>(2)?,
            row.try_get::<Option<i64>, _>(3)?,
            row.try_get::<i64, _>(4)?,
            row.try_get::<i64, _>(5)?,
        );
    } else {
        println!("supervisor: never started");
    }
    let lanes = sqlx::query(
        "SELECT lane_id, fence, status, role, worktree, expected_branch, provider_key, pid, \
         heartbeat_at_ms, lease_until_ms, recovery_reason FROM lane_lease ORDER BY lane_id",
    )
    .fetch_all(store.pool())
    .await?;
    for row in lanes {
        let lane_id = row.try_get::<String, _>(0)?;
        let worktree = row.try_get::<Option<String>, _>(4)?;
        println!(
            "lane={} fence={} status={} role={:?} worktree={:?} branch={:?} provider={:?} pid={:?} heartbeat_ms={} lease_until_ms={} recovery={:?}",
            lane_id,
            row.try_get::<i64, _>(1)?,
            row.try_get::<String, _>(2)?,
            row.try_get::<Option<String>, _>(3)?,
            worktree,
            row.try_get::<Option<String>, _>(5)?,
            row.try_get::<Option<String>, _>(6)?,
            row.try_get::<Option<i64>, _>(7)?,
            row.try_get::<i64, _>(8)?,
            row.try_get::<i64, _>(9)?,
            row.try_get::<Option<String>, _>(10)?,
        );
        if let Some(worktree) = worktree {
            println!(
                "lane_git={} {}",
                lane_id,
                lane_git_status(Path::new(&worktree))
            );
        }
    }
    let attempts = store.attempt_count().await?;
    let suppressions: i64 =
        sqlx::query_scalar("SELECT COALESCE(SUM(count), 0) FROM suppression_counter")
            .fetch_one(store.pool())
            .await?;
    println!("attempts={attempts} suppressions={suppressions}");
    let suppression_rows = sqlx::query(
        "SELECT reason, provider_key, count, retry_at_ms, last_seen_at_ms \
         FROM suppression_counter ORDER BY last_seen_at_ms DESC LIMIT 20",
    )
    .fetch_all(store.pool())
    .await?;
    for row in suppression_rows {
        println!(
            "suppression={} provider={} count={} retry_ms={:?} last_ms={}",
            row.try_get::<String, _>(0)?,
            row.try_get::<String, _>(1)?,
            row.try_get::<i64, _>(2)?,
            row.try_get::<Option<i64>, _>(3)?,
            row.try_get::<i64, _>(4)?,
        );
    }
    let circuits = sqlx::query(
        "SELECT provider_key, state, reason, retry_at_ms, consecutive_failures \
         FROM provider_circuit ORDER BY provider_key",
    )
    .fetch_all(store.pool())
    .await?;
    for row in circuits {
        println!(
            "provider={} circuit={} reason={:?} retry_ms={:?} failures={}",
            row.try_get::<String, _>(0)?,
            row.try_get::<String, _>(1)?,
            row.try_get::<Option<String>, _>(2)?,
            row.try_get::<Option<i64>, _>(3)?,
            row.try_get::<i64, _>(4)?,
        );
    }
    if let Some(row) = sqlx::query(
        "SELECT state, reason, retry_at_ms, diagnostics_passed_at_ms \
         FROM system_circuit WHERE singleton = 1",
    )
    .fetch_optional(store.pool())
    .await?
    {
        println!(
            "system_circuit={} reason={:?} retry_ms={:?} diagnostics_ms={:?}",
            row.try_get::<String, _>(0)?,
            row.try_get::<Option<String>, _>(1)?,
            row.try_get::<Option<i64>, _>(2)?,
            row.try_get::<Option<i64>, _>(3)?,
        );
    }
    for receipt in store.receipt_mirrors().await? {
        println!(
            "receipt={} state={} pr={:?} branch={:?} candidate={:?} merge={:?} error={:?} updated={}",
            receipt.receipt_id,
            receipt.state,
            receipt.pull_request,
            receipt.branch,
            receipt.candidate_sha,
            receipt.merge_sha,
            receipt.last_error,
            receipt.updated_at,
        );
    }
    render_domain_status(&config).await;
    Ok(())
}

fn lane_git_status(worktree: &Path) -> String {
    let head = git_text(worktree, &["rev-parse", "--short=12", "HEAD"])
        .unwrap_or_else(|error| format!("error={}", bounded_error(&error)));
    let dirty = git_text(
        worktree,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )
    .map_or_else(
        |error| format!("error={}", bounded_error(&error)),
        |status| status.lines().count().to_string(),
    );
    let divergence = git_text(
        worktree,
        &["rev-list", "--left-right", "--count", "origin/main...HEAD"],
    )
    .unwrap_or_else(|error| format!("error={}", bounded_error(&error)));
    format!("head={head} dirty_entries={dirty} origin_main_left_right={divergence}")
}

async fn render_domain_status(config: &LoopConfig) {
    let pool = match PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(StdDuration::from_secs(3))
        .connect(&config.database_url)
        .await
    {
        Ok(pool) => pool,
        Err(_error) => {
            println!("domain_monitor=unavailable reason=database_connect_failed");
            return;
        }
    };
    let rows = sqlx::query_as::<_, DomainLeaseRow>(
        "SELECT id, coverage_phase, claimed_by, claimed_at, lease_generation, \
         pending_integration_id FROM jurisdiction WHERE claimed_by IS NOT NULL \
         ORDER BY claimed_at, id",
    )
    .fetch_all(&pool)
    .await;
    pool.close().await;
    match rows {
        Ok(rows) if rows.is_empty() => println!("domain_leases=none"),
        Ok(rows) => {
            let now = Utc::now();
            for (id, phase, owner, claimed_at, generation, pending) in rows {
                println!(
                    "domain_lease={} owner={} phase={} generation={} pending={:?} age_min={}",
                    id,
                    owner,
                    phase,
                    generation,
                    pending,
                    (now - claimed_at).num_minutes(),
                );
            }
        }
        Err(_error) => println!("domain_monitor=unavailable reason=database_query_failed"),
    }
}

fn bounded_error(error: &dyn std::fmt::Display) -> String {
    error.to_string().chars().take(256).collect()
}

async fn backup() -> anyhow::Result<()> {
    let config = LoopConfig::from_env()?;
    config.paths.ensure()?;
    let store = ControlStore::open_writer(&config.paths.control_db).await?;
    let destination = store
        .backup_if_due(&config.paths.backups, Utc::now(), Duration::zero())
        .await?
        .ok_or_else(|| anyhow!("backup unexpectedly skipped"))?;
    println!("{}", destination.display());
    Ok(())
}

async fn recover_lane(lane_id: &str) -> anyhow::Result<()> {
    let mut config = LoopConfig::from_env()?;
    config.paths.ensure()?;
    let store = ControlStore::open_writer(&config.paths.control_db).await?;
    let row = sqlx::query(
        "SELECT status, worktree, expected_branch, pid FROM lane_lease WHERE lane_id = ?1",
    )
    .bind(lane_id)
    .fetch_optional(store.pool())
    .await?
    .ok_or_else(|| anyhow!("lane {lane_id:?} has no persisted state"))?;
    let status = row.try_get::<String, _>(0)?;
    let worktree = row
        .try_get::<Option<String>, _>(1)?
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("lane {lane_id:?} has no stored worktree"))?;
    let branch = row
        .try_get::<Option<String>, _>(2)?
        .ok_or_else(|| anyhow!("lane {lane_id:?} has no stored branch"))?;
    let pid = row.try_get::<Option<i64>, _>(3)?;
    if status != "recovery_required" || pid.is_some() {
        bail!(
            "lane {lane_id:?} is not an inactive recovery_required lane (status={status}, pid={pid:?})"
        );
    }
    let git = GitProbe {
        worktree: worktree.clone(),
        expected_branch: branch.clone(),
        allow_dirty: false,
    }
    .check(Utc::now())
    .await;
    if !git.is_pass() {
        bail!("lane {lane_id:?} Git recovery is incomplete: {git:?}");
    }
    config.worktree = worktree;
    config.expected_branch = branch;
    let authority = AuthorityProbe {
        binary: config.authority_bin.clone(),
        repo: config.worktree.clone(),
    }
    .check(Utc::now())
    .await;
    if !authority.is_pass() {
        bail!("lane {lane_id:?} authority recovery is incomplete: {authority:?}");
    }
    let skill_contract = SkillContractProbe {
        node: PathBuf::from("node"),
        worktree: config.worktree.clone(),
    }
    .check(Utc::now())
    .await;
    if !skill_contract.is_pass() {
        bail!("lane {lane_id:?} skill recovery is incomplete: {skill_contract:?}");
    }
    let owner = format!("recovery-{}-{}", std::process::id(), Ulid::new());
    let supervisor = store
        .acquire_supervisor(&owner, Utc::now(), OWNER_TTL)
        .await?;
    store
        .resolve_lane_recovery(&supervisor, lane_id, Utc::now())
        .await?;
    store.release_supervisor(&supervisor, Utc::now()).await?;
    println!("lane={lane_id} recovery=cleared next_start_requires_new_fence");
    Ok(())
}

async fn submit_receipt(path: &str) -> anyhow::Result<()> {
    let config = LoopConfig::from_env()?;
    let bytes = std::fs::read(path).with_context(|| format!("read receipt {path}"))?;
    let receipt: govfolio_core::integration::IntegrationReceipt =
        serde_json::from_slice(&bytes).context("parse typed integration receipt")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&config.database_url)
        .await
        .context("connect product database")?;
    let result = govfolio_core::integration::submit_receipt(&pool, &receipt).await?;
    println!(
        "receipt={} inserted={} state={} version={}",
        result.receipt_id, result.inserted, result.state, result.version
    );
    Ok(())
}

async fn receipt_status(receipt_id: &str) -> anyhow::Result<()> {
    let config = LoopConfig::from_env()?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&config.database_url)
        .await
        .context("connect product database")?;
    let state = govfolio_core::integration::receipt_status(&pool, receipt_id).await?;
    println!(
        "receipt={} state={} version={} base={:?} branch={:?} pr={:?} merge={:?} error={:?}",
        state.receipt_id,
        state.state,
        state.version,
        state.candidate_base_sha,
        state.integration_branch,
        state.pr_number,
        state.merge_sha,
        state.last_error,
    );
    Ok(())
}

async fn integrate_command(once: bool) -> anyhow::Result<()> {
    let config = LoopConfig::from_env()?;
    config.paths.ensure()?;
    let store = ControlStore::open_writer(&config.paths.control_db).await?;
    let owner_id = format!("integrator-{}-{}", std::process::id(), Ulid::new());
    let supervisor = store
        .acquire_supervisor(&owner_id, Utc::now(), OWNER_TTL)
        .await?;
    store
        .renew_supervisor(&supervisor, std::process::id(), Utc::now(), OWNER_TTL)
        .await?;
    let result = integrate_forever(&config, &store, once).await;
    store.release_supervisor(&supervisor, Utc::now()).await?;
    result
}

async fn integrate_forever(
    config: &LoopConfig,
    store: &ControlStore,
    once: bool,
) -> anyhow::Result<()> {
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&config.database_url)
        .await
        .context("connect product database for integrator")?;
    loop {
        let processed = integrate_next(config, store, &pool).await?;
        if once {
            return Ok(());
        }
        let delay = if processed {
            StdDuration::from_secs(5)
        } else {
            StdDuration::from_secs(30)
        };
        tokio::select! {
            () = sleep(delay) => {}
            signal = tokio::signal::ctrl_c() => {
                signal.context("wait for integrator stop")?;
                return Ok(());
            }
        }
    }
}

async fn integrate_next(
    config: &LoopConfig,
    store: &ControlStore,
    pool: &sqlx::PgPool,
) -> anyhow::Result<bool> {
    use govfolio_core::integration::IntegrationState;

    let Some((receipt, projection)) =
        govfolio_core::integration::next_actionable_receipt(pool).await?
    else {
        return Ok(false);
    };
    let repair_ordinal = u8::try_from(receipt.repair_ordinal.unwrap_or(0))
        .context("repair ordinal is outside u8")?;
    let candidate = ReceiptCandidate {
        receipt_id: receipt.id.clone(),
        source_sha: receipt.source_sha.clone(),
        base_sha: receipt.base_sha.clone(),
        journal_summary: receipt.journal_summary.clone(),
        repair_ordinal,
    };
    let backend = command_backend(config);
    let mut engine = IntegrationEngine::new(backend);
    match projection.state {
        IntegrationState::Submitted | IntegrationState::Preparing => {
            prepare_receipt(pool, store, projection, &candidate, &mut engine).await
        }
        IntegrationState::AwaitingCi => {
            finalize_receipt(
                pool,
                store,
                projection,
                &candidate,
                &mut engine,
                repair_ordinal,
            )
            .await
        }
        IntegrationState::MergedUnapplied => {
            apply_merged_receipt(pool, store, &receipt, &projection).await?;
            Ok(true)
        }
        IntegrationState::ReworkRequired
        | IntegrationState::Applied
        | IntegrationState::Deferred => Ok(false),
    }
}

async fn prepare_receipt(
    pool: &sqlx::PgPool,
    store: &ControlStore,
    mut projection: govfolio_core::integration::StateProjection,
    candidate: &ReceiptCandidate,
    engine: &mut IntegrationEngine<CommandIntegrationBackend>,
) -> anyhow::Result<bool> {
    use govfolio_core::integration::{IntegrationState, TransitionEvidence};

    if projection.state == IntegrationState::Submitted {
        projection = transition(
            pool,
            &projection,
            IntegrationState::Preparing,
            TransitionEvidence {
                candidate_base_sha: Some(engine.current_main()?),
                ..TransitionEvidence::default()
            },
        )
        .await?;
        mirror_projection(store, &projection).await?;
    }
    let outcome = engine.prepare(candidate)?;
    match outcome {
        PrepareOutcome::AwaitingCi {
            branch,
            pull_request,
            candidate_base_sha,
            candidate_sha,
        } => {
            if projection.candidate_base_sha.as_deref() != Some(&candidate_base_sha) {
                projection = transition(
                    pool,
                    &projection,
                    IntegrationState::Preparing,
                    TransitionEvidence {
                        candidate_base_sha: Some(candidate_base_sha),
                        details: serde_json::json!({"reason": "origin_main_moved"}),
                        ..TransitionEvidence::default()
                    },
                )
                .await?;
            }
            projection = transition(
                pool,
                &projection,
                IntegrationState::AwaitingCi,
                TransitionEvidence {
                    integration_branch: Some(branch),
                    pr_number: Some(i64::try_from(pull_request)?),
                    details: serde_json::json!({"candidate_sha": candidate_sha}),
                    ..TransitionEvidence::default()
                },
            )
            .await?;
        }
        PrepareOutcome::ReworkRequired { reason } => {
            projection =
                transition_failure(pool, &projection, IntegrationState::ReworkRequired, reason)
                    .await?;
        }
        PrepareOutcome::Deferred { reason } => {
            projection = transition_failure(
                pool,
                &projection,
                IntegrationState::ReworkRequired,
                reason.clone(),
            )
            .await?;
            projection =
                transition_failure(pool, &projection, IntegrationState::Deferred, reason).await?;
        }
    }
    mirror_projection(store, &projection).await?;
    Ok(true)
}

async fn finalize_receipt(
    pool: &sqlx::PgPool,
    store: &ControlStore,
    mut projection: govfolio_core::integration::StateProjection,
    candidate: &ReceiptCandidate,
    engine: &mut IntegrationEngine<CommandIntegrationBackend>,
    repair_ordinal: u8,
) -> anyhow::Result<bool> {
    use govfolio_core::integration::{IntegrationState, TransitionEvidence};

    let pull_request = projection
        .pr_number
        .ok_or_else(|| anyhow!("awaiting_ci receipt has no pull request"))?;
    match engine.finalize(candidate, u64::try_from(pull_request)?)? {
        FinalizeOutcome::AwaitingCi => {}
        FinalizeOutcome::ReworkRequired { reason } => {
            projection =
                transition_failure(pool, &projection, IntegrationState::ReworkRequired, reason)
                    .await?;
            if repair_ordinal >= 2 {
                projection = transition_failure(
                    pool,
                    &projection,
                    IntegrationState::Deferred,
                    "repair budget exhausted".to_owned(),
                )
                .await?;
            }
        }
        FinalizeOutcome::Merged { merge_sha } => {
            projection = transition(
                pool,
                &projection,
                IntegrationState::MergedUnapplied,
                TransitionEvidence {
                    merge_sha: Some(merge_sha),
                    ..TransitionEvidence::default()
                },
            )
            .await?;
        }
    }
    mirror_projection(store, &projection).await?;
    Ok(true)
}

async fn apply_merged_receipt(
    pool: &sqlx::PgPool,
    store: &ControlStore,
    receipt: &govfolio_core::integration::IntegrationReceipt,
    projection: &govfolio_core::integration::StateProjection,
) -> anyhow::Result<()> {
    let merge_sha = projection
        .merge_sha
        .clone()
        .ok_or_else(|| anyhow!("merged_unapplied receipt has no merge SHA"))?;
    let mut evidence =
        govfolio_core::integration::ApplyEvidence::successful(&receipt.source_sha, &merge_sha);
    evidence.real_source_verified = receipt.real_source_proof.is_some();
    let applied =
        govfolio_core::integration::apply_receipt(pool, &receipt.id, projection.version, &evidence)
            .await?;
    let applied_projection = govfolio_core::integration::receipt_status(pool, &receipt.id).await?;
    mirror_projection(store, &applied_projection).await?;
    println!(
        "applied receipt={} phase={} released={}",
        applied.receipt_id, applied.coverage_phase, applied.lease_released
    );
    Ok(())
}

fn command_backend(config: &LoopConfig) -> CommandIntegrationBackend {
    let gh = std::env::var_os("GOVFOLIO_GH_BIN").map_or_else(|| PathBuf::from("gh"), PathBuf::from);
    CommandIntegrationBackend::new(
        config.repo.clone(),
        config.paths.root.join("candidates"),
        gh,
    )
}

async fn transition(
    pool: &sqlx::PgPool,
    projection: &govfolio_core::integration::StateProjection,
    to_state: govfolio_core::integration::IntegrationState,
    evidence: govfolio_core::integration::TransitionEvidence,
) -> anyhow::Result<govfolio_core::integration::StateProjection> {
    Ok(govfolio_core::integration::transition_receipt(
        pool,
        &govfolio_core::integration::TransitionRequest {
            receipt_id: projection.receipt_id.clone(),
            expected_state: projection.state,
            expected_version: projection.version,
            to_state,
            actor: govfolio_core::integration::INTEGRATOR_ACTOR.to_owned(),
            evidence,
        },
    )
    .await?)
}

async fn transition_failure(
    pool: &sqlx::PgPool,
    projection: &govfolio_core::integration::StateProjection,
    to_state: govfolio_core::integration::IntegrationState,
    reason: String,
) -> anyhow::Result<govfolio_core::integration::StateProjection> {
    transition(
        pool,
        projection,
        to_state,
        govfolio_core::integration::TransitionEvidence {
            failure: Some(reason),
            ..govfolio_core::integration::TransitionEvidence::default()
        },
    )
    .await
}

async fn mirror_projection(
    store: &ControlStore,
    projection: &govfolio_core::integration::StateProjection,
) -> anyhow::Result<()> {
    store
        .upsert_receipt_mirror(&ReceiptMirror {
            receipt_id: projection.receipt_id.clone(),
            state: projection.state.to_string(),
            branch: projection.integration_branch.clone(),
            pull_request: projection.pr_number,
            candidate_sha: projection.candidate_base_sha.clone(),
            merge_sha: projection.merge_sha.clone(),
            last_error: projection.last_error.clone(),
            updated_at: Utc::now(),
        })
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preflight_config(role: &str) -> LoopConfig {
        let root = PathBuf::from("fixture");
        LoopConfig {
            paths: crate::config::RuntimePaths::under(root.join("state")),
            repo: root.clone(),
            worktree: root.clone(),
            expected_branch: "loop/fixture".to_owned(),
            lane_id: "fixture-0".to_owned(),
            role: role.to_owned(),
            provider: Provider::Codex,
            provider_executable: PathBuf::from("codex"),
            model: Some("gpt-fixture".to_owned()),
            prompt_file: root.join("agents/PROMPT.md"),
            authority_bin: root.join("validate-authority"),
            database_url: "postgres://fixture".to_owned(),
            bronze_root: root.join("bronze"),
            epoch_gate_bin: root.join("epoch-gate"),
            lease_bin: root.join("jurisdiction-lease"),
            epoch: "E3".to_owned(),
            poll_interval: StdDuration::from_secs(30),
        }
    }

    fn preflight_provider() -> ProviderIdentity {
        ProviderIdentity {
            provider: Provider::Codex,
            executable: PathBuf::from("codex"),
            cli_version: "codex fixture".to_owned(),
            model: Some("gpt-fixture".to_owned()),
            config_fingerprint: "fixture".to_owned(),
        }
    }

    #[test]
    fn factory_lanes_default_to_the_factory_prompt() {
        let worktree = PathBuf::from("fixture");

        assert_eq!(
            factory_prompt_file(&worktree, None),
            worktree.join("agents/PROMPT-FACTORY-LANE.md")
        );
        assert_eq!(
            factory_prompt_file(&worktree, Some("custom/prompt.md".into())),
            PathBuf::from("custom/prompt.md")
        );
    }

    fn alternate_provider() -> ProviderIdentity {
        ProviderIdentity {
            provider: Provider::Claude,
            executable: PathBuf::from("claude"),
            cli_version: "claude fixture".to_owned(),
            model: Some("claude-fixture".to_owned()),
            config_fingerprint: "alternate".to_owned(),
        }
    }

    #[test]
    fn orchestrator_preflight_is_not_blocked_by_factory_claimability() {
        let orchestrator =
            preflight_suite(&preflight_config("orchestrator"), &preflight_provider());
        let factory = preflight_suite(&preflight_config("factory"), &preflight_provider());

        assert!(
            !orchestrator
                .probe_names()
                .contains(&"factory_gate_and_claimable")
        );
        assert!(
            factory
                .probe_names()
                .contains(&"factory_gate_and_claimable")
        );
        assert!(orchestrator.probe_names().contains(&"codex_skill_contract"));
        assert!(factory.probe_names().contains(&"codex_skill_contract"));
    }

    #[test]
    fn root_dispatch_prompt_requires_exact_receipt() {
        let contract = "a".repeat(64);
        let envelope = format!(
            "{ROOT_ENVELOPE_BEGIN}\n{{\"contract_sha256\":\"{contract}\",\"role\":\"orchestrator\",\"skills\":[{{\"id\":\"skill:one\",\"codex_name\":\"one\",\"canonical_path\":\"agents/skills/one\"}},{{\"id\":\"skill:two\",\"codex_name\":\"two\",\"canonical_path\":\"agents/skills/two\"}}]}}\n{ROOT_ENVELOPE_END}\n"
        );

        let governed = compose_root_prompt(&envelope, "perform the coordinator task")
            .expect("valid root dispatch composes");

        assert_eq!(
            governed.receipt,
            format!(
                "SKILLS_LOADED role=orchestrator contract={contract} skills=skill:one,skill:two"
            )
        );
        assert!(governed.text.starts_with(ROOT_ENVELOPE_BEGIN));
        assert!(governed.text.contains("perform the coordinator task"));
        assert!(governed.text.contains(&governed.receipt));
        assert!(governed.text.contains("GOVFOLIO_AUTHORITY_BIN"));
        assert!(
            governed
                .reads
                .contains(&".agents/skills/one/SKILL.md".to_owned())
        );
        assert!(
            governed
                .reads
                .contains(&"agents/skills/two/SKILL.md".to_owned())
        );

        let wrong_role = envelope.replace("\"orchestrator\"", "\"implementer\"");
        assert!(compose_root_prompt(&wrong_role, "task").is_err());
        assert!(compose_root_prompt(&envelope.replace('\n', "\r\n"), "task").is_ok());
        assert!(compose_root_prompt("not an envelope", "task").is_err());
    }

    #[test]
    fn root_dispatch_receipt_accepts_only_structured_exact_standalone_lines() {
        let temp = tempfile::tempdir().expect("tempdir");
        let receipt = format!(
            "SKILLS_LOADED role=orchestrator contract={} skills=skill:one",
            "b".repeat(64)
        );
        let allowed_reads = vec!["agents/skills/one/SKILL.md".to_owned()];
        let codex = temp.path().join("codex.jsonl");
        std::fs::write(
            &codex,
            format!(
                "{{\"type\":\"item.started\",\"item\":{{\"type\":\"mcp_tool_call\",\"arguments\":{{\"code\":\"readFile('agents/skills/one/SKILL.md')\"}}}}}}\n{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"ready\\n{receipt}\\n\"}}}}\n"
            ),
        )
        .expect("codex fixture");
        assert!(
            structured_root_receipt(&codex, Provider::Codex, &receipt, &allowed_reads)
                .expect("codex receipt scan")
        );

        let claude = temp.path().join("claude.jsonl");
        std::fs::write(
            &claude,
            format!(
                "{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"{receipt}\"}}]}}}}\n"
            ),
        )
        .expect("claude fixture");
        assert!(
            structured_root_receipt(&claude, Provider::Claude, &receipt, &allowed_reads)
                .expect("claude receipt scan")
        );

        std::fs::write(
            &codex,
            format!(
                "{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"claimed {receipt} but did not emit it\"}}}}\n"
            ),
        )
        .expect("mismatch fixture");
        assert!(
            !structured_root_receipt(&codex, Provider::Codex, &receipt, &allowed_reads)
                .expect("mismatch receipt scan")
        );

        std::fs::write(
            &codex,
            format!(
                "{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\" {receipt} \"}}}}\n"
            ),
        )
        .expect("whitespace mismatch fixture");
        assert!(
            !structured_root_receipt(&codex, Provider::Codex, &receipt, &allowed_reads)
                .expect("whitespace mismatch receipt scan")
        );

        std::fs::write(
            &codex,
            format!(
                "{{\"type\":\"item.started\",\"item\":{{\"type\":\"mcp_tool_call\",\"arguments\":{{\"code\":\"readFile('agents/skills/one/SKILL.md'); readFile('agents/goals/000-INDEX.md')\"}}}}}}\n{{\"type\":\"item.completed\",\"item\":{{\"type\":\"agent_message\",\"text\":\"{receipt}\"}}}}\n"
            ),
        )
        .expect("late receipt fixture");
        assert!(
            !structured_root_receipt(&codex, Provider::Codex, &receipt, &allowed_reads)
                .expect("late receipt scan")
        );

        let decoy = serde_json::json!({
            "note": "agents/skills/one/SKILL.md",
            "code": "performTask()"
        });
        assert_eq!(
            classify_pre_receipt_tool(Some(&decoy), false, &allowed_reads),
            RootReceiptEvent::ForbiddenTool
        );
        let unlisted = serde_json::json!({
            "code": "readFile('agents/skills/unlisted/SKILL.md')"
        });
        assert_eq!(
            classify_pre_receipt_tool(Some(&unlisted), false, &allowed_reads),
            RootReceiptEvent::ForbiddenTool
        );
    }

    #[test]
    fn root_dispatch_recovery_preserves_the_receipt_boundary_first() {
        let original = format!("{ROOT_ENVELOPE_BEGIN}\nroot boundary\n{ROOT_ENVELOPE_END}");
        let recovered = recovery_prompt(&original, 9);

        assert!(recovered.starts_with(&original));
        assert!(recovered.find(ROOT_ENVELOPE_BEGIN) < recovered.find("Cross-provider recovery"));
        assert!(recovered.contains("After satisfying the governed root receipt boundary"));
    }

    #[test]
    fn root_dispatch_postcondition_rejects_completed_turn_without_receipt() {
        let temp = tempfile::tempdir().expect("tempdir");
        let events = temp.path().join("events.jsonl");
        std::fs::write(&events, "{\"type\":\"turn.completed\"}\n").expect("events fixture");
        let mut attempt = AttemptSpec {
            id: "attempt".to_owned(),
            lane_id: "orchestrator-0".to_owned(),
            lane_fence: 1,
            work_key: "work".to_owned(),
            worktree: PathBuf::from("worktree"),
            expected_branch: "loop/orchestrator-0".to_owned(),
            prompt: "prompt".to_owned(),
            required_root_receipt: Some("SKILLS_LOADED exact".to_owned()),
            required_root_reads: Vec::new(),
            prompt_kind: PromptKind::Normal,
            provider: preflight_provider(),
            resume_session_id: None,
            preflight_signature: "preflight".to_owned(),
            git_head_before: "head".to_owned(),
            journal_sha_before: "journal".to_owned(),
        };
        let mut result = NormalizedResult {
            class: ResultClass::Completed,
            terminal_type: Some("turn.completed".to_owned()),
            structured_started: true,
            session_id: Some("thread".to_owned()),
            provider_error_code: None,
            stable_error_hash: None,
            retry_at: None,
            exit_code: Some(0),
            summary: "completed".to_owned(),
        };

        apply_root_receipt_postcondition(&attempt, &events, &mut result)
            .expect("postcondition evaluates");

        assert_eq!(result.class, ResultClass::PostconditionFailed);
        assert!(result.stable_error_hash.is_some());

        attempt.required_root_receipt = None;
        result.class = ResultClass::Completed;
        apply_root_receipt_postcondition(&attempt, &events, &mut result)
            .expect("ungoverned compatibility attempt remains unchanged");
        assert_eq!(result.class, ResultClass::Completed);
    }

    #[test]
    fn provider_environment_injects_prebuilt_runtime_paths() {
        let config = preflight_config("orchestrator");
        let environment = provider_runtime_environment(&config);
        let expected = [
            ("GOVFOLIO_AUTHORITY_BIN", config.authority_bin.clone()),
            (
                "GOVFOLIO_LOOP_BIN",
                config.authority_bin.with_file_name(if cfg!(windows) {
                    "govfolio-loop.exe"
                } else {
                    "govfolio-loop"
                }),
            ),
            ("GOVFOLIO_EPOCH_GATE_BIN", config.epoch_gate_bin.clone()),
            ("GOVFOLIO_LEASE_BIN", config.lease_bin.clone()),
        ];
        for (key, path) in expected {
            assert!(environment.iter().any(|(actual_key, value)| {
                actual_key == key && value == &path.to_string_lossy()
            }));
        }
        assert!(
            environment
                .iter()
                .any(|(key, value)| key == "GOVFOLIO_EPOCH" && value == "E3")
        );
    }

    #[test]
    fn compiler_canary_cache_is_lane_scoped() {
        let first = preflight_config("orchestrator");
        let mut second = first.clone();
        second.lane_id = "factory-1".to_owned();

        assert_ne!(compiler_cache_dir(&first), compiler_cache_dir(&second));
        assert!(compiler_cache_dir(&first).starts_with(first.paths.root.join("canaries")));
        assert!(compiler_cache_dir(&second).starts_with(second.paths.root.join("canaries")));
    }

    #[tokio::test]
    async fn provider_selection_uses_proven_fallback_until_half_open_probe() {
        let temp = tempfile::tempdir().expect("tempdir");
        let store = ControlStore::open_writer(temp.path().join("control.sqlite3"))
            .await
            .expect("store");
        let preferred = preflight_provider();
        let fallback = alternate_provider();
        let now = Utc::now();

        store
            .open_provider_circuit(
                &provider_key(&preferred),
                &preferred.config_fingerprint,
                "transport",
                Some(now + Duration::minutes(5)),
                now,
            )
            .await
            .expect("circuit opens");
        let (selected, alternate) = select_lane_provider(
            &store,
            &preferred,
            Some(&fallback),
            now + Duration::minutes(1),
        )
        .await
        .expect("selection");
        assert_eq!(selected.provider, Provider::Claude);
        assert!(alternate.is_none());

        let (selected, alternate) = select_lane_provider(
            &store,
            &preferred,
            Some(&fallback),
            now + Duration::minutes(5),
        )
        .await
        .expect("half-open selection");
        assert_eq!(selected.provider, Provider::Codex);
        assert_eq!(
            alternate.map(|identity| identity.provider),
            Some(Provider::Claude)
        );
    }

    #[test]
    fn release0_failure_fingerprint_ignores_quota_reset_time() {
        let attempt = AttemptSpec {
            id: "attempt".to_owned(),
            lane_id: "orchestrator-0".to_owned(),
            lane_fence: 1,
            work_key: "work".to_owned(),
            worktree: PathBuf::from("worktree"),
            expected_branch: "loop/orchestrator-0".to_owned(),
            prompt: "prompt".to_owned(),
            required_root_receipt: None,
            required_root_reads: Vec::new(),
            prompt_kind: PromptKind::Normal,
            provider: ProviderIdentity {
                provider: Provider::Codex,
                executable: PathBuf::from("codex"),
                cli_version: "1.0".to_owned(),
                model: Some("model".to_owned()),
                config_fingerprint: "config".to_owned(),
            },
            resume_session_id: None,
            preflight_signature: "preflight".to_owned(),
            git_head_before: "head".to_owned(),
            journal_sha_before: "journal".to_owned(),
        };
        let result = |retry_at| NormalizedResult {
            class: ResultClass::QuotaExhausted,
            terminal_type: Some("turn.failed".to_owned()),
            structured_started: true,
            session_id: None,
            provider_error_code: Some("usage_limit".to_owned()),
            stable_error_hash: Some("stable".to_owned()),
            retry_at,
            exit_code: Some(1),
            summary: "quota".to_owned(),
        };
        assert_eq!(
            failure_fingerprint(&attempt, &result(Some(Utc::now()))),
            failure_fingerprint(&attempt, &result(None))
        );
    }
}
