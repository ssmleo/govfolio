//! Contract snapshots: the generated `(eu_parliament_dpi|fr_hatvp_dia|de_bundestag,
//! interest)` details schemas must match the committed copies the conformance
//! registry embeds (`crates/pipeline/schemas/details/*.interest.json`) —
//! invariant 5. Regenerate deliberately with
//! `UPDATE_SNAPSHOT=1 cargo test -p eu_fr_de_annual --test details_schema_snapshot`
//! and commit the diff.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::fs;
use std::path::PathBuf;

fn schemas_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("pipeline")
        .join("schemas")
        .join("details")
}

fn check(schema: &schemars::Schema, file: &str) {
    let mut current = serde_json::to_string_pretty(schema).expect("schema serializes");
    current.push('\n'); // committed files end with a newline

    let path = schemas_dir().join(file);
    if std::env::var_os("UPDATE_SNAPSHOT").is_some() {
        fs::create_dir_all(path.parent().expect("snapshot dir")).expect("create details/");
        fs::write(&path, &current).expect("write snapshot");
    }

    let committed = fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing committed snapshot at {}; generate it with \
             `UPDATE_SNAPSHOT=1 cargo test -p eu_fr_de_annual --test details_schema_snapshot` \
             and commit it",
            path.display()
        )
    });

    assert_eq!(
        committed, current,
        "{file} details schema drifted from the committed snapshot; if the contract change \
         is intentional, regenerate with UPDATE_SNAPSHOT=1 and commit the diff"
    );
}

#[test]
fn eu_details_schema_matches_committed_snapshot() {
    check(
        &eu_fr_de_annual::eu::interest_details_schema(),
        "eu_parliament_dpi.interest.json",
    );
}

#[test]
fn fr_details_schema_matches_committed_snapshot() {
    check(
        &eu_fr_de_annual::fr::interest_details_schema(),
        "fr_hatvp_dia.interest.json",
    );
}

#[test]
fn de_details_schema_matches_committed_snapshot() {
    check(
        &eu_fr_de_annual::de::interest_details_schema(),
        "de_bundestag.interest.json",
    );
}
