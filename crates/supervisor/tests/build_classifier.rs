#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::path::PathBuf;

use loop_supervisor::build_classifier::{
    CargoDisposition, ClassificationContext, ResourceClass, apply_job_budget, classify_cargo,
};

fn context() -> ClassificationContext {
    ClassificationContext {
        worktree: native_path("repo/worktrees/lane-a"),
        target_dir: native_path("repo/worktrees/lane-a/target"),
        shared_target: native_path("repo/target"),
        bronze_roots: vec![native_path("govfolio/bronze")],
        category: None,
    }
}

fn native_path(suffix: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(format!("C:/{suffix}"))
    } else {
        PathBuf::from(format!("/{suffix}"))
    }
}

#[test]
fn build_classifier_distinguishes_passthrough_focused_and_exclusive_commands() {
    assert_eq!(
        classify_cargo(&["metadata".into()], &context(), None).unwrap(),
        CargoDisposition::Passthrough
    );
    assert_eq!(
        classify_cargo(
            &["check".into(), "-p".into(), "core".into()],
            &context(),
            None
        )
        .unwrap(),
        CargoDisposition::Managed(ResourceClass::Focused)
    );
    for args in [
        vec!["check".into()],
        vec!["test".into(), "--no-run".into(), "-p".into(), "core".into()],
        vec!["test".into(), "--workspace".into()],
        vec!["bench".into(), "-p".into(), "core".into()],
        vec!["epoch-gate".into()],
        vec!["future-compiler-command".into()],
    ] {
        assert_eq!(
            classify_cargo(&args, &context(), None).unwrap(),
            CargoDisposition::Managed(ResourceClass::Exclusive),
            "args={args:?}"
        );
    }
    let mut cold = context();
    cold.category = Some("cold-build".to_owned());
    assert_eq!(
        classify_cargo(&["check".into(), "-p".into(), "core".into()], &cold, None,).unwrap(),
        CargoDisposition::Managed(ResourceClass::Exclusive)
    );
}

#[test]
fn build_classifier_rejects_destructive_or_governed_storage_commands() {
    assert!(classify_cargo(&["clean".into()], &context(), None).is_err());
    assert!(
        classify_cargo(
            &[
                "check".into(),
                "-p".into(),
                "core".into(),
                "--target-dir".into(),
                native_path("govfolio/bronze/build")
                    .to_string_lossy()
                    .into_owned(),
            ],
            &context(),
            None,
        )
        .is_err()
    );
    let mut shared = context();
    shared.target_dir = shared.shared_target.clone();
    assert!(classify_cargo(&["check".into(), "-p".into(), "core".into()], &shared, None).is_err());

    assert!(
        classify_cargo(
            &[
                "check".into(),
                "-p".into(),
                "core".into(),
                format!(
                    "--target-dir={}",
                    native_path("repo/target").to_string_lossy()
                ),
            ],
            &context(),
            None,
        )
        .is_err()
    );
    assert!(
        classify_cargo(
            &[
                "check".into(),
                "-p".into(),
                "core".into(),
                "--config".into(),
                format!(
                    "build.target-dir='{}'",
                    native_path("repo/target").to_string_lossy()
                ),
            ],
            &context(),
            None,
        )
        .is_err()
    );
    assert!(
        classify_cargo(
            &[
                "check".into(),
                "-p".into(),
                "core".into(),
                "--target-dir".into(),
                native_path("repo/worktrees/lane-a/target")
                    .to_string_lossy()
                    .into_owned(),
            ],
            &context(),
            None,
        )
        .is_ok()
    );
}

#[test]
fn build_classifier_allows_explicit_upgrades_but_not_downgrades() {
    assert_eq!(
        classify_cargo(
            &["check".into(), "-p".into(), "core".into()],
            &context(),
            Some(ResourceClass::Exclusive),
        )
        .unwrap(),
        CargoDisposition::Managed(ResourceClass::Exclusive)
    );
    assert!(
        classify_cargo(
            &["test".into(), "--workspace".into()],
            &context(),
            Some(ResourceClass::Focused),
        )
        .is_err()
    );
}

#[test]
fn build_classifier_caps_all_cargo_job_flag_forms() {
    for args in [
        vec!["check".into(), "-j12".into()],
        vec!["check".into(), "-j".into(), "12".into()],
        vec!["check".into(), "--jobs=12".into()],
        vec!["check".into(), "--jobs".into(), "12".into()],
    ] {
        let capped = apply_job_budget(&args, 6).unwrap();
        assert_eq!(capped, vec!["check", "--jobs", "6"]);
    }
    assert_eq!(
        apply_job_budget(&["check".into(), "--jobs=4".into()], 6).unwrap(),
        vec!["check", "--jobs", "4"]
    );
}
