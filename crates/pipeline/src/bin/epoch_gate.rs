//! `cargo run -p pipeline --bin epoch-gate -- E2` — the epoch gate for
//! orchestrator use (goal 016): verifies the frozen E1 reference bundle,
//! runs the real Rust-builder repository acceptance block, prints per-role
//! scores vs thresholds, and renders the entry verdict.
//! Exit 0 = gate open; nonzero = blocked (fail closed). A BLOCKED verdict
//! over missing scout/surveyor/sampler references is honest output, not a
//! harness failure.

use std::process::ExitCode;

use pipeline::evals::{self, Outcome};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(epoch), None) = (args.next(), args.next()) else {
        eprintln!("usage: cargo run -p pipeline --bin epoch-gate -- <epoch>  (only E2 is wired)");
        return ExitCode::FAILURE;
    };
    let root = pipeline::conformance::workspace_root();
    let report = match evals::full_gate(&root, &epoch) {
        Ok(report) => report,
        Err(e) => {
            eprintln!("epoch-gate: {e:#}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "epoch-gate {epoch}: role evals vs frozen E1 reference ({})",
        evals::LOCK_PATH
    );
    if report.lock_failures.is_empty() {
        println!("reference bundle: FROZEN, all pins verify");
    } else {
        println!("reference bundle: NOT INTACT");
        for failure in &report.lock_failures {
            println!("  - {failure}");
        }
    }
    for role_report in &report.roles {
        let name = role_report.role.name();
        match &role_report.outcome {
            Outcome::Scored { checks } => {
                let passed = checks.iter().filter(|c| c.passed).count();
                let verdict = if role_report.meets_threshold() {
                    "PASS"
                } else {
                    "BELOW THRESHOLD"
                };
                println!(
                    "  {name:<14} score {:.2} / threshold {:.2}  {verdict}  ({passed}/{} checks)",
                    evals::score(checks),
                    role_report.role.threshold(),
                    checks.len()
                );
                for check in checks.iter().filter(|c| !c.passed) {
                    println!(
                        "      FAIL {}: {}",
                        check.name,
                        check.detail.replace('\n', "\n      ")
                    );
                }
            }
            Outcome::NotApplicable { reason } => {
                println!("  {name:<14} NOT_APPLICABLE — {reason}");
            }
        }
    }
    if report.open() {
        println!("verdict: {epoch} GATE OPEN");
        ExitCode::SUCCESS
    } else {
        println!(
            "verdict: {epoch} BLOCKED — {} blocker(s):",
            report.blockers.len()
        );
        for blocker in &report.blockers {
            println!("  - {blocker}");
        }
        ExitCode::FAILURE
    }
}
