use std::io::Write as _;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context as _;
use chrono::{Duration as ChronoDuration, Utc};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt as _, BufReader};
use tokio::sync::{Mutex, Notify, RwLock, mpsc, oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::{Instant, MissedTickBehavior, interval};
use ulid::Ulid;

use crate::build_classifier::{
    CargoDisposition, ClassificationContext, apply_job_budget, classify_cargo,
};
use crate::build_interference::{foreign_govfolio_processes, observe_processes};
use crate::build_policy::BuildPolicySnapshot;
use crate::build_protocol::{
    BuildControlRequest, BuildRequestMessage, ClientEnvelope, ControlEndpoint, ProtocolError,
    ServerFrame, read_json_line, validate_envelope, write_json_line,
};
use crate::build_scheduler::{
    BuildAdmissionConfig, BuildScheduler, QueuedBuild, ResourceSnapshot, RunningBuild,
    sample_resource_snapshot,
};
use crate::build_store::{BuildRequestSpec, BuildRequestState, BuildTerminal};
use crate::build_transport::{LocalControlListener, LocalServerStream, connect_local_control};
use crate::model::CommandSpec;
use crate::process::{
    ProcessRunner, RawProcessEvent, RawProcessExecution, RawProcessOutputPaths, cancellation_pair,
    observed_process_activity, should_retry_build_failure,
};
use crate::store::{ControlStore, SupervisorFence};

const SUPERVISOR_TTL: ChronoDuration = ChronoDuration::seconds(90);
const SUPERVISOR_HEARTBEAT: Duration = Duration::from_secs(20);
const MAX_BOUNDED_POLICY_BYTES: usize = 64 * 1024;

pub struct BuildServerOptions {
    pub state_root: PathBuf,
    pub repository: PathBuf,
    pub bronze_roots: Vec<PathBuf>,
    pub cargo_program: PathBuf,
    pub cargo_prefix_args: Vec<String>,
    pub policy: BuildPolicySnapshot,
    pub bounded_policy: String,
    pub policy_reload: bool,
    pub process_observer: Option<ProcessObserver>,
    pub control_token: String,
    pub config: BuildAdmissionConfig,
    pub resource_override: Option<ResourceSnapshot>,
    pub store: Arc<ControlStore>,
    pub supervisor: SupervisorFence,
}

pub type ProcessObserver =
    Arc<dyn Fn() -> anyhow::Result<Vec<crate::build_interference::ObservedProcess>> + Send + Sync>;

#[derive(Default)]
struct RuntimeState {
    queued: Vec<QueuedBuild>,
    running: Vec<RunningBuild>,
}

#[derive(Clone)]
struct ActivePolicy {
    snapshot: BuildPolicySnapshot,
    bounded_policy: String,
}

struct AttemptRun {
    execution: RawProcessExecution,
    client_connected: bool,
    deadline_cancelled: bool,
    recovery_reason: Option<String>,
    interference_reason: Option<String>,
    output: RawProcessOutputPaths,
}

struct BuildServiceInner {
    state_root: PathBuf,
    repository: PathBuf,
    bronze_roots: Vec<PathBuf>,
    cargo_program: PathBuf,
    cargo_prefix_args: Vec<String>,
    active_policy: RwLock<ActivePolicy>,
    policy_refresh: Mutex<()>,
    policy_reload: bool,
    process_observer: Option<ProcessObserver>,
    control_token: String,
    scheduler: BuildScheduler,
    resource_override: Option<ResourceSnapshot>,
    store: Arc<ControlStore>,
    supervisor: SupervisorFence,
    runtime: Mutex<RuntimeState>,
    changed: Notify,
}

#[derive(Clone)]
pub struct BuildAdmissionServer {
    inner: Arc<BuildServiceInner>,
}

impl BuildAdmissionServer {
    #[must_use]
    pub fn new(options: BuildServerOptions) -> Self {
        Self {
            inner: Arc::new(BuildServiceInner {
                state_root: options.state_root,
                repository: options.repository,
                bronze_roots: options.bronze_roots,
                cargo_program: options.cargo_program,
                cargo_prefix_args: options.cargo_prefix_args,
                active_policy: RwLock::new(ActivePolicy {
                    snapshot: options.policy,
                    bounded_policy: bounded_text(&options.bounded_policy),
                }),
                policy_refresh: Mutex::new(()),
                policy_reload: options.policy_reload,
                process_observer: options.process_observer,
                control_token: options.control_token,
                scheduler: BuildScheduler::new(options.config),
                resource_override: options.resource_override,
                store: options.store,
                supervisor: options.supervisor,
                runtime: Mutex::new(RuntimeState::default()),
                changed: Notify::new(),
            }),
        }
    }

    pub async fn serve(self, shutdown: watch::Receiver<bool>) -> anyhow::Result<()> {
        self.serve_inner(shutdown, None).await
    }

    pub(crate) async fn serve_with_ready(
        self,
        shutdown: watch::Receiver<bool>,
        ready: tokio::sync::oneshot::Sender<()>,
    ) -> anyhow::Result<()> {
        self.serve_inner(shutdown, Some(ready)).await
    }

    async fn serve_inner(
        self,
        mut shutdown: watch::Receiver<bool>,
        ready: Option<tokio::sync::oneshot::Sender<()>>,
    ) -> anyhow::Result<()> {
        let policy = self.inner.active_policy.read().await.clone();
        self.inner
            .store
            .record_build_policy_snapshot(&policy.snapshot)
            .await
            .context("record active build policy snapshot")?;
        self.inner
            .store
            .reconcile_build_requests(&self.inner.supervisor, Utc::now())
            .await
            .context("reconcile build requests")?;
        let endpoint = ControlEndpoint::for_state_root(&self.inner.state_root)?;
        let mut listener =
            LocalControlListener::bind(&endpoint).context("bind local build control endpoint")?;
        if let Some(ready) = ready {
            let _receiver_dropped = ready.send(());
        }
        let mut heartbeat = interval(SUPERVISOR_HEARTBEAT);
        heartbeat.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut handlers = JoinSet::new();
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                _ = heartbeat.tick() => {
                    self.inner.store.renew_supervisor(
                        &self.inner.supervisor,
                        std::process::id(),
                        Utc::now(),
                        SUPERVISOR_TTL,
                    ).await.context("renew build supervisor fence")?;
                }
                accepted = listener.accept() => {
                    let stream = accepted.context("accept local build control client")?;
                    let server = self.clone();
                    handlers.spawn(async move { server.handle(stream).await });
                }
                joined = handlers.join_next(), if !handlers.is_empty() => {
                    if let Some(joined) = joined {
                        match joined {
                            Ok(Ok(())) => {}
                            Ok(Err(error)) => eprintln!(
                                "govfolio-loop: build control client failed: {}",
                                bounded_error(&error),
                            ),
                            Err(error) => eprintln!(
                                "govfolio-loop: build control handler panicked: {}",
                                bounded_error(&error),
                            ),
                        }
                    }
                }
            }
        }
        while let Some(joined) = handlers.join_next().await {
            match joined {
                Ok(Ok(())) => {}
                Ok(Err(error)) => eprintln!(
                    "govfolio-loop: build control client failed during shutdown: {}",
                    bounded_error(&error),
                ),
                Err(error) => eprintln!(
                    "govfolio-loop: build control handler panicked during shutdown: {}",
                    bounded_error(&error),
                ),
            }
        }
        Ok(())
    }

    async fn handle(&self, stream: LocalServerStream) -> anyhow::Result<()> {
        let (read, mut write) = tokio::io::split(stream);
        let mut read = BufReader::new(read);
        let Some(envelope) = read_json_line::<_, ClientEnvelope>(&mut read).await? else {
            return Ok(());
        };
        let active = self.refresh_policy().await?;
        if let Err(error) = validate_envelope(
            &envelope,
            &self.inner.control_token,
            self.inner.supervisor.fence,
            &active.snapshot.policy_sha256,
        ) {
            write_json_line(
                &mut write,
                &protocol_error_frame(&error, &active.bounded_policy),
            )
            .await?;
            return Ok(());
        }
        match envelope.request {
            BuildControlRequest::Build(build) => {
                self.run_build(build, active, &mut read, &mut write).await
            }
            BuildControlRequest::Policy => {
                write_json_line(
                    &mut write,
                    &ServerFrame::Policy {
                        snapshot: active.snapshot,
                        bounded_policy: active.bounded_policy,
                        supervisor_fence: self.inner.supervisor.fence,
                    },
                )
                .await?;
                Ok(())
            }
            BuildControlRequest::Recover { request_id, .. } => {
                match self
                    .inner
                    .store
                    .recover_build(&self.inner.supervisor, &request_id, Utc::now())
                    .await
                {
                    Ok(()) => {
                        write_json_line(
                            &mut write,
                            &ServerFrame::Terminal {
                                request_id,
                                state: BuildRequestState::Cancelled,
                                exit_code: None,
                            },
                        )
                        .await?;
                    }
                    Err(error) => {
                        write_json_line(
                            &mut write,
                            &ServerFrame::Error {
                                code: "recovery_required".to_owned(),
                                message: error.to_string(),
                                active_policy_sha256: None,
                                bounded_policy: None,
                            },
                        )
                        .await?;
                    }
                }
                Ok(())
            }
            BuildControlRequest::Status => {
                write_json_line(
                    &mut write,
                    &ServerFrame::Error {
                        code: "status_use_cli".to_owned(),
                        message: "use govfolio-loop status for the bounded durable snapshot"
                            .to_owned(),
                        active_policy_sha256: Some(active.snapshot.policy_sha256),
                        bounded_policy: None,
                    },
                )
                .await?;
                Ok(())
            }
        }
    }

    async fn refresh_policy(&self) -> anyhow::Result<ActivePolicy> {
        let _refresh = self.inner.policy_refresh.lock().await;
        if !self.inner.policy_reload {
            return Ok(self.inner.active_policy.read().await.clone());
        }
        let snapshot = crate::build_policy::load_build_policy(&self.inner.repository, Utc::now())?;
        {
            let active = self.inner.active_policy.read().await;
            if active.snapshot.policy_sha256 == snapshot.policy_sha256 {
                return Ok(active.clone());
            }
        }
        let bounded_policy = bounded_text(&std::fs::read_to_string(
            self.inner.repository.join(crate::build_policy::POLICY_PATH),
        )?);
        self.inner
            .store
            .record_build_policy_snapshot(&snapshot)
            .await?;
        let refreshed = ActivePolicy {
            snapshot,
            bounded_policy,
        };
        *self.inner.active_policy.write().await = refreshed.clone();
        Ok(refreshed)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "admission validates, persists, executes, and finalizes one fenced request"
    )]
    async fn run_build<R, W>(
        &self,
        build: BuildRequestMessage,
        acquired_policy: ActivePolicy,
        read: &mut R,
        write: &mut W,
    ) -> anyhow::Result<()>
    where
        R: AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        if self.inner.store.has_build_recovery_required().await? {
            write_json_line(
                write,
                &ServerFrame::Error {
                    code: "recovery_required".to_owned(),
                    message: "ambiguous prior build identity blocks admission".to_owned(),
                    active_policy_sha256: None,
                    bounded_policy: None,
                },
            )
            .await?;
            return Ok(());
        }
        let context = ClassificationContext {
            worktree: build.worktree.clone(),
            target_dir: build.target_dir.clone(),
            shared_target: self.inner.repository.join("target"),
            bronze_roots: self.inner.bronze_roots.clone(),
            category: build.category.clone(),
        };
        let class = match classify_cargo(&build.cargo_args, &context, build.explicit_class)? {
            CargoDisposition::Managed(class) => class,
            CargoDisposition::Passthrough => {
                write_json_line(
                    write,
                    &ServerFrame::Error {
                        code: "unmanaged_passthrough".to_owned(),
                        message: "non-compiling Cargo command does not require admission"
                            .to_owned(),
                        active_policy_sha256: None,
                        bounded_policy: None,
                    },
                )
                .await?;
                return Ok(());
            }
        };
        let jobs = self.inner.scheduler.config().jobs_for(class);
        let cargo_args = apply_job_budget(&build.cargo_args, jobs)?;
        let mut exact_args = self.inner.cargo_prefix_args.clone();
        exact_args.extend(cargo_args);
        let exact_command = ExactCommand {
            program: self.inner.cargo_program.to_string_lossy().into_owned(),
            args: exact_args.clone(),
            cwd: build.worktree.to_string_lossy().into_owned(),
            target_dir: build.target_dir.to_string_lossy().into_owned(),
            jobs,
        };
        let command_bytes = serde_json::to_vec(&exact_command)?;
        let request_id = Ulid::new().to_string();
        let now = Utc::now();
        let spec = BuildRequestSpec {
            request_id: request_id.clone(),
            lane_id: build.lane_id,
            lane_fence: build.lane_fence,
            owner_identity: build.owner_identity,
            policy_sha256: build.policy_sha256,
            resource_class: class,
            category: build.category,
            worktree: build.worktree.clone(),
            target_dir: build.target_dir.clone(),
            command_sha256: hex::encode(Sha256::digest(&command_bytes)),
            effective_jobs: jobs,
            deadline: now
                + ChronoDuration::from_std(self.inner.scheduler.config().queue_deadline)
                    .context("queue deadline exceeds chrono range")?,
        };
        let record = self
            .inner
            .store
            .enqueue_build(&self.inner.supervisor, &spec, now)
            .await?;
        let measurement = is_measurement_category(record.category.as_deref());
        {
            let mut runtime = self.inner.runtime.lock().await;
            runtime.queued.push(QueuedBuild {
                request_id: request_id.clone(),
                queue_sequence: record.queue_sequence,
                resource_class: class,
            });
        }
        self.inner.changed.notify_waiters();

        if !self.wait_for_admission(&record, read, write).await? {
            return Ok(());
        }
        if write_json_line(
            write,
            &ServerFrame::Admission {
                request_id: request_id.clone(),
                resource_class: class,
                effective_jobs: jobs,
                policy_sha256: acquired_policy.snapshot.policy_sha256,
            },
        )
        .await
        .is_err()
        {
            self.inner
                .store
                .cancel_queued_build(
                    &self.inner.supervisor,
                    &request_id,
                    "client_disconnected_before_start",
                    Utc::now(),
                )
                .await?;
            self.release_runtime(&request_id).await;
            return Ok(());
        }

        let evidence_dir = self
            .inner
            .state_root
            .join("build-evidence")
            .join(&request_id);
        std::fs::create_dir_all(&evidence_dir)?;
        let command = CommandSpec {
            program: self.inner.cargo_program.clone(),
            args: exact_args,
            cwd: build.worktree,
            stdin: Vec::new(),
            env: vec![
                (
                    "CARGO_TARGET_DIR".to_owned(),
                    exact_command.target_dir.clone(),
                ),
                ("CARGO_BUILD_JOBS".to_owned(), jobs.to_string()),
            ],
            remove_env: Vec::new(),
        };
        let mut retry_count = 0_u8;
        let mut attempts = Vec::new();
        let command_started = Instant::now();
        let final_run = loop {
            let output = RawProcessOutputPaths {
                stdout: evidence_dir.join(format!("stdout-{retry_count}.log")),
                stderr: evidence_dir.join(format!("stderr-{retry_count}.log")),
            };
            let run = match self
                .execute_attempt(
                    &request_id,
                    &command,
                    output.clone(),
                    retry_count,
                    command_started,
                    measurement,
                    read,
                    write,
                )
                .await
            {
                Ok(run) => run,
                Err(error) => {
                    self.fail_attempt(
                        &request_id,
                        &evidence_dir,
                        &command_bytes,
                        output,
                        attempts,
                        &error,
                        write,
                    )
                    .await?;
                    return Ok(());
                }
            };
            attempts.push(AttemptEvidenceSource {
                output: run.output.clone(),
                exit_code: run.execution.exit_code,
                cancelled: run.execution.cancelled,
            });
            if run.client_connected
                && !run.deadline_cancelled
                && run.recovery_reason.is_none()
                && run.interference_reason.is_none()
                && should_retry_build_failure(
                    run.execution.exit_code,
                    &run.execution.stderr_tail,
                    run.execution.cancelled,
                    retry_count,
                )
            {
                retry_count += 1;
                continue;
            }
            break run;
        };
        let execution = final_run.execution;
        let client_connected = final_run.client_connected;
        let deadline_cancelled = final_run.deadline_cancelled;
        let recovery_reason = final_run.recovery_reason;
        let interference_reason = final_run.interference_reason;
        let (terminal, state, reported_exit) = if let Some(reason) = interference_reason {
            (
                BuildTerminal::Inconclusive { reason },
                BuildRequestState::Inconclusive,
                None,
            )
        } else if deadline_cancelled {
            (BuildTerminal::TimedOut, BuildRequestState::TimedOut, None)
        } else if execution.cancelled || !client_connected {
            (BuildTerminal::Cancelled, BuildRequestState::Cancelled, None)
        } else if execution.exit_code == Some(0) {
            (
                BuildTerminal::Completed { exit_code: 0 },
                BuildRequestState::Completed,
                Some(0),
            )
        } else {
            let exit_code = execution.exit_code.unwrap_or(1);
            (
                BuildTerminal::Failed { exit_code },
                BuildRequestState::Failed,
                Some(exit_code),
            )
        };
        let evidence = persist_evidence(&evidence_dir, &command_bytes, &attempts)?;
        self.inner
            .store
            .record_build_evidence(
                &request_id,
                &evidence.sha256,
                "cargo_execution",
                &evidence.path,
                evidence.size_bytes,
                Utc::now(),
            )
            .await?;
        if let Some(reason) = recovery_reason {
            self.inner
                .store
                .mark_build_recovery_required(
                    &self.inner.supervisor,
                    &request_id,
                    &reason,
                    Some(&evidence.sha256),
                    Utc::now(),
                )
                .await?;
            self.release_runtime(&request_id).await;
            if client_connected {
                write_json_line(
                    write,
                    &ServerFrame::Terminal {
                        request_id,
                        state: BuildRequestState::RecoveryRequired,
                        exit_code: None,
                    },
                )
                .await?;
            }
            return Ok(());
        }
        self.inner
            .store
            .finish_build(
                &self.inner.supervisor,
                &request_id,
                terminal,
                Some(&evidence.sha256),
                Utc::now(),
            )
            .await?;
        self.release_runtime(&request_id).await;
        if client_connected {
            write_json_line(
                write,
                &ServerFrame::Terminal {
                    request_id,
                    state,
                    exit_code: reported_exit,
                },
            )
            .await?;
        }
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "failure finalization needs request, evidence, diagnostic, and client context"
    )]
    async fn fail_attempt<W>(
        &self,
        request_id: &str,
        evidence_dir: &std::path::Path,
        command_bytes: &[u8],
        output: RawProcessOutputPaths,
        mut attempts: Vec<AttemptEvidenceSource>,
        error: &anyhow::Error,
        write: &mut W,
    ) -> anyhow::Result<()>
    where
        W: tokio::io::AsyncWrite + Unpin,
    {
        let diagnostic = format!("supervisor failed to execute admitted Cargo: {error:#}\n");
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&output.stderr)?
            .write_all(diagnostic.as_bytes())?;
        attempts.push(AttemptEvidenceSource {
            output,
            exit_code: None,
            cancelled: false,
        });
        let evidence = persist_evidence(evidence_dir, command_bytes, &attempts)?;
        self.inner
            .store
            .record_build_evidence(
                request_id,
                &evidence.sha256,
                "cargo_execution",
                &evidence.path,
                evidence.size_bytes,
                Utc::now(),
            )
            .await?;
        let request = self
            .inner
            .store
            .build_request(request_id)
            .await?
            .context("admitted build disappeared while recording execution failure")?;
        match request.state {
            BuildRequestState::Queued => {
                self.inner
                    .store
                    .fail_queued_build(
                        &self.inner.supervisor,
                        request_id,
                        "cargo_launch_failed",
                        Some(&evidence.sha256),
                        Utc::now(),
                    )
                    .await?;
            }
            BuildRequestState::Running => {
                self.inner
                    .store
                    .finish_build(
                        &self.inner.supervisor,
                        request_id,
                        BuildTerminal::Failed { exit_code: 1 },
                        Some(&evidence.sha256),
                        Utc::now(),
                    )
                    .await?;
            }
            state => anyhow::bail!(
                "admitted build {request_id} entered unexpected {state:?} after execution failure"
            ),
        }
        self.release_runtime(request_id).await;
        let _client_disconnected = write_json_line(
            write,
            &ServerFrame::Terminal {
                request_id: request_id.to_owned(),
                state: BuildRequestState::Failed,
                exit_code: Some(1),
            },
        )
        .await;
        Ok(())
    }

    #[expect(
        clippy::too_many_arguments,
        clippy::too_many_lines,
        reason = "attempt execution coordinates fencing, streaming, cancellation, interference, and deadlines"
    )]
    async fn execute_attempt<R, W>(
        &self,
        request_id: &str,
        command: &CommandSpec,
        output: RawProcessOutputPaths,
        retry_count: u8,
        command_started: Instant,
        measurement: bool,
        read: &mut R,
        write: &mut W,
    ) -> anyhow::Result<AttemptRun>
    where
        R: AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        let (events_tx, mut events_rx) = mpsc::channel(64);
        let (pid_tx, pid_rx) = oneshot::channel();
        let (cancel, cancellation) = cancellation_pair();
        let runner = ProcessRunner::default();
        let task_output = output.clone();
        let task_command = command.clone();
        let process_task = tokio::spawn(async move {
            runner
                .run_raw(&task_command, &task_output, cancellation, events_tx, pid_tx)
                .await
        });
        let identity = pid_rx
            .await
            .context("raw Cargo process did not report its PID identity")?;
        if retry_count == 0 {
            self.inner
                .store
                .start_build(&self.inner.supervisor, request_id, &identity, Utc::now())
                .await?;
        } else {
            self.inner
                .store
                .retry_build(&self.inner.supervisor, request_id, &identity, Utc::now())
                .await?;
        }

        let mut client_connected = true;
        let mut disconnect_buffer = [0_u8; 1];
        let mut progress_tick = interval(self.inner.scheduler.config().heartbeat);
        progress_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut interference_tick = interval(Duration::from_secs(1));
        interference_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let started = Instant::now();
        let mut last_progress = started;
        let mut last_activity = observed_process_activity(identity.pid).ok().flatten();
        let mut deadline_cancelled = false;
        let mut recovery_reason = None;
        let mut interference_reason = None;
        loop {
            tokio::select! {
                event = events_rx.recv() => {
                    let Some(event) = event else { break; };
                    let frame = match event {
                        RawProcessEvent::Stdout(bytes) => Some(ServerFrame::Stdout {
                            request_id: request_id.to_owned(),
                            bytes,
                        }),
                        RawProcessEvent::Stderr(bytes) => Some(ServerFrame::Stderr {
                            request_id: request_id.to_owned(),
                            bytes,
                        }),
                        RawProcessEvent::Progress => None,
                    };
                    last_progress = Instant::now();
                    if let Some(frame) = frame
                        && client_connected
                        && write_json_line(write, &frame).await.is_err()
                    {
                        client_connected = false;
                        cancel.cancel();
                    }
                }
                disconnected = read.read(&mut disconnect_buffer), if client_connected => {
                    drop(disconnected);
                    client_connected = false;
                    cancel.cancel();
                }
                _ = progress_tick.tick() => {
                    let activity = observed_process_activity(identity.pid).ok().flatten();
                    if activity.is_some() && activity != last_activity {
                        last_activity = activity;
                        last_progress = Instant::now();
                    }
                    if let Err(error) = self.inner.store.heartbeat_build(
                        &self.inner.supervisor,
                        request_id,
                        Utc::now(),
                    ).await {
                        recovery_reason = Some(format!("build fence invalidated: {error}"));
                        cancel.cancel();
                    }
                    if command_started.elapsed() >= self.inner.scheduler.config().experiment_deadline
                        || last_progress.elapsed() >= self.inner.scheduler.config().no_progress_deadline
                    {
                        deadline_cancelled = true;
                        cancel.cancel();
                    }
                }
                _ = interference_tick.tick(), if measurement && interference_reason.is_none() => {
                    match self.foreign_processes(&command.cwd, vec![identity.pid]).await {
                        Ok(foreign) if !foreign.is_empty() => {
                            let pids = foreign
                                .iter()
                                .map(|process| process.pid.to_string())
                                .collect::<Vec<_>>()
                                .join(",");
                            interference_reason = Some(format!(
                                "foreign_govfolio_rust_processes:{pids}"
                            ));
                            cancel.cancel();
                        }
                        Err(error) => {
                            interference_reason = Some(format!(
                                "process_observation_failed:{}",
                                bounded_error(&error)
                            ));
                            cancel.cancel();
                        }
                        Ok(_) => {}
                    }
                }
            }
        }
        let execution = process_task
            .await
            .context("raw Cargo process task panicked")??;
        Ok(AttemptRun {
            execution,
            client_connected,
            deadline_cancelled,
            recovery_reason,
            interference_reason,
            output,
        })
    }

    #[expect(
        clippy::too_many_lines,
        reason = "admission atomically coordinates resources, interference, fairness, deadlines, and disconnects"
    )]
    async fn wait_for_admission<R, W>(
        &self,
        record: &crate::build_store::BuildRequestRecord,
        read: &mut R,
        write: &mut W,
    ) -> anyhow::Result<bool>
    where
        R: AsyncRead + Unpin,
        W: tokio::io::AsyncWrite + Unpin,
    {
        loop {
            let now = Utc::now();
            if now >= record.deadline {
                {
                    let mut runtime = self.inner.runtime.lock().await;
                    runtime
                        .queued
                        .retain(|item| item.request_id != record.request_id);
                }
                self.inner
                    .store
                    .timeout_queued_build(&self.inner.supervisor, &record.request_id, now)
                    .await?;
                self.inner.changed.notify_waiters();
                write_json_line(
                    write,
                    &ServerFrame::Terminal {
                        request_id: record.request_id.clone(),
                        state: BuildRequestState::TimedOut,
                        exit_code: None,
                    },
                )
                .await?;
                return Ok(false);
            }
            let active_policy = self.refresh_policy().await?;
            if active_policy.snapshot.policy_sha256 != record.policy_sha256 {
                self.cancel_queued(&record.request_id, "policy_refresh_required")
                    .await?;
                write_json_line(
                    write,
                    &ServerFrame::Error {
                        code: "policy_refresh_required".to_owned(),
                        message: "queued build policy is no longer active".to_owned(),
                        active_policy_sha256: Some(active_policy.snapshot.policy_sha256),
                        bounded_policy: Some(active_policy.bounded_policy),
                    },
                )
                .await?;
                return Ok(false);
            }
            let resources = self.inner.resource_override.map_or_else(
                || sample_resource_snapshot(existing_volume_path(&record.target_dir)),
                Ok,
            );
            let supervised_roots = self
                .inner
                .store
                .list_build_requests()
                .await?
                .into_iter()
                .filter(|request| request.state == BuildRequestState::Running)
                .filter_map(|request| request.process_identity.map(|identity| identity.pid))
                .collect();
            let interference = match self
                .foreign_processes(&record.worktree, supervised_roots)
                .await
            {
                Ok(foreign) => !foreign.is_empty(),
                Err(error) => {
                    eprintln!(
                        "govfolio-loop: admissions paused because process observation failed: {}",
                        bounded_error(&error)
                    );
                    true
                }
            };
            let (admitted, position) = {
                let mut runtime = self.inner.runtime.lock().await;
                let admitted = if interference {
                    Vec::new()
                } else {
                    resources.as_ref().ok().map_or_else(Vec::new, |resources| {
                        self.inner
                            .scheduler
                            .admit(&runtime.queued, &runtime.running, *resources)
                    })
                };
                if admitted.iter().any(|id| id == &record.request_id) {
                    runtime
                        .queued
                        .retain(|item| item.request_id != record.request_id);
                    runtime.running.push(RunningBuild {
                        request_id: record.request_id.clone(),
                        resource_class: record.resource_class,
                    });
                    (true, 0)
                } else {
                    let mut queued = runtime.queued.clone();
                    queued.sort_by_key(|item| item.queue_sequence);
                    let position = queued
                        .iter()
                        .position(|item| item.request_id == record.request_id)
                        .map_or(0, |index| index + 1);
                    (false, position)
                }
            };
            if admitted {
                return Ok(true);
            }
            if write_json_line(
                write,
                &ServerFrame::QueueHeartbeat {
                    request_id: record.request_id.clone(),
                    position,
                },
            )
            .await
            .is_err()
            {
                self.cancel_queued(&record.request_id, "client_disconnected")
                    .await?;
                return Ok(false);
            }
            let remaining = (record.deadline - now)
                .to_std()
                .unwrap_or(Duration::from_millis(1));
            let wait = self.inner.scheduler.config().heartbeat.min(remaining);
            let mut disconnect_buffer = [0_u8; 1];
            tokio::select! {
                () = self.inner.changed.notified() => {}
                () = tokio::time::sleep(wait) => {}
                _ = read.read(&mut disconnect_buffer) => {
                    self.cancel_queued(&record.request_id, "client_disconnected").await?;
                    return Ok(false);
                }
            }
        }
    }

    async fn cancel_queued(&self, request_id: &str, reason: &str) -> anyhow::Result<()> {
        {
            let mut runtime = self.inner.runtime.lock().await;
            runtime.queued.retain(|item| item.request_id != request_id);
        }
        self.inner
            .store
            .cancel_queued_build(&self.inner.supervisor, request_id, reason, Utc::now())
            .await?;
        self.inner.changed.notify_waiters();
        Ok(())
    }

    async fn release_runtime(&self, request_id: &str) {
        let mut runtime = self.inner.runtime.lock().await;
        runtime.running.retain(|item| item.request_id != request_id);
        drop(runtime);
        self.inner.changed.notify_waiters();
    }

    async fn foreign_processes(
        &self,
        worktree: &std::path::Path,
        supervised_roots: Vec<u32>,
    ) -> anyhow::Result<Vec<crate::build_interference::ObservedProcess>> {
        let repository = self.inner.repository.clone();
        let worktree = worktree.to_path_buf();
        let observer = self.inner.process_observer.clone();
        tokio::task::spawn_blocking(move || {
            let processes = observer.map_or_else(observe_processes, |observer| observer())?;
            Ok(foreign_govfolio_processes(
                &processes,
                &repository,
                &worktree,
                std::process::id(),
                &supervised_roots,
            ))
        })
        .await
        .context("join process interference observation")?
    }
}

