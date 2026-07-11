use std::future::pending;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use chrono::Utc;
use thiserror::Error;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdout, Command};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinError;
use tokio::time::sleep;

use crate::artifacts::AttemptArtifacts;
use crate::model::{CommandSpec, NormalizedResult, ResultClass};
use crate::provider::{EventClassifier, MAX_STDERR_CLASSIFIER_BYTES};

const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(25);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessOutputPaths {
    pub events: PathBuf,
    pub stderr: PathBuf,
}

impl ProcessOutputPaths {
    #[must_use]
    pub fn in_directory(directory: impl AsRef<Path>) -> Self {
        Self {
            events: directory.as_ref().join("events.jsonl"),
            stderr: directory.as_ref().join("stderr.log"),
        }
    }
}

impl From<&AttemptArtifacts> for ProcessOutputPaths {
    fn from(attempt: &AttemptArtifacts) -> Self {
        Self {
            events: attempt.events_path(),
            stderr: attempt.stderr_path(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProcessRunner {
    stderr_tail_bytes: usize,
    poll_interval: Duration,
}

impl Default for ProcessRunner {
    fn default() -> Self {
        Self {
            stderr_tail_bytes: MAX_STDERR_CLASSIFIER_BYTES,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

impl ProcessRunner {
    #[must_use]
    pub fn new(stderr_tail_bytes: usize, poll_interval: Duration) -> Self {
        Self {
            stderr_tail_bytes: stderr_tail_bytes.min(MAX_STDERR_CLASSIFIER_BYTES),
            poll_interval: poll_interval.max(Duration::from_millis(1)),
        }
    }

    /// Runs one provider command in an owned operating-system process group.
    /// Stdout alone feeds the structured classifier; stderr is retained as a
    /// bounded suffix and supplied only when the classifier is finalized.
    ///
    /// # Errors
    ///
    /// Returns an error when artifact streams cannot be created or flushed,
    /// required pipes are unexpectedly absent, a capture task fails, or an
    /// owned process group cannot be terminated and reaped.
    pub async fn run(
        &self,
        specification: &CommandSpec,
        output: &ProcessOutputPaths,
        classifier: Box<dyn EventClassifier>,
        cancellation: ProcessCancellation,
    ) -> Result<ProcessExecution, ProcessError> {
        self.run_inner(specification, output, classifier, cancellation, None)
            .await
    }

    /// Runs one provider command and reports the owned child PID immediately
    /// after process-group attachment and before the child can perform work.
    pub async fn run_with_pid(
        &self,
        specification: &CommandSpec,
        output: &ProcessOutputPaths,
        classifier: Box<dyn EventClassifier>,
        cancellation: ProcessCancellation,
        pid_sender: oneshot::Sender<u32>,
    ) -> Result<ProcessExecution, ProcessError> {
        self.run_inner(
            specification,
            output,
            classifier,
            cancellation,
            Some(pid_sender),
        )
        .await
    }

    async fn run_inner(
        &self,
        specification: &CommandSpec,
        output: &ProcessOutputPaths,
        classifier: Box<dyn EventClassifier>,
        mut cancellation: ProcessCancellation,
        pid_sender: Option<oneshot::Sender<u32>>,
    ) -> Result<ProcessExecution, ProcessError> {
        let events_file = create_output_file(&output.events).await?;
        let stderr_file = create_output_file(&output.stderr).await?;
        let mut command = build_command(specification);
        let prepared_group = match PreparedProcessGroup::prepare(&mut command) {
            Ok(group) => group,
            Err(error) => {
                sync_empty_outputs(events_file, stderr_file).await?;
                return Ok(ProcessExecution::runner_config(
                    false,
                    format!("process-group setup failed ({:?})", error.kind()),
                ));
            }
        };

        let mut child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                sync_empty_outputs(events_file, stderr_file).await?;
                return Ok(ProcessExecution::spawn_failed(&error));
            }
        };
        let group = match prepared_group.attach(&child) {
            Ok(group) => group,
            Err(error) => {
                terminate_unowned_child(&mut child).await?;
                sync_empty_outputs(events_file, stderr_file).await?;
                return Ok(ProcessExecution::runner_config(
                    true,
                    format!("process-group attachment failed ({:?})", error.kind()),
                ));
            }
        };
        if let (Some(sender), Some(pid)) = (pid_sender, child.id()) {
            let _receiver_dropped = sender.send(pid);
        }

        let stdin = take_stdin(&mut child, &group).await?;
        let stdout = take_stdout(&mut child, &group).await?;
        let stderr = take_stderr(&mut child, &group).await?;
        let prompt = specification.stdin.clone();
        let stdin_task = tokio::spawn(write_stdin(stdin, prompt));
        let stdout_task = tokio::spawn(capture_stdout(stdout, events_file, classifier));
        let tail_limit = self.stderr_tail_bytes;
        let stderr_task = tokio::spawn(capture_stderr(stderr, stderr_file, tail_limit));

        let (status, operator_stopped) =
            wait_for_exit(&mut child, &group, &mut cancellation, self.poll_interval).await?;
        group.terminate_remaining()?;

        tolerate_closed_stdin(stdin_task.await?)?;
        let (classifier, stdout_bytes) = stdout_task.await??;
        let stderr_capture = stderr_task.await??;
        let exit_code = status.code();
        let result = classifier.finish(
            exit_code,
            &stderr_capture.tail,
            Utc::now(),
            operator_stopped,
        );

        Ok(ProcessExecution {
            result,
            spawned: true,
            exit_code,
            operator_stopped,
            stdout_bytes,
            stderr_bytes: stderr_capture.total_bytes,
            stderr_tail: stderr_capture.tail,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcessExecution {
    pub result: NormalizedResult,
    pub spawned: bool,
    pub exit_code: Option<i32>,
    pub operator_stopped: bool,
    pub stdout_bytes: u64,
    pub stderr_bytes: u64,
    pub stderr_tail: Vec<u8>,
}

impl ProcessExecution {
    fn spawn_failed(error: &io::Error) -> Self {
        Self {
            result: NormalizedResult::spawn_failed(format!(
                "provider process spawn failed ({:?})",
                error.kind()
            )),
            spawned: false,
            exit_code: None,
            operator_stopped: false,
            stdout_bytes: 0,
            stderr_bytes: 0,
            stderr_tail: Vec::new(),
        }
    }

    fn runner_config(spawned: bool, summary: String) -> Self {
        Self {
            result: NormalizedResult {
                class: ResultClass::RunnerConfig,
                terminal_type: None,
                structured_started: false,
                session_id: None,
                provider_error_code: None,
                stable_error_hash: None,
                retry_at: None,
                exit_code: None,
                summary,
            },
            spawned,
            exit_code: None,
            operator_stopped: false,
            stdout_bytes: 0,
            stderr_bytes: 0,
            stderr_tail: Vec::new(),
        }
    }
}

#[derive(Debug, Error)]
pub enum ProcessError {
    #[error("process artifact I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("process capture task failed: {0}")]
    TaskJoin(#[from] JoinError),
    #[error("spawned provider is missing its piped {0}")]
    MissingPipe(&'static str),
}

#[derive(Clone, Debug)]
pub struct ProcessCancelHandle {
    sender: watch::Sender<bool>,
}

impl ProcessCancelHandle {
    pub fn cancel(&self) {
        let _previous = self.sender.send_replace(true);
    }
}

#[derive(Debug)]
pub struct ProcessCancellation {
    receiver: watch::Receiver<bool>,
}

impl ProcessCancellation {
    async fn cancelled(&mut self) {
        loop {
            if *self.receiver.borrow() {
                return;
            }
            if self.receiver.changed().await.is_err() {
                pending::<()>().await;
            }
        }
    }
}

#[must_use]
pub fn cancellation_pair() -> (ProcessCancelHandle, ProcessCancellation) {
    let (sender, receiver) = watch::channel(false);
    (
        ProcessCancelHandle { sender },
        ProcessCancellation { receiver },
    )
}

fn build_command(specification: &CommandSpec) -> Command {
    let mut command = Command::new(&specification.program);
    command
        .args(&specification.args)
        .current_dir(&specification.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (name, value) in &specification.env {
        command.env(name, value);
    }
    for name in &specification.remove_env {
        command.env_remove(name);
    }
    command
}

async fn create_output_file(path: &Path) -> io::Result<File> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("process output path has no parent: {}", path.display()),
        )
    })?;
    if !parent.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "process output directory does not exist: {}",
                parent.display()
            ),
        ));
    }
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .await
}

async fn sync_empty_outputs(events: File, stderr: File) -> io::Result<()> {
    events.sync_all().await?;
    stderr.sync_all().await
}

async fn write_stdin(mut stdin: tokio::process::ChildStdin, prompt: Vec<u8>) -> io::Result<()> {
    stdin.write_all(&prompt).await?;
    stdin.shutdown().await
}

async fn capture_stdout(
    stdout: ChildStdout,
    mut file: File,
    mut classifier: Box<dyn EventClassifier>,
) -> io::Result<(Box<dyn EventClassifier>, u64)> {
    let mut reader = BufReader::new(stdout);
    let mut line = Vec::new();
    let mut total_bytes = 0_u64;
    loop {
        line.clear();
        let read = reader.read_until(b'\n', &mut line).await?;
        if read == 0 {
            break;
        }
        file.write_all(&line).await?;
        classifier.observe_stdout_line(&line);
        total_bytes = total_bytes.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }
    file.flush().await?;
    file.sync_all().await?;
    Ok((classifier, total_bytes))
}

async fn capture_stderr(
    mut stderr: ChildStderr,
    mut file: File,
    tail_limit: usize,
) -> io::Result<StderrCapture> {
    let mut buffer = [0_u8; 8 * 1024];
    let mut tail = TailBuffer::new(tail_limit);
    let mut total_bytes = 0_u64;
    loop {
        let read = stderr.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read]).await?;
        tail.push(&buffer[..read]);
        total_bytes = total_bytes.saturating_add(u64::try_from(read).unwrap_or(u64::MAX));
    }
    file.flush().await?;
    file.sync_all().await?;
    Ok(StderrCapture {
        tail: tail.into_bytes(),
        total_bytes,
    })
}

async fn wait_for_exit(
    child: &mut Child,
    group: &ProcessGroup,
    cancellation: &mut ProcessCancellation,
    poll_interval: Duration,
) -> io::Result<(ExitStatus, bool)> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((status, false));
        }
        tokio::select! {
            () = cancellation.cancelled() => {
                group.terminate()?;
                return child.wait().await.map(|status| (status, true));
            }
            () = sleep(poll_interval) => {}
        }
    }
}

