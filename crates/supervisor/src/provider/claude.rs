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

/// Claude Code one-shot adapter using verbose `stream-json` output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClaudeAdapter {
    effort: String,
}

impl ClaudeAdapter {
    /// Creates an adapter with the explicit Claude effort CLI value.
    #[must_use]
    pub fn new(effort: impl Into<String>) -> Self {
        Self {
            effort: effort.into(),
        }
    }

    fn build(
        &self,
        attempt: &AttemptSpec,
        exact_session_id: Option<&str>,
        inherited_env: &[(String, String)],
    ) -> Result<CommandSpec, ProviderBuildError> {
        validate_attempt_provider(attempt, Provider::Claude)?;
        if let Some(session_id) = exact_session_id {
            validate_session_id(session_id)?;
        }

        let historical = inherited_env.iter().any(|(key, value)| {
            key.eq_ignore_ascii_case("GOVFOLIO_HISTORICAL_CONTRACT") && value == "1"
        });
        let mut args = ["-p", "--output-format", "stream-json", "--verbose"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if historical {
            args.extend([
                "--permission-mode".to_owned(),
                "dontAsk".to_owned(),
                "--allowedTools".to_owned(),
                "Read,Edit,Write,Glob,Grep,Bash(git status *),Bash(git diff *),Bash(git add *),Bash(git commit *),Bash(git rev-parse *),Bash(cargo *)".to_owned(),
            ]);
        } else {
            args.push("--dangerously-skip-permissions".to_owned());
        }
        args.push("--effort".to_owned());
        args.push(self.effort.clone());
        if let Some(model) = &attempt.provider.model {
            args.extend(["--model".to_owned(), model.clone()]);
        }
        if let Some(session_id) = exact_session_id {
            args.extend(["--resume".to_owned(), session_id.to_owned()]);
        }

        let sanitized = sanitize_environment(
            Provider::Claude,
            &attempt.lane_id,
            attempt.lane_fence,
            inherited_env,
        );
        Ok(command_spec(
            attempt,
            args,
            sanitized.env,
            sanitized.remove_env,
        ))
    }
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new("max")
    }
}

impl ProviderAdapter for ClaudeAdapter {
    fn provider(&self) -> Provider {
        Provider::Claude
    }

    fn build_fresh(
        &self,
        attempt: &AttemptSpec,
        inherited_env: &[(String, String)],
    ) -> Result<CommandSpec, ProviderBuildError> {
        self.build(attempt, None, inherited_env)
    }

    fn build_resume(
        &self,
        attempt: &AttemptSpec,
        exact_session_id: &str,
        inherited_env: &[(String, String)],
    ) -> Result<CommandSpec, ProviderBuildError> {
        self.build(attempt, Some(exact_session_id), inherited_env)
    }

    fn classifier(&self) -> Box<dyn EventClassifier> {
        Box::new(ClaudeClassifier::default())
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

/// Incremental Claude `stream-json` classifier.
#[derive(Debug, Default)]
pub struct ClaudeClassifier {
    stream: StreamState,
}

impl EventClassifier for ClaudeClassifier {
    fn observe_stdout_line(&mut self, line: &[u8]) {
        let Some(value) = self.stream.parse_line(line) else {
            return;
        };
        capture_session(&mut self.stream, &value);

        let Some(event_type) = value.get("type").and_then(Value::as_str) else {
            return;
        };
        if event_type != "result" {
            return;
        }

        let subtype = value
            .get("subtype")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let is_error = value
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(subtype != "success");
        if subtype == "success" && !is_error {
            self.stream.terminal = Some(terminal_success("result.success"));
        } else {
            self.stream.terminal = Some(terminal_error(
                format!("result.{subtype}"),
                &value,
                Some(subtype),
            ));
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

fn capture_session(stream: &mut StreamState, value: &Value) {
    if let Some(session_id) = value.get("session_id").and_then(Value::as_str) {
        stream.capture_session(session_id);
    }
}
