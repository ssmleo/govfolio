#![allow(clippy::expect_used)]

use std::path::PathBuf;

use chrono::{TimeZone, Utc};
use loop_supervisor::model::{AttemptSpec, PromptKind, Provider, ProviderIdentity, ResultClass};
use loop_supervisor::provider::{
    ClassificationInput, ClaudeAdapter, CodexAdapter, ProviderAdapter, stable_error_hash,
};

const OBSERVED_AT_SECONDS: i64 = 1_783_739_200;

fn fixture(name: &str) -> &'static [u8] {
    match name {
        "claude_success" => include_bytes!("fixtures/provider/claude_success.jsonl"),
        "claude_quota_reset" => {
            include_bytes!("fixtures/provider/claude_quota_reset.jsonl")
        }
        "claude_monthly_usage" => {
            include_bytes!("fixtures/provider/claude_monthly_usage.jsonl")
        }
        "claude_rate_limit" => include_bytes!("fixtures/provider/claude_rate_limit.jsonl"),
        "claude_auth" => include_bytes!("fixtures/provider/claude_auth.jsonl"),
        "claude_transport" => include_bytes!("fixtures/provider/claude_transport.jsonl"),
        "claude_terminal_failure" => {
            include_bytes!("fixtures/provider/claude_terminal_failure.jsonl")
        }
        "claude_truncated" => include_bytes!("fixtures/provider/claude_truncated.jsonl"),
        "claude_corrupt" => include_bytes!("fixtures/provider/claude_corrupt.jsonl"),
        "claude_completed_cleanup" => {
            include_bytes!("fixtures/provider/claude_completed_cleanup.jsonl")
        }
        "codex_success" => include_bytes!("fixtures/provider/codex_success.jsonl"),
        "codex_quota_reset" => include_bytes!("fixtures/provider/codex_quota_reset.jsonl"),
        "codex_monthly_usage" => {
            include_bytes!("fixtures/provider/codex_monthly_usage.jsonl")
        }
        "codex_rate_limit" => include_bytes!("fixtures/provider/codex_rate_limit.jsonl"),
        "codex_auth" => include_bytes!("fixtures/provider/codex_auth.jsonl"),
        "codex_transport" => include_bytes!("fixtures/provider/codex_transport.jsonl"),
        "codex_terminal_failure" => {
            include_bytes!("fixtures/provider/codex_terminal_failure.jsonl")
        }
        "codex_truncated" => include_bytes!("fixtures/provider/codex_truncated.jsonl"),
        "codex_corrupt" => include_bytes!("fixtures/provider/codex_corrupt.jsonl"),
        "codex_completed_cleanup" => {
            include_bytes!("fixtures/provider/codex_completed_cleanup.jsonl")
        }
        unknown => panic!("unknown provider fixture {unknown}"),
    }
}

