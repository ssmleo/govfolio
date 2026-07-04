//! Conformance harness proof (plan Task 7): the passing fixture case is green,
//! and the deliberately-broken case FAILS with a unified diff — a harness that
//! cannot fail proves nothing.

#![allow(clippy::unwrap_used)]

use std::time::Duration;

use fixture_fake::FixtureFakeAdapter;
use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter, PolitenessCfg, RunCtx};
use pipeline::conformance::{adapter_dir, fixtures_dir, run_cases};

fn test_ctx(tag: &str) -> RunCtx {
    let root = std::env::temp_dir().join(format!(
        "govfolio-conformance-test-{tag}-{}",
        std::process::id()
    ));
    RunCtx::new(
        BronzeStore::open(root).unwrap(),
        None,
        Clock::System,
        &PolitenessCfg::new(Duration::ZERO, "test@govfolio.io"),
    )
    .unwrap()
}

#[tokio::test]
async fn passing_case_is_green() {
    let ctx = test_ctx("pass");
    let outcomes = run_cases(&FixtureFakeAdapter, &fixtures_dir("fixture_fake"), &ctx)
        .await
        .unwrap();
    assert!(!outcomes.is_empty(), "harness must find fixture cases");
    for outcome in &outcomes {
        assert!(
            outcome.passed(),
            "case {} failed:\n{}",
            outcome.name,
            outcome.failures.join("\n")
        );
    }
}

#[tokio::test]
async fn broken_case_fails_with_unified_diff() {
    let ctx = test_ctx("broken");
    let dir = adapter_dir("fixture_fake").join("fixtures-broken");
    let outcomes = run_cases(&FixtureFakeAdapter, &dir, &ctx).await.unwrap();
    assert_eq!(outcomes.len(), 1);
    let outcome = &outcomes[0];
    assert_eq!(outcome.name, "wrong_expected_amount");
    assert!(!outcome.passed(), "deliberately-broken case must fail");
    let report = outcome.failures.join("\n");
    assert!(
        report.contains("expected.silver.json"),
        "failure must name the mismatched artifact:\n{report}"
    );
    assert!(
        report.contains("@@"),
        "unified hunk header expected:\n{report}"
    );
    assert!(
        report
            .lines()
            .any(|l| l.starts_with('-') && l.contains("$1,001 - $14,000")),
        "expected side must show as removal:\n{report}"
    );
    assert!(
        report
            .lines()
            .any(|l| l.starts_with('+') && l.contains("$1,001 - $15,000")),
        "actual side must show as addition:\n{report}"
    );
}

#[tokio::test]
async fn discover_and_fetch_land_in_bronze() {
    let ctx = test_ctx("fetch");
    let filings = FixtureFakeAdapter.discover(&ctx).await.unwrap();
    assert!(!filings.is_empty(), "fixture cases are the fake source");
    let doc = FixtureFakeAdapter.fetch(&filings[0], &ctx).await.unwrap();
    assert_eq!(doc.sha256.len(), 64, "Bronze is sha256-addressed");
    let bytes = ctx.bronze.get(&doc).unwrap();
    assert!(
        !bytes.is_empty(),
        "raw bytes must be retrievable from Bronze"
    );
}
