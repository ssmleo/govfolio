//! Contract snapshots: generated JSON Schemas must match the committed copies
//! under `crates/core/schemas/`. Contract changes must be visible in git —
//! regenerate deliberately with `UPDATE_SNAPSHOT=1 cargo test -p core --test
//! schema_snapshot` and commit the diff.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

fn assert_snapshot(schema: &schemars::Schema, file: &str) {
    let mut current = serde_json::to_string_pretty(schema).expect("schema serializes");
    current.push('\n'); // committed files end with a newline

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("schemas")
        .join(file);

    if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
        fs::create_dir_all(path.parent().expect("snapshot dir")).expect("create schemas/");
        fs::write(&path, &current).expect("write snapshot");
    }

    let committed = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing committed snapshot at {}; generate it with \
             `UPDATE_SNAPSHOT=1 cargo test -p core --test schema_snapshot` and commit it",
            path.display()
        )
    });

    assert_eq!(
        committed, current,
        "{file} drifted from the committed snapshot; if the contract change is \
         intentional, regenerate with UPDATE_SNAPSHOT=1 and commit the diff"
    );
}

#[test]
fn gold_candidate_schema_matches_committed_snapshot() {
    assert_snapshot(
        &govfolio_core::schemas::gold_candidate(),
        "gold_candidate.json",
    );
}

#[test]
fn record_filter_schema_matches_committed_snapshot() {
    assert_snapshot(
        &govfolio_core::schemas::record_filter(),
        "record_filter.json",
    );
}
