#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::Path;

use loop_supervisor::build_experiment::{
    ExperimentOutcome, ExperimentReview, SampleDisposition, SampleMeasurement, SamplePhase,
    SampleVariant, WorkloadKind, deterministic_pair_order, evaluate_measurements, parse_manifest,
    parse_review, write_immutable_json,
};
use serde_json::json;

fn pilot_bytes() -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/superpowers/pilots/2026-07-12-build-experiment-schema-pilot-v3.json"),
    )
    .unwrap()
}

#[test]
fn experiment_manifest_pilot_is_strict_and_deterministically_ordered() {
    let parsed =
        parse_manifest(&pilot_bytes()).expect("manual pilot must become executable schema");
    assert_eq!(parsed.manifest.schema_version, 2);
    assert_eq!(parsed.manifest.workload.kind, WorkloadKind::Cold);
    assert_eq!(parsed.sha256.len(), 64);
    let first = deterministic_pair_order(&parsed.manifest.experiment_id, 0);
    let repeated = deterministic_pair_order(&parsed.manifest.experiment_id, 0);
    assert_eq!(first, repeated);
    assert_ne!(first[0], first[1]);
    assert_eq!(
        deterministic_pair_order(&parsed.manifest.experiment_id, 1),
        first
    );

    let mut unknown: serde_json::Value = serde_json::from_slice(&pilot_bytes()).unwrap();
    unknown["unreviewed_expansion"] = json!(true);
    assert!(parse_manifest(&serde_json::to_vec(&unknown).unwrap()).is_err());

    let mut destructive: serde_json::Value = serde_json::from_slice(&pilot_bytes()).unwrap();
    destructive["workload"]["cargo_args"] = json!(["clean"]);
    assert!(parse_manifest(&serde_json::to_vec(&destructive).unwrap()).is_err());
    for command in [
        json!(["run", "-p", "worker"]),
        json!(["publish"]),
        json!(["test", "--workspace"]),
        json!(["check", "--manifest-path", "C:/other/Cargo.toml"]),
        json!(["check", "--config", "net.git-fetch-with-cli=true"]),
        json!(["clippy", "--fix", "--allow-dirty", "--locked"]),
        json!(["check", "-CC:/other", "--locked"]),
        json!(["check", "--", "--locked"]),
        json!(["test", "--", "--no-run", "--locked"]),
    ] {
        destructive["workload"]["cargo_args"] = command;
        assert!(parse_manifest(&serde_json::to_vec(&destructive).unwrap()).is_err());
    }

    let mut unsafe_edit: serde_json::Value = serde_json::from_slice(&pilot_bytes()).unwrap();
    unsafe_edit["workload"]["kind"] = json!("edit");
    assert!(parse_manifest(&serde_json::to_vec(&unsafe_edit).unwrap()).is_err());
    unsafe_edit["workload"]["edit_path"] = json!("../Cargo.toml");
    assert!(parse_manifest(&serde_json::to_vec(&unsafe_edit).unwrap()).is_err());
    unsafe_edit["workload"]["cargo_args"] = json!(["check", "-p", "loop-supervisor", "--locked"]);
    unsafe_edit["workload"]["edit_path"] = json!("crates/supervisor/src/lib.rs");
    assert!(parse_manifest(&serde_json::to_vec(&unsafe_edit).unwrap()).is_ok());
}

#[test]
fn experiment_review_is_fixed_and_cannot_expand_the_contract() {
    let review = json!({
        "schema_version": 2,
        "experiment_id": "release3-build-cost-pilot-v3",
        "manifest_sha256": "a".repeat(64),
        "exploratory_evidence_sha256": "b".repeat(64),
        "decision": "accept",
        "reason": "exploratory evidence satisfies the fixed contract",
        "auditor": "operator",
        "reviewed_at": "2026-07-12T19:00:00Z"
    });
    assert!(matches!(
        parse_review(&serde_json::to_vec(&review).unwrap()).unwrap(),
        ExperimentReview::Accept { .. }
    ));
    let mut expanded = review;
    expanded["extra_commands"] = json!([["test", "--workspace"]]);
    assert!(parse_review(&serde_json::to_vec(&expanded).unwrap()).is_err());

    let matrix = json!({
        "schema_version": 2,
        "experiment_id": "release3-build-cost-pilot-v3",
        "manifest_sha256": "a".repeat(64),
        "exploratory_evidence_sha256": "b".repeat(64),
        "decision": "request_matrix_rerun",
        "reason": "one platform remains unmeasured",
        "auditor": "operator",
        "reviewed_at": "2026-07-12T19:00:00Z",
        "repeated_commands": [["check", "--workspace"]],
        "estimated_cost_seconds": 900
    });
    assert!(matches!(
        parse_review(&serde_json::to_vec(&matrix).unwrap()).unwrap(),
        ExperimentReview::RequestMatrixRerun { .. }
    ));
}

