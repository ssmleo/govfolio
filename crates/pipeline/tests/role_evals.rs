//! Goal 016 acceptance: `cargo test -p pipeline role_evals`.
//!
//! Runs every role scorer against the frozen E1 `us_house` reference and
//! asserts each role meets its threshold (`docs/decisions/role-eval-thresholds.md`).
//! Stage 0 calibration produced real scout/surveyor/sampler E1 reference
//! artifacts (previously missing since the goal-001 walking skeleton skipped
//! those phases), so no role is `NOT_APPLICABLE` anymore and the E2 gate is
//! open. The freeze test makes the reference bundle tamper-evident.

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use pipeline::evals::{self, Check, Outcome, Role};

fn root() -> PathBuf {
    pipeline::conformance::workspace_root()
}

fn passing_rust_builder_outcome() -> Outcome {
    Outcome::Scored {
        checks: vec![
            Check {
                name: "conformance_us_house_5_of_5",
                passed: true,
                detail: "synthetic passing evidence for process-free gate logic".to_owned(),
            },
            Check {
                name: "cargo_fmt_check",
                passed: true,
                detail: "synthetic passing evidence for process-free gate logic".to_owned(),
            },
            Check {
                name: "cargo_clippy_deny_warnings",
                passed: true,
                detail: "synthetic passing evidence for process-free gate logic".to_owned(),
            },
            Check {
                name: "cargo_test_workspace",
                passed: true,
                detail: "synthetic passing evidence for process-free gate logic".to_owned(),
            },
        ],
    }
}

#[test]
fn role_evals_scorers_do_not_spawn_cargo() {
    let scorer_sources = [
        include_str!("../src/evals/mod.rs"),
        include_str!("../src/evals/roles.rs"),
    ]
    .join("\n");
    let forbidden = [
        ["GOVFOLIO_ROLE_EVALS_", "INNER"].concat(),
        ["CARGO", "_TARGET_DIR"].concat(),
        ["Command", "::new"].concat(),
        ["std::process::", "Command"].concat(),
    ];

    for token in forbidden {
        assert!(
            !scorer_sources.contains(&token),
            "role scoring must be pure filesystem evaluation; found forbidden token {token:?}"
        );
    }
}

fn assert_meets_threshold(role: Role) {
    let report = evals::score_artifact_role(&root(), role).unwrap();
    match &report.outcome {
        Outcome::Scored { checks } => {
            let failed: Vec<String> = checks
                .iter()
                .filter(|c| !c.passed)
                .map(|c| format!("{}: {}", c.name, c.detail))
                .collect();
            assert!(
                report.meets_threshold(),
                "role {} scored {:.2} < threshold {:.2}; failed checks:\n{}",
                role.name(),
                evals::score(checks),
                role.threshold(),
                failed.join("\n")
            );
        }
        Outcome::NotApplicable { reason } => {
            panic!("role {} unexpectedly NOT_APPLICABLE: {reason}", role.name())
        }
    }
}

// ---------------------------------------------------------------------------
// 1. Reference bundle frozen (tamper-evident)
// ---------------------------------------------------------------------------

#[test]
fn role_evals_reference_bundle_frozen() {
    let failures = evals::verify_lock(&root());
    assert_eq!(
        failures,
        Vec::<String>::new(),
        "frozen E1 reference drifted or is missing (supersede the lock, never mutate artifacts)"
    );
}

#[test]
fn role_evals_lock_pins_required_artifacts() {
    let lock = evals::load_lock(&root()).unwrap();
    assert_eq!(lock.epoch, "E1");
    assert_eq!(lock.reference, "us_house");
    assert!(lock.version >= 1);
    let mut required = vec![
        "docs/regimes/us-house.md".to_owned(),
        "crates/adapters/us_house/fixtures/MANIFEST.json".to_owned(),
        "crates/pipeline/schemas/details/us_house.transaction.json".to_owned(),
    ];
    for case in [
        "typical_single_row",
        "multi_row_sp_vehicle",
        "amendment_unlinked",
        "sp_owner_options",
        "scanned_paper_ptr", // goal 021 LLM-path case (lock v2 supersede)
    ] {
        required.push(format!(
            "crates/adapters/us_house/fixtures/{case}/input.pdf"
        ));
        required.push(format!(
            "crates/adapters/us_house/fixtures/{case}/expected.silver.json"
        ));
        required.push(format!(
            "crates/adapters/us_house/fixtures/{case}/expected.gold.json"
        ));
    }
    // The conformance extraction cache is ground-truth-derived — pinned too.
    required.push(
        "crates/adapters/us_house/fixtures/scanned_paper_ptr/extraction.cache.json".to_owned(),
    );
    // Lock v2 (goal 021) must carry its supersession trail.
    assert!(lock.version >= 2, "goal 021 superseded the lock to v2");
    assert!(lock.supersedes.is_some() && lock.reason.is_some() && lock.date.is_some());
    for path in &required {
        assert!(
            lock.pins.contains_key(path),
            "lock must pin {path} (got {} pins)",
            lock.pins.len()
        );
    }
    assert!(
        lock.pins
            .keys()
            .any(|k| k.starts_with("docs/regimes/us-house/evidence/")),
        "lock must pin the archived evidence files"
    );
    assert!(
        lock.pins.len() >= 17,
        "expected >= 17 pins, got {}",
        lock.pins.len()
    );
}

