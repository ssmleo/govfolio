//! `cargo run -p pipeline --bin conformance -- <adapter>` — the commanded
//! conformance runner. Adapter crates depend on `pipeline` for the trait, so
//! this bin cannot link them back (cargo forbids dependency cycles). It
//! dispatches instead: every adapter ships a three-line `conformance_entry`
//! bin calling [`pipeline::conformance::adapter_entry`], and this runner
//! re-execs it via cargo, propagating the exit code.

use std::process::{Command, ExitCode};

use anyhow::Context as _;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("conformance: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    let mut args = std::env::args().skip(1);
    let (Some(adapter), None) = (args.next(), args.next()) else {
        anyhow::bail!("usage: cargo run -p pipeline --bin conformance -- <adapter>");
    };
    anyhow::ensure!(
        !adapter.is_empty()
            && adapter
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
        "adapter name must be [a-z0-9_]+, got {adapter:?}"
    );
    let dir = pipeline::conformance::adapter_dir(&adapter);
    anyhow::ensure!(dir.is_dir(), "no adapter crate at {}", dir.display());
    // Package name == adapter name == fixture directory name, by convention.
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    let status = Command::new(cargo)
        .args([
            "run",
            "--quiet",
            "-p",
            &adapter,
            "--bin",
            "conformance_entry",
        ])
        .current_dir(pipeline::conformance::workspace_root())
        .status()
        .with_context(|| format!("spawning conformance entry for {adapter}"))?;
    Ok(if status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