#[test]
fn evaluation_uses_medians_vetoes_regression_and_fails_closed() {
    let manifest = parse_manifest(&pilot_bytes()).unwrap().manifest;
    let exploratory = vec![
        SampleMeasurement::completed(SampleVariant::Baseline, SamplePhase::Exploratory, 0, 1_000),
        SampleMeasurement::completed(SampleVariant::Candidate, SamplePhase::Exploratory, 0, 900),
    ];
    let go = evaluate_measurements(&manifest, &exploratory).unwrap();
    assert_eq!(go.outcome, ExperimentOutcome::Go);
    assert_eq!(go.baseline_median_ms, Some(1_000));
    assert_eq!(go.candidate_median_ms, Some(900));
    assert_eq!(go.improvement_bps, Some(1_000));

    let no_signal = vec![
        SampleMeasurement::completed(SampleVariant::Baseline, SamplePhase::Exploratory, 0, 1_000),
        SampleMeasurement::completed(SampleVariant::Candidate, SamplePhase::Exploratory, 0, 990),
    ];
    assert_eq!(
        evaluate_measurements(&manifest, &no_signal)
            .unwrap()
            .outcome,
        ExperimentOutcome::NoGo
    );

    let inconclusive = vec![
        SampleMeasurement::inconclusive(SampleVariant::Baseline, SamplePhase::Exploratory, 0),
        SampleMeasurement::completed(SampleVariant::Candidate, SamplePhase::Exploratory, 0, 900),
    ];
    assert_eq!(
        evaluate_measurements(&manifest, &inconclusive)
            .unwrap()
            .outcome,
        ExperimentOutcome::Inconclusive
    );

    let candidate_failure = vec![
        SampleMeasurement::completed(SampleVariant::Baseline, SamplePhase::Exploratory, 0, 1_000),
        SampleMeasurement {
            variant: SampleVariant::Candidate,
            phase: SamplePhase::Exploratory,
            ordinal: 0,
            wall_ms: Some(100),
            disposition: SampleDisposition::Failed,
        },
    ];
    assert_eq!(
        evaluate_measurements(&manifest, &candidate_failure)
            .unwrap()
            .outcome,
        ExperimentOutcome::NoGo
    );
}

#[test]
fn warm_confidence_requires_three_pairs_and_immutable_artifacts_never_change() {
    let mut value: serde_json::Value = serde_json::from_slice(&pilot_bytes()).unwrap();
    value["workload"]["kind"] = json!("warm");
    let manifest = parse_manifest(&serde_json::to_vec(&value).unwrap())
        .unwrap()
        .manifest;
    let mut samples = Vec::new();
    samples.push(SampleMeasurement::completed(
        SampleVariant::Baseline,
        SamplePhase::Preparation,
        0,
        5_000,
    ));
    samples.push(SampleMeasurement::completed(
        SampleVariant::Candidate,
        SamplePhase::Preparation,
        0,
        5_000,
    ));
    for ordinal in 0..3 {
        samples.push(SampleMeasurement::completed(
            SampleVariant::Baseline,
            SamplePhase::Confidence,
            ordinal,
            1_000 + u64::from(ordinal) * 10,
        ));
        samples.push(SampleMeasurement::completed(
            SampleVariant::Candidate,
            SamplePhase::Confidence,
            ordinal,
            900 + u64::from(ordinal) * 10,
        ));
    }
    let result = evaluate_measurements(&manifest, &samples).unwrap();
    assert_eq!(result.baseline_median_ms, Some(1_010));
    assert_eq!(result.candidate_median_ms, Some(910));
    assert_eq!(result.outcome, ExperimentOutcome::Go);

    let incomplete = &samples[..4];
    assert!(evaluate_measurements(&manifest, incomplete).is_err());

    let failed_preparation = vec![
        SampleMeasurement::inconclusive(SampleVariant::Baseline, SamplePhase::Preparation, 0),
        SampleMeasurement::completed(SampleVariant::Candidate, SamplePhase::Preparation, 0, 5_000),
        SampleMeasurement::completed(SampleVariant::Baseline, SamplePhase::Exploratory, 0, 1_000),
        SampleMeasurement::completed(SampleVariant::Candidate, SamplePhase::Exploratory, 0, 900),
    ];
    assert_eq!(
        evaluate_measurements(&manifest, &failed_preparation)
            .unwrap()
            .outcome,
        ExperimentOutcome::Inconclusive
    );

    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("evidence.json");
    let first = write_immutable_json(&path, &json!({"outcome": "go"})).unwrap();
    let repeated = write_immutable_json(&path, &json!({"outcome": "go"})).unwrap();
    assert_eq!(first, repeated);
    assert!(write_immutable_json(&path, &json!({"outcome": "no_go"})).is_err());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(temp.path()).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