fn input(stdout: &'static [u8], exit_code: i32) -> ClassificationInput<'static> {
    ClassificationInput {
        stdout,
        stderr: &[],
        exit_code: Some(exit_code),
        observed_at: Utc
            .timestamp_opt(OBSERVED_AT_SECONDS, 0)
            .single()
            .expect("test timestamp is valid"),
        operator_stopped: false,
    }
}

fn attempt(provider: Provider, model: Option<&str>) -> AttemptSpec {
    AttemptSpec {
        id: "attempt-1".to_owned(),
        lane_id: "orchestrator-0".to_owned(),
        lane_fence: 7,
        work_key: "goal-108".to_owned(),
        worktree: PathBuf::from("C:/worktrees/orchestrator-0"),
        expected_branch: "goal/108".to_owned(),
        prompt: "Perform exactly one bounded phase.".to_owned(),
        prompt_kind: PromptKind::Normal,
        provider: ProviderIdentity {
            provider,
            executable: PathBuf::from(match provider {
                Provider::Claude => "C:/tools/claude.exe",
                Provider::Codex => "C:/tools/codex.exe",
            }),
            cli_version: "test-cli".to_owned(),
            model: model.map(str::to_owned),
            config_fingerprint: "config-v1".to_owned(),
        },
        resume_session_id: None,
        preflight_signature: "preflight-v1".to_owned(),
        git_head_before: "0123456789abcdef".to_owned(),
        journal_sha_before: "fedcba9876543210".to_owned(),
    }
}

fn inherited_environment() -> Vec<(String, String)> {
    [
        ("PATH", "C:/tools"),
        ("USERPROFILE", "C:/Users/loop"),
        ("DATABASE_URL", "postgres://local"),
        ("GOVFOLIO_BRONZE_ROOT", "C:/bronze"),
        ("CARGO_TARGET_DIR", "C:/targets/lane-0"),
        ("ANTHROPIC_API_KEY", "anthropic-secret"),
        ("CLAUDE_CODE_OAUTH_TOKEN", "claude-secret"),
        ("OPENAI_API_KEY", "openai-secret"),
        ("CODEX_API_KEY", "codex-secret"),
        ("CODEX_HOME", "C:/Users/loop/.codex"),
        ("GITHUB_TOKEN", "must-not-leak"),
        ("GIT_DIR", "must-not-redirect"),
        ("RUSTFLAGS", "must-not-inject"),
        ("CLAUDE_CODE_EFFORT_LEVEL", "low"),
        ("CODEX_THREAD_ID", "wrong-thread"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_owned(), value.to_owned()))
    .collect()
}

#[test]
fn provider_claude_fresh_command_uses_stream_json_and_stdin_prompt() {
    let attempt = attempt(Provider::Claude, Some("claude-opus-4-6"));
    let command = ClaudeAdapter::new("max")
        .build_fresh(&attempt, &inherited_environment())
        .expect("matching provider builds");

    assert_eq!(
        command.args,
        [
            "-p",
            "--output-format",
            "stream-json",
            "--verbose",
            "--dangerously-skip-permissions",
            "--effort",
            "max",
            "--model",
            "claude-opus-4-6",
        ]
    );
    assert_eq!(command.stdin, attempt.prompt.as_bytes());
    assert!(!command.args.iter().any(|arg| arg == &attempt.prompt));
}

#[test]
fn provider_claude_resume_command_names_the_exact_session() {
    let attempt = attempt(Provider::Claude, None);
    let command = ClaudeAdapter::default()
        .build_resume(&attempt, "claude-session-exact", &inherited_environment())
        .expect("nonempty exact session builds");

    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--resume", "claude-session-exact"])
    );
    assert!(!command.args.iter().any(|arg| arg == "--continue"));
}

#[test]
fn provider_codex_fresh_command_enforces_workspace_write_and_never_approves() {
    let attempt = attempt(Provider::Codex, Some("gpt-5.4"));
    let command = CodexAdapter
        .build_fresh(&attempt, &inherited_environment())
        .expect("matching provider builds");

    assert_eq!(
        command.args,
        [
            "--ask-for-approval",
            "never",
            "--cd",
            "C:/worktrees/orchestrator-0",
            "--model",
            "gpt-5.4",
            "--add-dir",
            "C:/bronze",
            "--config",
            "agents.max_depth=2",
            "--config",
            "agents.max_threads=6",
            "--config",
            "model_reasoning_effort=\"xhigh\"",
            "--config",
            "sandbox_workspace_write.network_access=true",
            "exec",
            "--json",
            "--sandbox",
            "workspace-write",
            "--color",
            "never",
            "-",
        ]
    );
    assert!(!command.args.iter().any(|arg| arg == "--full-auto"));
    assert!(
        !command
            .args
            .iter()
            .any(|arg| arg == "--dangerously-bypass-approvals-and-sandbox")
    );
    assert_eq!(command.stdin, attempt.prompt.as_bytes());
    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--config", "agents.max_depth=2"])
    );
    assert!(
        command
            .args
            .windows(2)
            .any(|pair| pair == ["--config", "agents.max_threads=6"])
    );
}

