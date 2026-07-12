use std::path::Path;
use std::process::Command;

use super::{Check, Outcome};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CommandSpec {
    pub(super) check_name: &'static str,
    pub(super) program: String,
    pub(super) args: Vec<&'static str>,
    pub(super) env_overrides: Vec<(String, String)>,
    pub(super) stdout_marker: Option<&'static str>,
}

pub(super) trait CommandRunner {
    fn run(&mut self, root: &Path, spec: &CommandSpec) -> Check;
}

pub(super) struct ProcessCommandRunner;

impl CommandRunner for ProcessCommandRunner {
    fn run(&mut self, root: &Path, spec: &CommandSpec) -> Check {
        let mut command = Command::new(&spec.program);
        command.args(&spec.args).current_dir(root);
        for (key, value) in &spec.env_overrides {
            command.env(key, value);
        }
        match command.output() {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);
                let marker_ok = spec
                    .stdout_marker
                    .is_none_or(|marker| stdout.contains(marker));
                let passed = output.status.success() && marker_ok;
                let detail = if passed {
                    format!("`{} {}` exit 0", spec.program, spec.args.join(" "))
                } else {
                    format!(
                        "`{} {}` status {:?}, marker {:?} found: {marker_ok}\n\
                         stdout tail: {}\nstderr tail: {}",
                        spec.program,
                        spec.args.join(" "),
                        output.status.code(),
                        spec.stdout_marker,
                        tail(&stdout),
                        tail(&stderr),
                    )
                };
                Check::new(spec.check_name, passed, detail)
            }
            Err(error) => Check::new(
                spec.check_name,
                false,
                format!("failed to spawn {}: {error}", spec.program),
            ),
        }
    }
}

pub(super) fn rust_builder_outcome_with_runner<R: CommandRunner>(
    root: &Path,
    cargo: &str,
    runner: &mut R,
) -> Outcome {
    let checks = rust_builder_command_specs(cargo)
        .iter()
        .map(|spec| runner.run(root, spec))
        .collect();
    Outcome::Scored { checks }
}

fn rust_builder_command_specs(cargo: &str) -> Vec<CommandSpec> {
    [
        (
            "conformance_us_house_5_of_5",
            vec![
                "run",
                "--quiet",
                "-p",
                "pipeline",
                "--bin",
                "conformance",
                "--",
                "us_house",
            ],
            Some("5/5 cases green"),
        ),
        ("cargo_fmt_check", vec!["fmt", "--check"], None),
        (
            "cargo_clippy_deny_warnings",
            vec!["clippy", "--all-targets", "--", "-D", "warnings"],
            None,
        ),
        ("cargo_test_workspace", vec!["test", "--workspace"], None),
    ]
    .into_iter()
    .map(|(check_name, args, stdout_marker)| CommandSpec {
        check_name,
        program: cargo.to_owned(),
        args,
        env_overrides: Vec::new(),
        stdout_marker,
    })
    .collect()
}

fn tail(text: &str) -> &str {
    let len = text.len();
    if len <= 2000 {
        return text.trim_end();
    }
    let mut start = len - 2000;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::evals::Check;

    #[derive(Default)]
    struct RecordingRunner {
        seen: Vec<CommandSpec>,
    }

    impl CommandRunner for RecordingRunner {
        fn run(&mut self, _root: &Path, spec: &CommandSpec) -> Check {
            self.seen.push(spec.clone());
            Check::new(spec.check_name, true, "recorded without spawning")
        }
    }

    #[test]
    fn explicit_full_gate_requests_complete_rust_builder_block() {
        let root = crate::conformance::workspace_root();
        let mut runner = RecordingRunner::default();

        let report =
            super::super::full_gate_with_runner(&root, "E2", "sentinel-cargo", &mut runner)
                .unwrap();

        assert!(report.open());
        assert_eq!(
            runner
                .seen
                .iter()
                .map(|spec| (spec.check_name, spec.args.clone(), spec.stdout_marker))
                .collect::<Vec<_>>(),
            vec![
                (
                    "conformance_us_house_5_of_5",
                    vec![
                        "run",
                        "--quiet",
                        "-p",
                        "pipeline",
                        "--bin",
                        "conformance",
                        "--",
                        "us_house",
                    ],
                    Some("5/5 cases green"),
                ),
                ("cargo_fmt_check", vec!["fmt", "--check"], None),
                (
                    "cargo_clippy_deny_warnings",
                    vec!["clippy", "--all-targets", "--", "-D", "warnings"],
                    None,
                ),
                ("cargo_test_workspace", vec!["test", "--workspace"], None,),
            ]
        );
        assert!(
            runner
                .seen
                .iter()
                .all(|spec| spec.program == "sentinel-cargo" && spec.env_overrides.is_empty())
        );
    }

    #[derive(Default)]
    struct FailingWorkspaceRunner {
        seen: Vec<&'static str>,
    }

    impl CommandRunner for FailingWorkspaceRunner {
        fn run(&mut self, _root: &Path, spec: &CommandSpec) -> Check {
            self.seen.push(spec.check_name);
            Check::new(
                spec.check_name,
                spec.check_name != "cargo_test_workspace",
                "injected command result",
            )
        }
    }

    #[test]
    fn explicit_full_gate_blocks_when_workspace_test_fails() {
        let root = crate::conformance::workspace_root();
        let mut runner = FailingWorkspaceRunner::default();

        let report =
            super::super::full_gate_with_runner(&root, "E2", "sentinel-cargo", &mut runner)
                .unwrap();

        assert_eq!(runner.seen.len(), 4, "the complete block remains requested");
        assert!(!report.open(), "a required command failure must block E2");
        let rust_builder = report
            .roles
            .iter()
            .find(|role| role.role == crate::evals::Role::RustBuilder)
            .unwrap();
        assert!(!rust_builder.meets_threshold());
        assert!(
            report
                .blockers
                .iter()
                .any(|blocker| blocker.contains("rust-builder")
                    && blocker.contains("cargo_test_workspace")),
            "workspace failure must remain a named Rust-builder blocker: {:#?}",
            report.blockers
        );
    }

    #[test]
    fn explicit_full_gate_rejects_unwired_epoch_before_commands() {
        let root = crate::conformance::workspace_root();
        let mut runner = RecordingRunner::default();

        let error = super::super::full_gate_with_runner(&root, "E3", "sentinel-cargo", &mut runner)
            .unwrap_err();

        assert!(error.to_string().contains("E2"));
        assert!(
            runner.seen.is_empty(),
            "unsupported epochs must fail before requesting repository acceptance"
        );
    }
}