async fn take_stdin(
    child: &mut Child,
    group: &ProcessGroup,
) -> Result<tokio::process::ChildStdin, ProcessError> {
    let Some(stdin) = child.stdin.take() else {
        terminate_started_child(child, group).await?;
        return Err(ProcessError::MissingPipe("stdin"));
    };
    Ok(stdin)
}

async fn take_stdout(child: &mut Child, group: &ProcessGroup) -> Result<ChildStdout, ProcessError> {
    let Some(stdout) = child.stdout.take() else {
        terminate_started_child(child, group).await?;
        return Err(ProcessError::MissingPipe("stdout"));
    };
    Ok(stdout)
}

async fn take_stderr(child: &mut Child, group: &ProcessGroup) -> Result<ChildStderr, ProcessError> {
    let Some(stderr) = child.stderr.take() else {
        terminate_started_child(child, group).await?;
        return Err(ProcessError::MissingPipe("stderr"));
    };
    Ok(stderr)
}

async fn terminate_started_child(child: &mut Child, group: &ProcessGroup) -> io::Result<()> {
    group.terminate()?;
    let _status = child.wait().await?;
    Ok(())
}

async fn terminate_unowned_child(child: &mut Child) -> io::Result<()> {
    match child.start_kill() {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => {}
        Err(error) => return Err(error),
    }
    let _status = child.wait().await?;
    Ok(())
}