#[test]
fn provider_codex_resume_command_names_the_exact_thread() {
    let attempt = attempt(Provider::Codex, None);
    let command = CodexAdapter
        .build_resume(&attempt, "019f-thread-exact", &inherited_environment())
        .expect("nonempty exact thread builds");

    assert_eq!(
        &command.args[command.args.len() - 3..],
        ["resume", "019f-thread-exact", "-"]
    );
    let json_index = command
        .args
        .iter()
        .position(|arg| arg == "--json")
        .expect("JSON mode is present");
    let resume_index = command
        .args
        .iter()
        .position(|arg| arg == "resume")
        .expect("resume subcommand is present");
    assert!(json_index < resume_index);
    assert!(!command.args.iter().any(|arg| arg == "--last"));
}

#[test]
fn provider_command_rejects_mismatched_adapter_and_empty_resume_id() {
    let codex_attempt = attempt(Provider::Codex, None);
    let claude_attempt = attempt(Provider::Claude, None);

    assert!(
        ClaudeAdapter::default()
            .build_fresh(&codex_attempt, &[])
            .is_err()
    );
    assert!(CodexAdapter.build_resume(&claude_attempt, "", &[]).is_err());
}

#[test]
fn provider_environment_is_allowlisted_and_provider_scoped() {
    let source = inherited_environment();
    let claude = ClaudeAdapter::default()
        .build_fresh(&attempt(Provider::Claude, None), &source)
        .expect("claude command builds");
    let codex = CodexAdapter
        .build_fresh(&attempt(Provider::Codex, None), &source)
        .expect("codex command builds");

    assert!(!claude.env.iter().any(|(key, _)| key == "CODEX_HOME"));
    assert!(codex.env.iter().any(|(key, _)| key == "CODEX_HOME"));
    assert!(
        claude
            .env
            .iter()
            .any(|(key, value)| { key == "GOVFOLIO_LANE_ID" && value == "orchestrator-0" })
    );
    for command in [&claude, &codex] {
        for (key, value) in [
            ("GIT_CONFIG_NOSYSTEM", "1"),
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "credential.helper"),
            ("GIT_CONFIG_VALUE_0", ""),
            ("GIT_TERMINAL_PROMPT", "0"),
            ("GCM_INTERACTIVE", "Never"),
        ] {
            assert!(
                command
                    .env
                    .iter()
                    .any(|(actual_key, actual_value)| actual_key == key && actual_value == value),
                "missing protected environment {key}"
            );
        }
        assert!(command.env.iter().any(|(key, _)| key == "GH_CONFIG_DIR"));
        assert!(
            command
                .env
                .iter()
                .any(|(key, _)| key == "GIT_CONFIG_GLOBAL")
        );
        assert!(command.env.iter().all(|(key, _)| {
            !command
                .remove_env
                .iter()
                .any(|removed| removed.eq_ignore_ascii_case(key))
        }));
    }
    for blocked in [
        "GITHUB_TOKEN",
        "GIT_DIR",
        "RUSTFLAGS",
        "CLAUDE_CODE_EFFORT_LEVEL",
        "CODEX_THREAD_ID",
        "ANTHROPIC_API_KEY",
        "CLAUDE_CODE_OAUTH_TOKEN",
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
    ] {
        assert!(claude.remove_env.iter().any(|key| key == blocked));
        assert!(codex.remove_env.iter().any(|key| key == blocked));
    }
}

#[test]
fn provider_claude_success_captures_session_and_ignores_agent_text() {
    let result = ClaudeAdapter::default().classify(&input(fixture("claude_success"), 0));

    assert_eq!(result.class, ResultClass::Completed);
    assert_eq!(result.session_id.as_deref(), Some("claude-session-1"));
    assert_eq!(result.terminal_type.as_deref(), Some("result.success"));
}

