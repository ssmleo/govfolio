//! Contract snapshot: the generated `(uk_commons_register, interest)` details
//! schema must match the committed copy the conformance registry embeds
//! (`crates/pipeline/schemas/details/uk_commons_register.interest.json`) —
//! invariant 5. Regenerate deliberately with
//! `UPDATE_SNAPSHOT=1 cargo test -p uk_commons_register --test details_schema_snapshot`
//! and commit the diff.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::Path;

#[test]
fn details_schema_matches_committed_snapshot() {
    let schema = uk_commons_register::details::interest_details_schema();
    let mut current = serde_json::to_string_pretty(&schema).expect("schema serializes");
    current.push('\n'); // committed files end with a newline

    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("pipeline")
        .join("schemas")
        .join("details")
        .join("uk_commons_register.interest.json");

    if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
        fs::create_dir_all(path.parent().expect("snapshot dir")).expect("create details/");
        fs::write(&path, &current).expect("write snapshot");
    }

    let committed = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing committed snapshot at {}; generate it with \
             `UPDATE_SNAPSHOT=1 cargo test -p uk_commons_register --test details_schema_snapshot` \
             and commit it",
            path.display()
        )
    });

    assert_eq!(
        committed, current,
        "(uk_commons_register, interest) details schema drifted from the committed snapshot; \
         if the contract change is intentional, regenerate with UPDATE_SNAPSHOT=1 and commit \
         the diff"
    );
}
