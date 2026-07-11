//! One-shot provider command construction and structured terminal classification.

mod claude;
mod codex;
mod environment;
mod normalize;

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::model::{AttemptSpec, CommandSpec, NormalizedResult, Provider};

pub use claude::{ClaudeAdapter, ClaudeClassifier};
pub use codex::{CodexAdapter, CodexClassifier};
pub use normalize::stable_error_hash;

/// Maximum stderr suffix considered when structured stdout never began.
pub const MAX_STDERR_CLASSIFIER_BYTES: usize = 8 * 1024;

/// Complete captured output used by the deterministic convenience classifier.
#[derive(Clone, Copy, Debug)]
pub struct ClassificationInput<'a> {
    pub stdout: &'a [u8],
    pub stderr: &'a [u8],
    pub exit_code: Option<i32>,
    pub observed_at: DateTime<Utc>,
    pub operator_stopped: bool,
}

/// Stateful, single-invocation classifier owned by the stdout reader task.
///
/// Only newline-delimited stdout is observed. Stderr is supplied once, at the
/// end, so it cannot override a structured stream that already began.
pub trait EventClassifier: Send {
    /// Observes one raw stdout line. The line may include its newline.
    fn observe_stdout_line(&mut self, line: &[u8]);

    /// Produces the normalized terminal result and consumes classifier state.
    fn finish(
        self: Box<Self>,
        exit_code: Option<i32>,
        bounded_stderr: &[u8],
        observed_at: DateTime<Utc>,
        operator_stopped: bool,
    ) -> NormalizedResult;
}

/// One-shot provider protocol. Implementations never spawn a process directly.
pub trait ProviderAdapter: Send + Sync {
    /// Provider represented by this adapter.
    fn provider(&self) -> Provider;

    /// Builds a fresh, non-resuming invocation.
    ///
    /// # Errors
    ///
    /// Returns an error when the attempt names a different provider.
    fn build_fresh(
        &self,
        attempt: &AttemptSpec,
        inherited_env: &[(String, String)],
    ) -> Result<CommandSpec, ProviderBuildError>;

    /// Builds an exact-session resume invocation.
    ///
    /// # Errors
    ///
    /// Returns an error for a provider mismatch or an empty session identifier.
    fn build_resume(
        &self,
        attempt: &AttemptSpec,
        exact_session_id: &str,
        inherited_env: &[(String, String)],
    ) -> Result<CommandSpec, ProviderBuildError>;

    /// Creates fresh classifier state for one provider invocation.
    fn classifier(&self) -> Box<dyn EventClassifier>;

    /// Classifies a complete transcript through the incremental interface.
    #[must_use]
    fn classify(&self, input: &ClassificationInput<'_>) -> NormalizedResult {
        let mut classifier = self.classifier();
        for line in input.stdout.split_inclusive(|byte| *byte == b'\n') {
            classifier.observe_stdout_line(line);
        }
        classifier.finish(
            input.exit_code,
            input.stderr,
            input.observed_at,
            input.operator_stopped,
        )
    }
}

/// Command-construction failures that must be handled before process spawn.
#[derive(Debug, Error)]
pub enum ProviderBuildError {
    #[error("{expected} adapter cannot build a {actual} attempt")]
    ProviderMismatch {
        expected: Provider,
        actual: Provider,
    },
    #[error("exact resume session identifier is empty")]
    EmptySessionId,
}

pub(crate) fn validate_attempt_provider(
    attempt: &AttemptSpec,
    expected: Provider,
) -> Result<(), ProviderBuildError> {
    let actual = attempt.provider.provider;
    if actual == expected {
        Ok(())
    } else {
        Err(ProviderBuildError::ProviderMismatch { expected, actual })
    }
}

pub(crate) fn validate_session_id(session_id: &str) -> Result<(), ProviderBuildError> {
    if session_id.trim().is_empty() {
        Err(ProviderBuildError::EmptySessionId)
    } else {
        Ok(())
    }
}