#[test]
fn provider_codex_success_captures_thread_and_ignores_agent_text() {
    let result = CodexAdapter.classify(&input(fixture("codex_success"), 0));

    assert_eq!(result.class, ResultClass::Completed);
    assert_eq!(result.session_id.as_deref(), Some("019f-thread-1"));
    assert_eq!(result.terminal_type.as_deref(), Some("turn.completed"));
}

#[test]
fn provider_claude_quota_and_monthly_usage_are_terminal_with_resets() {
    let quota = ClaudeAdapter::default().classify(&input(fixture("claude_quota_reset"), 1));
    let monthly = ClaudeAdapter::default().classify(&input(fixture("claude_monthly_usage"), 1));

    assert_eq!(quota.class, ResultClass::QuotaExhausted);
    assert_eq!(monthly.class, ResultClass::QuotaExhausted);
    assert_eq!(
        quota.retry_at,
        Some(
            Utc.with_ymd_and_hms(2026, 7, 12, 3, 0, 0)
                .single()
                .expect("fixture reset is valid")
        )
    );
    assert_eq!(
        monthly.retry_at,
        Some(
            Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0)
                .single()
                .expect("fixture reset is valid")
        )
    );
}

#[test]
fn provider_codex_quota_and_monthly_usage_are_terminal_with_resets() {
    let quota = CodexAdapter.classify(&input(fixture("codex_quota_reset"), 1));
    let monthly = CodexAdapter.classify(&input(fixture("codex_monthly_usage"), 1));

    assert_eq!(quota.class, ResultClass::QuotaExhausted);
    assert_eq!(monthly.class, ResultClass::QuotaExhausted);
    assert_eq!(
        quota.retry_at,
        Some(
            Utc.with_ymd_and_hms(2026, 7, 12, 4, 30, 0)
                .single()
                .expect("fixture reset is valid")
        )
    );
    assert_eq!(
        monthly.retry_at,
        Some(
            Utc.with_ymd_and_hms(2026, 8, 1, 0, 0, 0)
                .single()
                .expect("fixture reset is valid")
        )
    );
}

#[test]
fn provider_retry_after_is_relative_to_observation_time() {
    let claude = ClaudeAdapter::default().classify(&input(fixture("claude_rate_limit"), 1));
    let codex = CodexAdapter.classify(&input(fixture("codex_rate_limit"), 1));
    let observed_at = Utc
        .timestamp_opt(OBSERVED_AT_SECONDS, 0)
        .single()
        .expect("test timestamp is valid");

    assert_eq!(claude.class, ResultClass::RateLimited);
    assert_eq!(codex.class, ResultClass::RateLimited);
    assert_eq!(
        claude.retry_at,
        Some(observed_at + chrono::Duration::seconds(90))
    );
    assert_eq!(
        codex.retry_at,
        Some(observed_at + chrono::Duration::seconds(45))
    );
}

#[test]
fn provider_auth_transport_and_terminal_failures_are_normalized() {
    let claude = ClaudeAdapter::default();
    let codex = CodexAdapter;
    let cases: [(ResultClass, &dyn ProviderAdapter, &str); 6] = [
        (ResultClass::Auth, &claude, "claude_auth"),
        (ResultClass::TransientTransport, &claude, "claude_transport"),
        (ResultClass::Policy, &claude, "claude_terminal_failure"),
        (ResultClass::Auth, &codex, "codex_auth"),
        (ResultClass::TransientTransport, &codex, "codex_transport"),
        (ResultClass::RunnerConfig, &codex, "codex_terminal_failure"),
    ];

    for (expected, classifier, fixture_name) in cases {
        let result = classifier.classify(&input(fixture(fixture_name), 1));
        assert_eq!(result.class, expected, "fixture {fixture_name}");
        assert!(result.stable_error_hash.is_some(), "fixture {fixture_name}");
    }
}