// ---------------------------------------------------------------------------
// 2. Applicable roles score >= threshold vs the frozen reference
// ---------------------------------------------------------------------------

#[test]
fn role_evals_spec_writer_meets_threshold() {
    assert_meets_threshold(Role::SpecWriter);
}

#[test]
fn role_evals_test_designer_meets_threshold() {
    assert_meets_threshold(Role::TestDesigner);
}

#[test]
fn role_evals_auditor_meets_threshold() {
    assert_meets_threshold(Role::Auditor);
}

// ---------------------------------------------------------------------------
// 3. Scout/surveyor/sampler now have E1 reference artifacts (Stage 0
//    calibration, docs/decisions/role-eval-thresholds.md) and score like any
//    other role — no longer NOT_APPLICABLE / E2-blocking.
// ---------------------------------------------------------------------------

#[test]
fn role_evals_scout_meets_threshold() {
    assert_meets_threshold(Role::Scout);
}

#[test]
fn role_evals_surveyor_meets_threshold() {
    assert_meets_threshold(Role::Surveyor);
}

#[test]
fn role_evals_sampler_meets_threshold() {
    assert_meets_threshold(Role::Sampler);
}

// ---------------------------------------------------------------------------
// 4. Thresholds documented + epoch gate wired
// ---------------------------------------------------------------------------

#[test]
fn role_evals_thresholds_documented() {
    let doc = std::fs::read_to_string(
        root()
            .join("docs")
            .join("decisions")
            .join("role-eval-thresholds.md"),
    )
    .expect("docs/decisions/role-eval-thresholds.md must exist (goal 016 checklist item 3)");
    for role in Role::ALL {
        assert!(
            doc.contains(role.name()),
            "thresholds doc must cover role {}",
            role.name()
        );
    }
    assert!(
        doc.contains("NOT_APPLICABLE"),
        "thresholds doc must define NOT_APPLICABLE epoch-gate semantics"
    );
    assert!(
        doc.contains("founder"),
        "thresholds doc must state that changes are founder-gated"
    );
}

#[test]
fn role_evals_gate_rejects_unwired_epochs() {
    let err = evals::evaluate_gate(&root(), "E3", passing_rust_builder_outcome()).unwrap_err();
    assert!(
        err.to_string().contains("E2"),
        "only the E2 gate is wired; got: {err}"
    );
}

#[test]
fn role_evals_gate_blocks_missing_reference_artifacts() {
    let empty_root = tempfile::tempdir().unwrap();
    let report =
        evals::evaluate_gate(empty_root.path(), "E2", passing_rust_builder_outcome()).unwrap();

    assert!(!report.open(), "an empty reference root must fail closed");
    assert!(
        !report.lock_failures.is_empty(),
        "a missing reference lock must be a blocker"
    );
    assert!(
        report
            .roles
            .iter()
            .any(|role| matches!(role.outcome, Outcome::NotApplicable { .. })),
        "missing role artifacts must remain NOT_APPLICABLE"
    );
    assert!(
        report
            .blockers
            .iter()
            .any(|blocker| blocker.contains("NOT_APPLICABLE")
                || blocker.contains("no reference artifact")),
        "NOT_APPLICABLE must remain epoch-blocking: {:#?}",
        report.blockers
    );
}

/// Process-free gate logic: every role >= threshold, zero `NOT_APPLICABLE`, E2 OPEN.
/// Stage 0 calibration (docs/decisions/role-eval-thresholds.md) produced real
/// scout/surveyor/sampler E1 reference artifacts, so all 7 roles now score
/// instead of blocking on missing references. The explicit epoch-gate binary
/// owns real Rust-builder command execution.
#[test]
fn role_evals_gate_logic_e2_open_with_passing_rust_builder_evidence() {
    let report = evals::evaluate_gate(&root(), "E2", passing_rust_builder_outcome()).unwrap();
    assert_eq!(
        report.lock_failures,
        Vec::<String>::new(),
        "reference bundle must verify inside the gate"
    );
    for role_report in &report.roles {
        match &role_report.outcome {
            Outcome::Scored { checks } => {
                let failed: Vec<String> = checks
                    .iter()
                    .filter(|c| !c.passed)
                    .map(|c| format!("{}: {}", c.name, c.detail))
                    .collect();
                assert!(
                    role_report.meets_threshold(),
                    "role {} scored {:.2} < threshold {:.2}; failed checks:\n{}",
                    role_report.role.name(),
                    evals::score(checks),
                    role_report.role.threshold(),
                    failed.join("\n")
                );
            }
            Outcome::NotApplicable { reason } => panic!(
                "role {} unexpectedly NOT_APPLICABLE (Stage 0 calibration should have \
                 produced every reference artifact): {reason}",
                role_report.role.name()
            ),
        }
    }
    assert!(
        report.open(),
        "E2 must be OPEN now that every role has a reference artifact: blockers {:#?}",
        report.blockers
    );
    assert_eq!(
        report.blockers.len(),
        0,
        "no blockers once every role scores at threshold: {:#?}",
        report.blockers
    );
}
