//! Contract snapshots: the generated `(canada_ciec, interest)` and
//! `(canada_ciec, change_notification)` details schemas must match the
//! committed copies the conformance registry embeds
//! (`crates/pipeline/schemas/details/canada_ciec.*.json`) — invariant 5.
//! Regenerate deliberately with
//! `UPDATE_SNAPSHOT=1 cargo test -p canada_ciec --test details_schema_snapshot`
//! and commit the diff.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

fn schema_path(file: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("pipeline")
        .join("schemas")
        .join("details")
        .join(file)
}

fn assert_snapshot(schema: &schemars::Schema, file: &str) {
    let mut current = serde_json::to_string_pretty(schema).expect("schema serializes");
    current.push('\n'); // committed files end with a newline

    let path = schema_path(file);
    if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
        fs::create_dir_all(path.parent().expect("snapshot dir")).expect("create details/");
        fs::write(&path, &current).expect("write snapshot");
    }

    let committed = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing committed snapshot at {}; generate it with \
             `UPDATE_SNAPSHOT=1 cargo test -p canada_ciec --test details_schema_snapshot` and commit it",
            path.display()
        )
    });

    assert_eq!(
        committed, current,
        "{file} details schema drifted from the committed snapshot; if the contract change is \
         intentional, regenerate with UPDATE_SNAPSHOT=1 and commit the diff"
    );
}

#[test]
fn interest_details_schema_matches_committed_snapshot() {
    assert_snapshot(
        &canada_ciec::details::interest_details_schema(),
        "canada_ciec.interest.json",
    );
}

#[test]
fn change_notification_details_schema_matches_committed_snapshot() {
    assert_snapshot(
        &canada_ciec::details::change_notification_details_schema(),
        "canada_ciec.change_notification.json",
    );
}