#[test]
fn provider_corrupt_and_truncated_structured_streams_are_ambiguous() {
    let claude = ClaudeAdapter::default();
    let codex = CodexAdapter;
    for (classifier, fixture_name) in [
        (&claude as &dyn ProviderAdapter, "claude_truncated"),
        (&claude, "claude_corrupt"),
        (&codex as &dyn ProviderAdapter, "codex_truncated"),
        (&codex, "codex_corrupt"),
    ] {
        let result = classifier.classify(&input(fixture(fixture_name), 1));
        assert_eq!(
            result.class,
            ResultClass::Ambiguous,
            "fixture {fixture_name}"
        );
        assert!(result.structured_started, "fixture {fixture_name}");
    }
}

#[test]
fn provider_completed_terminal_wins_over_nonzero_cleanup_exit() {
    let claude = ClaudeAdapter::default().classify(&input(fixture("claude_completed_cleanup"), 37));
    let codex = CodexAdapter.classify(&input(fixture("codex_completed_cleanup"), 37));

    assert_eq!(claude.class, ResultClass::Completed);
    assert_eq!(codex.class, ResultClass::Completed);
    assert_eq!(claude.exit_code, Some(37));
    assert_eq!(codex.exit_code, Some(37));
}

#[test]
fn provider_stderr_fallback_is_bounded_and_only_used_before_structured_stdout() {
    let observed_at = Utc
        .timestamp_opt(OBSERVED_AT_SECONDS, 0)
        .single()
        .expect("test timestamp is valid");
    let stderr = format!(
        "{}rate limit exceeded; retry after 30 seconds",
        "x".repeat(20_000)
    );
    let fallback = CodexAdapter.classify(&ClassificationInput {
        stdout: &[],
        stderr: stderr.as_bytes(),
        exit_code: Some(1),
        observed_at,
        operator_stopped: false,
    });
    let structured = CodexAdapter.classify(&ClassificationInput {
        stdout: fixture("codex_truncated"),
        stderr: b"authentication failed: invalid API key",
        exit_code: Some(1),
        observed_at,
        operator_stopped: false,
    });

    assert_eq!(fallback.class, ResultClass::RateLimited);
    assert!(fallback.summary.len() <= 512);
    assert_eq!(structured.class, ResultClass::Ambiguous);
}

#[test]
fn provider_explicit_operator_stop_has_precedence_and_no_error_hash() {
    let mut stopped = input(fixture("claude_quota_reset"), 130);
    stopped.operator_stopped = true;

    let result = ClaudeAdapter::default().classify(&stopped);

    assert_eq!(result.class, ResultClass::OperatorStop);
    assert!(result.stable_error_hash.is_none());
}

#[test]
fn provider_stable_hash_ignores_reset_timestamps_but_not_error_identity() {
    let first = stable_error_hash(
        Some("usage_limit"),
        "Usage limit reached; resets 2026-07-12T03:00:00Z; request req_abc123",
    );
    let second = stable_error_hash(
        Some("usage_limit"),
        "Usage limit reached; resets 2026-07-19T09:45:00Z; request req_def456",
    );
    let different = stable_error_hash(
        Some("invalid_api_key"),
        "Authentication failed; request req_abc123",
    );

    assert_eq!(first, second);
    assert_ne!(first, different);
}

#[test]
fn provider_stable_hash_ignores_human_reset_times_and_retry_durations() {
    let first = stable_error_hash(
        Some("usage_limit"),
        "You've hit your limit; resets 2am (America/Sao_Paulo)",
    );
    let second = stable_error_hash(
        Some("usage_limit"),
        "You've hit your limit; resets 11pm (America/Sao_Paulo)",
    );
    let retry_first = stable_error_hash(Some("rate_limit"), "Rate limit; retry after 15 seconds");
    let retry_second = stable_error_hash(Some("rate_limit"), "Rate limit; retry after 90 seconds");

    assert_eq!(first, second);
    assert_eq!(retry_first, retry_second);
}