pub async fn execute_control_request(
    state_root: &std::path::Path,
    envelope: &ClientEnvelope,
) -> anyhow::Result<Vec<ServerFrame>> {
    let mut frames = Vec::new();
    stream_control_request(state_root, envelope, |frame| {
        frames.push(frame);
        Ok(())
    })
    .await?;
    Ok(frames)
}

pub async fn stream_control_request<F>(
    state_root: &std::path::Path,
    envelope: &ClientEnvelope,
    mut on_frame: F,
) -> anyhow::Result<()>
where
    F: FnMut(ServerFrame) -> anyhow::Result<()>,
{
    let endpoint = ControlEndpoint::for_state_root(state_root)?;
    let stream = connect_local_control(&endpoint)
        .await
        .context("connect to build admission server")?;
    let (read, mut write) = tokio::io::split(stream);
    write_json_line(&mut write, envelope).await?;
    let _keep_write_open = write;
    let mut read = BufReader::new(read);
    while let Some(frame) = read_json_line::<_, ServerFrame>(&mut read).await? {
        on_frame(frame)?;
    }
    Ok(())
}

#[derive(Serialize)]
struct ExactCommand {
    program: String,
    args: Vec<String>,
    cwd: String,
    target_dir: String,
    jobs: usize,
}

#[derive(Serialize)]
struct EvidenceManifest {
    command: serde_json::Value,
    attempts: Vec<AttemptEvidence>,
}

