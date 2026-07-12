use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde_json::Value;

use super::environment::sanitize_environment;
use super::normalize::{StreamState, finish_stream, terminal_error, terminal_success};
use super::{
    EventClassifier, ProviderAdapter, ProviderBuildError, validate_attempt_provider,
    validate_session_id,
};
use crate::model::{AttemptSpec, CommandSpec, NormalizedResult, Provider};

/// Codex one-shot adapter using `exec --json` in `workspace-write` mode.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CodexAdapter;

impl CodexAdapter {
    fn build(
        attempt: &AttemptSpec,
        exact_session_id: Option<&str>,
        inherited_env: &[(String, String)],
    ) -> Result<CommandSpec, ProviderBuildError> {
        validate_attempt_provider(attempt, Provider::Codex)?;
        if let Some(session_id) = exact_session_id {
            validate_session_id(session_id)?;
        }

        let mut args = ["--ask-for-approval", "never", "--cd"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        args.push(attempt.worktree.to_string_lossy().into_owned());
        if let Some(model) = &attempt.provider.model {
            args.extend(["--model".to_owned(), model.clone()]);
        }
        let sanitized = sanitize_environment(
            Provider::Codex,
            &attempt.lane_id,
            attempt.lane_fence,
            inherited_env,
        );
        let historical = sanitized.env.iter().any(|(key, value)| {
            key.eq_ignore_ascii_case("GOVFOLIO_HISTORICAL_CONTRACT") && value == "1"
        });
        if let Some((_, bronze_root)) = sanitized
            .env
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("GOVFOLIO_BRONZE_ROOT"))
        {
            args.extend(["--add-dir".to_owned(), bronze_root.clone()]);
        }
        args.extend([
            "--config".to_owned(),
            "agents.max_depth=2".to_owned(),
            "--config".to_owned(),
            "agents.max_threads=6".to_owned(),
            "--config".to_owned(),
            "model_reasoning_effort=\"xhigh\"".to_owned(),
            "--config".to_owned(),
            format!("sandbox_workspace_write.network_access={}", !historical),
        ]);
        args.push("exec".to_owned());
        args.extend(
            ["--json", "--sandbox", "workspace-write", "--color", "never"]
                .into_iter()
                .map(str::to_owned),
        );
        if let Some(session_id) = exact_session_id {
            args.extend(["resume".to_owned(), session_id.to_owned()]);
        }
        args.push("-".to_owned());

        Ok(command_spec(
            attempt,
            args,
            sanitized.env,
            sanitized.remove_env,
        ))
    }
}

impl ProviderAdapter for CodexAdapter {
    fn provider(&self) -> Provider {
        Provider::Codex
    }

    fn build_fresh(
        &self,
        attempt: &AttemptSpec,
        inherited_env: &[(String, String)],
    ) -> Result<CommandSpec, ProviderBuildError> {
        Self::build(attempt, None, inherited_env)
    }

    fn build_resume(
        &self,
        attempt: &AttemptSpec,
        exact_session_id: &str,
        inherited_env: &[(String, String)],
    ) -> Result<CommandSpec, ProviderBuildError> {
        Self::build(attempt, Some(exact_session_id), inherited_env)
    }

    fn classifier(&self) -> Box<dyn EventClassifier> {
        Box::new(CodexClassifier::default())
    }
}

fn command_spec(
    attempt: &AttemptSpec,
    args: Vec<String>,
    env: Vec<(String, String)>,
    remove_env: Vec<String>,
) -> CommandSpec {
    CommandSpec {
        program: PathBuf::from(&attempt.provider.executable),
        args,
        cwd: attempt.worktree.clone(),
        stdin: attempt.prompt.as_bytes().to_vec(),
        env,
        remove_env,
    }
}

/// Incremental Codex JSONL event classifier.
#[derive(Debug, Default)]
pub struct CodexClassifier {
    stream: StreamState,
}

impl EventClassifier for CodexClassifier {
    fn observe_stdout_line(&mut self, line: &[u8]) {
        let Some(value) = self.stream.parse_line(line) else {
            return;
        };
        capture_thread(&mut self.stream, &value);

        let Some(event_type) = value.get("type").and_then(Value::as_str) else {
            return;
        };
        match event_type {
            "turn.completed" => {
                self.stream.terminal = Some(terminal_success("turn.completed"));
            }
            "turn.failed" => {
                self.stream.terminal = Some(terminal_error("turn.failed", &value, None));
            }
            "error" if !will_retry(&value) => {
                self.stream.terminal = Some(terminal_error("error", &value, None));
            }
            _ => {}
        }
    }

    fn finish(
        self: Box<Self>,
        exit_code: Option<i32>,
        bounded_stderr: &[u8],
        observed_at: DateTime<Utc>,
        operator_stopped: bool,
    ) -> NormalizedResult {
        finish_stream(
            self.stream,
            exit_code,
            bounded_stderr,
            observed_at,
            operator_stopped,
        )
    }
}

fn capture_thread(stream: &mut StreamState, value: &Value) {
    if let Some(thread_id) = value.get("thread_id").and_then(Value::as_str) {
        stream.capture_session(thread_id);
    }
}

fn will_retry(value: &Value) -> bool {
    value
        .get("will_retry")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}