fn tolerate_closed_stdin(result: io::Result<()>) -> io::Result<()> {
    match result {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::BrokenPipe
                    | io::ErrorKind::ConnectionAborted
                    | io::ErrorKind::ConnectionReset
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[derive(Debug)]
struct StderrCapture {
    tail: Vec<u8>,
    total_bytes: u64,
}

#[derive(Debug)]
struct TailBuffer {
    bytes: Vec<u8>,
    limit: usize,
}

impl TailBuffer {
    const fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    fn push(&mut self, bytes: &[u8]) {
        if self.limit == 0 {
            return;
        }
        if bytes.len() >= self.limit {
            self.bytes.clear();
            self.bytes
                .extend_from_slice(&bytes[bytes.len() - self.limit..]);
            return;
        }
        let excess = self
            .bytes
            .len()
            .saturating_add(bytes.len())
            .saturating_sub(self.limit);
        if excess > 0 {
            self.bytes.drain(..excess);
        }
        self.bytes.extend_from_slice(bytes);
    }

    fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(unix)]
#[derive(Debug)]
struct PreparedProcessGroup;

#[cfg(unix)]
impl PreparedProcessGroup {
    #[allow(clippy::unnecessary_wraps)] // signature matches fallible Windows Job setup
    fn prepare(command: &mut Command) -> io::Result<Self> {
        command.process_group(0);
        Ok(Self)
    }

    fn attach(self, child: &Child) -> io::Result<ProcessGroup> {
        let pid = child.id().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "spawned provider has no process identifier",
            )
        })?;
        let group_id = i32::try_from(pid).map_err(io::Error::other)?;
        Ok(ProcessGroup { pgid: group_id })
    }
}