#[derive(Serialize)]
struct AttemptEvidence {
    stdout_sha256: String,
    stderr_sha256: String,
    exit_code: Option<i32>,
    cancelled: bool,
}

struct AttemptEvidenceSource {
    output: RawProcessOutputPaths,
    exit_code: Option<i32>,
    cancelled: bool,
}

struct PersistedEvidence {
    path: PathBuf,
    sha256: String,
    size_bytes: u64,
}

fn persist_evidence(
    directory: &std::path::Path,
    command_bytes: &[u8],
    sources: &[AttemptEvidenceSource],
) -> anyhow::Result<PersistedEvidence> {
    let command = serde_json::from_slice(command_bytes)?;
    let attempts = sources
        .iter()
        .map(|source| {
            let stdout = std::fs::read(&source.output.stdout)?;
            let stderr = std::fs::read(&source.output.stderr)?;
            Ok(AttemptEvidence {
                stdout_sha256: hex::encode(Sha256::digest(stdout)),
                stderr_sha256: hex::encode(Sha256::digest(stderr)),
                exit_code: source.exit_code,
                cancelled: source.cancelled,
            })
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    let manifest = EvidenceManifest { command, attempts };
    let bytes = serde_json::to_vec(&manifest)?;
    let path = directory.join("evidence.json");
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(PersistedEvidence {
        path,
        sha256: hex::encode(Sha256::digest(&bytes)),
        size_bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    })
}

fn protocol_error_frame(error: &ProtocolError, bounded_policy: &str) -> ServerFrame {
    let (code, active, policy) = match error {
        ProtocolError::PolicyRefreshRequired { active } => (
            "policy_refresh_required",
            Some(active.clone()),
            Some(bounded_policy.to_owned()),
        ),
        ProtocolError::InvalidToken => ("authentication_failed", None, None),
        ProtocolError::InvalidVersion(_) => ("protocol_version", None, None),
        ProtocolError::StaleFence { .. } => ("stale_fence", None, None),
        ProtocolError::InvalidOwner | ProtocolError::InvalidRequest(_) => {
            ("invalid_identity", None, None)
        }
        ProtocolError::Io(_) | ProtocolError::Json(_) | ProtocolError::FrameTooLarge => {
            ("invalid_frame", None, None)
        }
    };
    ServerFrame::Error {
        code: code.to_owned(),
        message: error.to_string(),
        active_policy_sha256: active,
        bounded_policy: policy,
    }
}

fn is_measurement_category(category: Option<&str>) -> bool {
    category.is_some_and(|category| {
        let category = category.to_ascii_lowercase();
        [
            "experiment",
            "measurement",
            "benchmark",
            "cold",
            "warm",
            "edit",
        ]
        .iter()
        .any(|marker| category.contains(marker))
    })
}

fn bounded_text(policy: &str) -> String {
    if policy.len() <= MAX_BOUNDED_POLICY_BYTES {
        return policy.to_owned();
    }
    let mut end = MAX_BOUNDED_POLICY_BYTES;
    while !policy.is_char_boundary(end) {
        end -= 1;
    }
    policy[..end].to_owned()
}

fn existing_volume_path(path: &std::path::Path) -> &std::path::Path {
    let mut candidate = path;
    while !candidate.exists() {
        let Some(parent) = candidate.parent() else {
            break;
        };
        candidate = parent;
    }
    candidate
}

fn bounded_error(error: &dyn std::fmt::Display) -> String {
    error.to_string().chars().take(512).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_refresh_error_carries_the_active_hash_and_bounded_policy() {
        let frame = protocol_error_frame(
            &ProtocolError::PolicyRefreshRequired {
                active: "a".repeat(64),
            },
            "canonical policy",
        );
        assert!(matches!(
            frame,
            ServerFrame::Error {
                ref code,
                active_policy_sha256: Some(ref active),
                bounded_policy: Some(ref policy),
                ..
            } if code == "policy_refresh_required"
                && active == &"a".repeat(64)
                && policy == "canonical policy"
        ));
    }
}