#[cfg(unix)]
#[derive(Debug)]
struct ProcessGroup {
    pgid: i32,
}

#[cfg(unix)]
impl ProcessGroup {
    fn terminate(&self) -> io::Result<()> {
        // SAFETY: the child creates a new group whose ID is the validated child
        // PID. A negative PID addresses that process group only.
        let result = unsafe { libc::kill(-self.pgid, libc::SIGKILL) };
        if result == 0 {
            Ok(())
        } else {
            let error = io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(())
            } else {
                Err(error)
            }
        }
    }

    fn terminate_remaining(&self) -> io::Result<()> {
        self.terminate()
    }
}

#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        let _terminated = self.terminate();
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct PreparedProcessGroup {
    job: WindowsJob,
}

#[cfg(windows)]
impl PreparedProcessGroup {
    fn prepare(command: &mut Command) -> io::Result<Self> {
        use std::os::windows::process::CommandExt;

        use windows_sys::Win32::System::Threading::CREATE_SUSPENDED;

        command.as_std_mut().creation_flags(CREATE_SUSPENDED);
        Ok(Self {
            job: WindowsJob::new()?,
        })
    }

    fn attach(self, child: &Child) -> io::Result<ProcessGroup> {
        self.job.assign(child)?;
        resume_suspended_process(child)?;
        Ok(ProcessGroup { job: self.job })
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct ProcessGroup {
    job: WindowsJob,
}

#[cfg(windows)]
impl ProcessGroup {
    fn terminate(&self) -> io::Result<()> {
        self.job.terminate()
    }

    fn terminate_remaining(&self) -> io::Result<()> {
        self.job.terminate()
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct WindowsJob {
    handle: usize,
}

#[cfg(windows)]
impl WindowsJob {
    fn new() -> io::Result<Self> {
        use std::ffi::c_void;

        use windows_sys::Win32::System::JobObjects::{
            CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
            SetInformationJobObject,
        };

        // SAFETY: null attributes and name request a private job with default
        // security. The returned owned handle is closed in `Drop`.
        let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        let job = Self {
            handle: handle as usize,
        };
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let limits_size =
            u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                .map_err(io::Error::other)?;
        // SAFETY: `limits` matches the declared information class and remains
        // alive for the duration of this synchronous call.
        let succeeded = unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast::<c_void>(),
                limits_size,
            )
        };
        if succeeded == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(job)
        }
    }

    fn assign(&self, child: &Child) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::AssignProcessToJobObject;

        let process = child.raw_handle().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "spawned provider has no process handle",
            )
        })?;
        // SAFETY: both handles are live; the child remains suspended until the
        // assignment succeeds, so it cannot create an unowned descendant.
        let succeeded = unsafe { AssignProcessToJobObject(self.raw(), process.cast()) };
        if succeeded == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn terminate(&self) -> io::Result<()> {
        use windows_sys::Win32::System::JobObjects::TerminateJobObject;

        // SAFETY: the job handle is owned and remains live for the call.
        let succeeded = unsafe { TerminateJobObject(self.raw(), 1) };
        if succeeded == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    fn raw(&self) -> windows_sys::Win32::Foundation::HANDLE {
        self.handle as windows_sys::Win32::Foundation::HANDLE
    }
}

#[cfg(windows)]
impl Drop for WindowsJob {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::CloseHandle;

        // SAFETY: `handle` is uniquely owned by this value and closed once.
        let _closed = unsafe { CloseHandle(self.raw()) };
    }
}

#[cfg(windows)]
fn resume_suspended_process(child: &Child) -> io::Result<()> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
    };
    use windows_sys::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};

    let pid = child.id().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "suspended provider has no process identifier",
        )
    })?;
    // SAFETY: the snapshot call has no pointer arguments and its owned handle
    // is closed on every return path below.
    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    let result = (|| {
        let entry_size =
            u32::try_from(std::mem::size_of::<THREADENTRY32>()).map_err(io::Error::other)?;
        let mut entry = THREADENTRY32 {
            dwSize: entry_size,
            ..THREADENTRY32::default()
        };
        // SAFETY: `entry` has the required size and remains writable.
        let mut has_entry = unsafe { Thread32First(snapshot, &raw mut entry) } != 0;
        while has_entry {
            if entry.th32OwnerProcessID == pid {
                // SAFETY: the enumerated thread ID belongs to the still-live,
                // suspended child. The returned handle is closed below.
                let thread = unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) };
                if thread.is_null() {
                    return Err(io::Error::last_os_error());
                }
                // SAFETY: `thread` grants suspend/resume access and is live.
                let previous_count = unsafe { ResumeThread(thread) };
                // SAFETY: `thread` is uniquely owned on this path.
                let _closed = unsafe { CloseHandle(thread) };
                if previous_count == u32::MAX {
                    return Err(io::Error::last_os_error());
                }
                return Ok(());
            }
            // SAFETY: `snapshot` and `entry` remain valid across enumeration.
            has_entry = unsafe { Thread32Next(snapshot, &raw mut entry) } != 0;
        }
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "suspended provider thread was not found",
        ))
    })();
    // SAFETY: `snapshot` is uniquely owned and is closed once.
    let _closed = unsafe { CloseHandle(snapshot) };
    result
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use chrono::{DateTime, Utc};
    use tempfile::tempdir;
    use tokio::time::{sleep, timeout};

    use super::{ProcessOutputPaths, ProcessRunner, cancellation_pair};
    use crate::model::{CommandSpec, NormalizedResult, ResultClass};
    use crate::provider::EventClassifier;

    #[tokio::test]
    async fn process_should_pipe_stdin_and_keep_stdout_separate_from_stderr() {
        let temp = tempdir().expect("tempdir should be available");
        let output = output_paths(temp.path());
        let specification = shell_command(
            temp.path(),
            "$text = [Console]::In.ReadToEnd(); [Console]::Out.WriteLine('terminal-complete:' + $text.Trim()); [Console]::Error.WriteLine('stderr-marker')",
            "IFS= read -r line; printf 'terminal-complete:%s\\n' \"$line\"; printf 'stderr-marker\\n' >&2",
            b"prompt-marker\n",
        );
        let (_cancel, cancellation) = cancellation_pair();

        let execution = ProcessRunner::default()
            .run(
                &specification,
                &output,
                Box::new(TestClassifier::default()),
                cancellation,
            )
            .await
            .expect("process should run");

        assert_eq!(execution.result.class, ResultClass::Completed);
        let events = fs::read_to_string(&output.events).expect("events should exist");
        let stderr = fs::read_to_string(&output.stderr).expect("stderr should exist");
        assert!(events.contains("terminal-complete:prompt-marker"));
        assert!(!events.contains("stderr-marker"));
        assert!(stderr.contains("stderr-marker"));
    }

    #[tokio::test]
    async fn process_should_report_the_owned_child_pid() {
        let temp = tempdir().expect("tempdir should be available");
        let output = output_paths(temp.path());
        let specification = shell_command(
            temp.path(),
            "[Console]::Out.WriteLine('terminal-complete')",
            "printf 'terminal-complete\\n'",
            b"",
        );
        let (_cancel, cancellation) = cancellation_pair();
        let (pid_sender, pid_receiver) = tokio::sync::oneshot::channel();
        let runner = ProcessRunner::default();
        let execution = runner.run_with_pid(
            &specification,
            &output,
            Box::new(TestClassifier::default()),
            cancellation,
            pid_sender,
        );
        let (execution, pid) = tokio::join!(execution, pid_receiver);

        assert!(pid.expect("attached child should report a PID") > 0);
        assert_eq!(
            execution.expect("process should run").result.class,
            ResultClass::Completed
        );
    }

    #[tokio::test]
    async fn process_should_bound_the_stderr_classifier_tail() {
        let temp = tempdir().expect("tempdir should be available");
        let output = output_paths(temp.path());
        let specification = shell_command(
            temp.path(),
            "[Console]::Out.WriteLine('terminal-complete'); [Console]::Error.WriteLine('0123456789ABCDEF')",
            "printf 'terminal-complete\\n'; printf '0123456789ABCDEF\\n' >&2",
            b"",
        );
        let (_cancel, cancellation) = cancellation_pair();

        let execution = ProcessRunner::new(8, Duration::from_millis(5))
            .run(
                &specification,
                &output,
                Box::new(TestClassifier::default()),
                cancellation,
            )
            .await
            .expect("process should run");

        assert!(execution.stderr_tail.len() <= 8);
        assert!(
            String::from_utf8_lossy(&execution.stderr_tail)
                .trim_end()
                .ends_with("ABCDEF")
        );
    }

    #[tokio::test]
    async fn missing_program_should_be_a_normalized_spawn_failure() {
        let temp = tempdir().expect("tempdir should be available");
        let output = output_paths(temp.path());
        let specification = CommandSpec {
            program: temp.path().join("definitely-missing-provider"),
            args: Vec::new(),
            cwd: temp.path().to_path_buf(),
            stdin: Vec::new(),
            env: Vec::new(),
            remove_env: Vec::new(),
        };
        let (_cancel, cancellation) = cancellation_pair();

        let execution = ProcessRunner::default()
            .run(
                &specification,
                &output,
                Box::new(TestClassifier::default()),
                cancellation,
            )
            .await
            .expect("spawn failure should be data");

        assert_eq!(execution.result.class, ResultClass::SpawnFailed);
        assert!(!execution.spawned);
        assert!(output.events.exists());
        assert!(output.stderr.exists());
    }

    #[tokio::test]
    async fn completed_structured_terminal_should_survive_nonzero_exit() {
        let temp = tempdir().expect("tempdir should be available");
        let output = output_paths(temp.path());
        let specification = shell_command(
            temp.path(),
            "[Console]::Out.WriteLine('terminal-complete'); exit 7",
            "printf 'terminal-complete\\n'; exit 7",
            b"",
        );
        let (_cancel, cancellation) = cancellation_pair();

        let execution = ProcessRunner::default()
            .run(
                &specification,
                &output,
                Box::new(TestClassifier::default()),
                cancellation,
            )
            .await
            .expect("process should run");

        assert_eq!(execution.exit_code, Some(7));
        assert_eq!(execution.result.class, ResultClass::Completed);
    }

    #[tokio::test]
    async fn cancellation_should_kill_and_reap_the_process_tree() {
        let temp = tempdir().expect("tempdir should be available");
        let sentinel = temp.path().join("escaped-child.txt");
        let output = output_paths(temp.path());
        let specification = tree_command(temp.path(), &sentinel);
        let (cancel, cancellation) = cancellation_pair();
        let task_output = output.clone();
        let task = tokio::spawn(async move {
            ProcessRunner::default()
                .run(
                    &specification,
                    &task_output,
                    Box::new(TestClassifier::default()),
                    cancellation,
                )
                .await
        });
        wait_for_text(&output.events, "ready").await;

        cancel.cancel();
        let execution = timeout(Duration::from_secs(5), task)
            .await
            .expect("cancelled group should stop promptly")
            .expect("process task should not panic")
            .expect("process cancellation should be clean");
        sleep(Duration::from_millis(1_300)).await;

        assert!(execution.operator_stopped);
        assert_eq!(execution.result.class, ResultClass::OperatorStop);
        assert!(!sentinel.exists());
    }

    #[derive(Default)]
    struct TestClassifier {
        structured_started: bool,
        completed: bool,
    }

    impl EventClassifier for TestClassifier {
        fn observe_stdout_line(&mut self, line: &[u8]) {
            if !line.is_empty() {
                self.structured_started = true;
            }
            if line
                .windows(b"terminal-complete".len())
                .any(|window| window == b"terminal-complete")
            {
                self.completed = true;
            }
        }

        fn finish(
            self: Box<Self>,
            exit_code: Option<i32>,
            bounded_stderr: &[u8],
            _observed_at: DateTime<Utc>,
            operator_stopped: bool,
        ) -> NormalizedResult {
            let class = if self.completed {
                ResultClass::Completed
            } else if operator_stopped {
                ResultClass::OperatorStop
            } else {
                ResultClass::Ambiguous
            };
            NormalizedResult {
                class,
                terminal_type: self.completed.then(|| "test_terminal".to_owned()),
                structured_started: self.structured_started,
                session_id: None,
                provider_error_code: None,
                stable_error_hash: None,
                retry_at: None,
                exit_code,
                summary: String::from_utf8_lossy(bounded_stderr).into_owned(),
            }
        }
    }

    fn output_paths(directory: &Path) -> ProcessOutputPaths {
        ProcessOutputPaths::in_directory(directory)
    }

    #[cfg(windows)]
    fn shell_command(
        cwd: &Path,
        windows_script: &str,
        _unix_script: &str,
        stdin: &[u8],
    ) -> CommandSpec {
        CommandSpec {
            program: PathBuf::from("powershell.exe"),
            args: vec![
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-Command".to_owned(),
                windows_script.to_owned(),
            ],
            cwd: cwd.to_path_buf(),
            stdin: stdin.to_vec(),
            env: Vec::new(),
            remove_env: Vec::new(),
        }
    }

    #[cfg(unix)]
    fn shell_command(
        cwd: &Path,
        _windows_script: &str,
        unix_script: &str,
        stdin: &[u8],
    ) -> CommandSpec {
        CommandSpec {
            program: PathBuf::from("/bin/sh"),
            args: vec!["-c".to_owned(), unix_script.to_owned()],
            cwd: cwd.to_path_buf(),
            stdin: stdin.to_vec(),
            env: Vec::new(),
            remove_env: Vec::new(),
        }
    }

    #[cfg(windows)]
    fn tree_command(cwd: &Path, sentinel: &Path) -> CommandSpec {
        let script = "$job = Start-Job -ScriptBlock { param($path) Start-Sleep -Milliseconds 1000; [System.IO.File]::WriteAllText($path, 'leaked') } -ArgumentList $args[0]; [Console]::Out.WriteLine('ready'); Wait-Job -Job $job | Out-Null";
        let script_path = cwd.join("process-tree-helper.ps1");
        fs::write(&script_path, script).expect("process helper should be writable");
        CommandSpec {
            program: PathBuf::from("powershell.exe"),
            args: vec![
                "-NoProfile".to_owned(),
                "-NonInteractive".to_owned(),
                "-File".to_owned(),
                script_path.display().to_string(),
                sentinel.display().to_string(),
            ],
            cwd: cwd.to_path_buf(),
            stdin: Vec::new(),
            env: Vec::new(),
            remove_env: Vec::new(),
        }
    }

    #[cfg(unix)]
    fn tree_command(cwd: &Path, sentinel: &Path) -> CommandSpec {
        CommandSpec {
            program: PathBuf::from("/bin/sh"),
            args: vec![
                "-c".to_owned(),
                "(sleep 1; printf leaked > \"$1\") & printf 'ready\\n'; sleep 10".to_owned(),
                "govfolio-process-test".to_owned(),
                sentinel.display().to_string(),
            ],
            cwd: cwd.to_path_buf(),
            stdin: Vec::new(),
            env: Vec::new(),
            remove_env: Vec::new(),
        }
    }

    async fn wait_for_text(path: &Path, expected: &str) {
        timeout(Duration::from_secs(5), async {
            loop {
                if fs::read_to_string(path).is_ok_and(|text| text.contains(expected)) {
                    return;
                }
                sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("provider should emit readiness marker");
    }
}
