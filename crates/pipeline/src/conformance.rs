//! Shared conformance harness (plan Task 7). For every fixture case under
//! `crates/adapters/<x>/fixtures/<case>/` it runs `parse` and deep-compares
//! Silver, then runs `normalize`, `validate()`s every candidate, checks
//! `details` against the (regime, `record_type`) JSON Schema (invariant 5),
//! and deep-compares Gold. Mismatches print unified diffs. Fail closed
//! (invariant 6): zero-row parses, missing schemas, and unreadable fixtures
//! are failures, never silence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::Context as _;
use serde_json::Value;
use similar::TextDiff;

use govfolio_core::domain::enums::RecordType;
use govfolio_core::domain::gold::GoldCandidate;

use crate::adapter::{BronzeStore, Clock, JurisdictionAdapter, RunCtx, ScratchDir};

/// Result of one fixture case; empty `failures` means the case passed.
#[derive(Debug)]
pub struct CaseOutcome {
    /// Fixture case directory name.
    pub name: String,
    /// Human-readable failure reports (unified diffs included).
    pub failures: Vec<String>,
}

impl CaseOutcome {
    /// True when the case produced no failures.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Workspace root, resolved from this crate's manifest — the conformance
/// harness is a source-tree development tool.
#[must_use]
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// Parent directory for DURABLE Bronze stores (`bronze-local`,
/// `bronze-backfill-real`, ...): `GOVFOLIO_BRONZE_ROOT` (absolute path) when
/// set, else this checkout's `target/`.
///
/// Why the env exists (invariant 2, raw is sacred): `workspace_root()` is
/// compile-time (`CARGO_MANIFEST_DIR`), so every git worktree resolves to its
/// OWN `target/` — that is exactly the JOURNAL 2026-07-09 incident where a
/// 2014 br filing's Bronze bytes physically lived under the `front_b`
/// worktree's `target/`, invisible to the main checkout. Parallel loop lanes
/// (goal 097) export one shared absolute `GOVFOLIO_BRONZE_ROOT` so all lanes
/// converge on a single content-addressed store. Never an OS-temp path.
#[must_use]
pub fn durable_bronze_parent() -> PathBuf {
    durable_bronze_parent_from(|k| std::env::var(k).ok())
}

/// Pure core of [`durable_bronze_parent`] — env lookup injected so tests
/// never mutate process env (racy under the parallel test runner).
///
/// Non-absolute values are IGNORED (fallback applies): a relative root would
/// resolve against each process's cwd, silently splitting the "shared" store
/// across lanes/worktrees — the exact failure the env exists to prevent.
#[must_use]
pub fn durable_bronze_parent_from(lookup: impl Fn(&str) -> Option<String>) -> PathBuf {
    match lookup("GOVFOLIO_BRONZE_ROOT") {
        Some(root) if Path::new(root.trim()).is_absolute() => PathBuf::from(root),
        _ => workspace_root().join("target"),
    }
}

/// `crates/adapters/<name>`.
#[must_use]
pub fn adapter_dir(name: &str) -> PathBuf {
    workspace_root().join("crates").join("adapters").join(name)
}

/// Default fixture location: `crates/adapters/<name>/fixtures`.
#[must_use]
pub fn fixtures_dir(name: &str) -> PathBuf {
    adapter_dir(name).join("fixtures")
}

/// The `details` schema registry (invariant 5): maps (regime, `record_type`)
/// to its committed JSON Schema document. Grows one arm per contract; schema
/// files are snapshot-committed under `crates/pipeline/schemas/details/`.
/// `Ok(None)` means no contract is registered — callers must fail closed.
///
/// # Errors
/// A committed schema document that is not valid JSON.
pub fn details_schema(regime: &str, record_type: RecordType) -> anyhow::Result<Option<Value>> {
    let doc = match (regime, record_type) {
        ("fixture_fake", RecordType::Transaction) => {
            include_str!("../schemas/details/fixture_fake.transaction.json")
        }
        ("us_house", RecordType::Transaction) => {
            include_str!("../schemas/details/us_house.transaction.json")
        }
        ("us_senate", RecordType::Transaction) => {
            include_str!("../schemas/details/us_senate.transaction.json")
        }
        ("uk_commons_register", RecordType::Interest) => {
            include_str!("../schemas/details/uk_commons_register.interest.json")
        }
        ("canada_ciec", RecordType::Interest) => {
            include_str!("../schemas/details/canada_ciec.interest.json")
        }
        ("canada_ciec", RecordType::ChangeNotification) => {
            include_str!("../schemas/details/canada_ciec.change_notification.json")
        }
        ("australia_register", RecordType::Interest) => {
            include_str!("../schemas/details/australia_register.interest.json")
        }
        ("australia_register", RecordType::ChangeNotification) => {
            include_str!("../schemas/details/australia_register.change_notification.json")
        }
        ("eu_parliament_dpi", RecordType::Interest) => {
            include_str!("../schemas/details/eu_parliament_dpi.interest.json")
        }
        ("fr_hatvp_dia", RecordType::Interest) => {
            include_str!("../schemas/details/fr_hatvp_dia.interest.json")
        }
        ("de_bundestag", RecordType::Interest) => {
            include_str!("../schemas/details/de_bundestag.interest.json")
        }
        ("br", RecordType::Holding) => {
            include_str!("../schemas/details/br.holding.json")
        }
        _ => return Ok(None),
    };
    serde_json::from_str(doc)
        .map(Some)
        .with_context(|| format!("committed details schema for ({regime}, {record_type:?})"))
}

/// Validates one candidate's `details` against its registered schema.
/// A missing schema is itself a failure (fail closed, invariants 5 + 6).
///
/// # Errors
/// Registry or schema-compilation failure (not instance mismatches — those
/// are returned as failure strings).
pub fn check_details(regime: &str, candidate: &GoldCandidate) -> anyhow::Result<Vec<String>> {
    let record_type = candidate.record_type;
    let Some(schema) = details_schema(regime, record_type)? else {
        return Ok(vec![format!(
            "no details schema registered for ({regime}, {record_type:?}) — fail closed (invariant 5)"
        )]);
    };
    let validator = jsonschema::validator_for(&schema).map_err(|e| {
        anyhow::anyhow!("compiling details schema for ({regime}, {record_type:?}): {e}")
    })?;
    Ok(validator
        .iter_errors(&candidate.details)
        .map(|err| {
            format!(
                "details violates the ({regime}, {record_type:?}) contract at `{}`: {err}",
                err.instance_path()
            )
        })
        .collect())
}

/// Runs every fixture case directory under `dir` through the harness.
///
/// # Errors
/// Unreadable fixture root or an empty case list (fail closed).
pub async fn run_cases(
    adapter: &dyn JurisdictionAdapter,
    dir: &Path,
    ctx: &RunCtx,
) -> anyhow::Result<Vec<CaseOutcome>> {
    let mut case_dirs = Vec::new();
    for entry in
        fs::read_dir(dir).with_context(|| format!("reading fixture root {}", dir.display()))?
    {
        let path = entry?.path();
        if path.is_dir() {
            case_dirs.push(path);
        }
    }
    case_dirs.sort();
    anyhow::ensure!(
        !case_dirs.is_empty(),
        "no fixture cases under {} — fail closed (invariant 6)",
        dir.display()
    );
    let mut outcomes = Vec::with_capacity(case_dirs.len());
    for case_dir in &case_dirs {
        outcomes.push(run_case(adapter, case_dir, ctx).await);
    }
    Ok(outcomes)
}

async fn run_case(adapter: &dyn JurisdictionAdapter, case_dir: &Path, ctx: &RunCtx) -> CaseOutcome {
    let name = case_dir.file_name().map_or_else(
        || case_dir.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    let mut failures = Vec::new();
    if let Err(e) = run_case_inner(adapter, case_dir, ctx, &mut failures).await {
        failures.push(format!("case did not complete: {e:#}"));
    }
    CaseOutcome { name, failures }
}

async fn run_case_inner(
    adapter: &dyn JurisdictionAdapter,
    case_dir: &Path,
    ctx: &RunCtx,
    failures: &mut Vec<String>,
) -> anyhow::Result<()> {
    let input_path = find_input(case_dir)?;
    let bytes =
        fs::read(&input_path).with_context(|| format!("reading {}", input_path.display()))?;
    let doc = ctx.bronze.put(&bytes)?;

    // parse → Silver, deep-compared against the committed expectation.
    let rows = adapter.parse(&doc, ctx).await.context("parse failed")?;
    let actual_silver = serde_json::to_value(&rows)?;
    let expected_silver = load_json(&case_dir.join("expected.silver.json"))?;
    // Fail closed on a zero-row parse (invariant 6) — UNLESS the fixture's
    // own committed ground truth also expects zero rows (e.g. `br`'s
    // zero-asset-declaration case, plan.md edge case 1: a legitimate "no
    // assets declared" outcome, not a parser bug). A real mismatch (rows
    // empty but nonzero expected, or vice versa) is still caught below by
    // the exact Silver diff.
    if rows.is_empty() && expected_silver != Value::Array(Vec::new()) {
        failures.push("parse produced zero rows — fail closed (invariant 6)".to_owned());
    }
    if let Some(diff) = json_diff("expected.silver.json", &expected_silver, &actual_silver) {
        failures.push(format!("Silver mismatch:\n{diff}"));
    }

    // normalize → Gold: domain validation + details contract + deep compare.
    let candidates = adapter
        .normalize(&rows, ctx)
        .await
        .context("normalize failed")?;
    let regime = adapter.regime().code;
    for (ordinal, candidate) in candidates.iter().enumerate() {
        if let Err(e) = candidate.validate() {
            failures.push(format!("gold[{ordinal}] fails domain validation: {e}"));
        }
        match check_details(regime, candidate) {
            Ok(errors) => {
                failures.extend(errors.into_iter().map(|e| format!("gold[{ordinal}]: {e}")));
            }
            Err(e) => failures.push(format!("gold[{ordinal}]: {e:#}")),
        }
    }
    let actual_gold = serde_json::to_value(&candidates)?;
    let expected_gold = load_json(&case_dir.join("expected.gold.json"))?;
    if let Some(diff) = json_diff("expected.gold.json", &expected_gold, &actual_gold) {
        failures.push(format!("Gold mismatch:\n{diff}"));
    }
    Ok(())
}

fn find_input(case_dir: &Path) -> anyhow::Result<PathBuf> {
    let mut inputs = Vec::new();
    for entry in
        fs::read_dir(case_dir).with_context(|| format!("reading case {}", case_dir.display()))?
    {
        let path = entry?.path();
        if path.is_file() && path.file_stem().is_some_and(|stem| stem == "input") {
            inputs.push(path);
        }
    }
    match inputs.as_slice() {
        [one] => Ok(one.clone()),
        [] => anyhow::bail!("no input.* file in {}", case_dir.display()),
        _ => anyhow::bail!("multiple input.* files in {}", case_dir.display()),
    }
}

fn load_json(path: &Path) -> anyhow::Result<Value> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

/// Entry point for an adapter crate's `conformance_entry` bin: runs the
/// default fixtures, prints per-case outcomes (diffs included), returns
/// nonzero on any failure. The `pipeline` conformance bin dispatches here.
#[must_use]
pub fn adapter_entry(adapter: &dyn JurisdictionAdapter, name: &str) -> ExitCode {
    match entry_inner(adapter, name) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(e) => {
            eprintln!("conformance harness error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn entry_inner(adapter: &dyn JurisdictionAdapter, name: &str) -> anyhow::Result<bool> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let bronze_root = std::env::temp_dir().join(format!(
        "govfolio-conformance-{name}-{}-{nanos}",
        std::process::id()
    ));
    // Ephemeral: removed on drop (success, error, or panic during
    // run_cases) — conformance never writes raw_document, so nothing durably
    // references this path (see ScratchDir's own doc comment).
    let _scratch = ScratchDir::new(bronze_root.clone());
    let ctx = RunCtx::new(
        BronzeStore::open(&bronze_root)?,
        None,
        Clock::System,
        &adapter.politeness(),
    )?;
    let outcomes = runtime.block_on(run_cases(adapter, &fixtures_dir(name), &ctx));
    let outcomes = outcomes?;
    let green = outcomes.iter().filter(|o| o.passed()).count();
    for outcome in &outcomes {
        if outcome.passed() {
            println!("PASS {name}::{}", outcome.name);
        } else {
            println!("FAIL {name}::{}", outcome.name);
            for failure in &outcome.failures {
                println!("  - {}", failure.replace('\n', "\n    "));
            }
        }
    }
    println!("{name}: {green}/{} cases green", outcomes.len());
    Ok(green == outcomes.len())
}

/// Unified diff between expected and actual JSON (pretty-printed, stable key
/// order); `None` when they deep-compare equal.
fn json_diff(header: &str, expected: &Value, actual: &Value) -> Option<String> {
    if expected == actual {
        return None;
    }
    let expected_pretty = pretty(expected);
    let actual_pretty = pretty(actual);
    let text_diff = TextDiff::from_lines(&expected_pretty, &actual_pretty);
    let diff = text_diff
        .unified_diff()
        .context_radius(3)
        .header(
            &format!("expected ({header})"),
            &format!("actual ({header})"),
        )
        .to_string();
    Some(diff)
}

/// Deterministic pretty JSON (`serde_json` maps are key-sorted `BTreeMap`s).
fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn bronze_parent_honors_env_override_and_defaults_to_workspace_target() {
        let overridden = durable_bronze_parent_from(|k| {
            assert_eq!(k, "GOVFOLIO_BRONZE_ROOT");
            Some("C:/shared/bronze-root".to_owned())
        });
        assert_eq!(overridden, PathBuf::from("C:/shared/bronze-root"));

        // Unset, blank, and RELATIVE values all fall back to this checkout's
        // target/ — a relative root would resolve per-process-cwd and split
        // the shared store across lanes.
        assert_eq!(
            durable_bronze_parent_from(|_| None),
            workspace_root().join("target")
        );
        assert_eq!(
            durable_bronze_parent_from(|_| Some("  ".to_owned())),
            workspace_root().join("target")
        );
        assert_eq!(
            durable_bronze_parent_from(|_| Some("target".to_owned())),
            workspace_root().join("target")
        );
    }

    #[test]
    fn registry_has_fixture_fake_transaction_schema() {
        // The fixture_fake transaction contract must be registered.
        let schema = details_schema("fixture_fake", RecordType::Transaction)
            .unwrap()
            .unwrap();
        assert_eq!(schema["type"], json!("object"));
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("amount_band_raw")),
            "raw band must be contractually required: {schema}"
        );
    }

    #[test]
    fn registry_has_us_house_transaction_schema() {
        let schema = details_schema("us_house", RecordType::Transaction)
            .unwrap()
            .unwrap();
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("amount_band_raw")),
            "raw band must be contractually required: {schema}"
        );
    }

    #[test]
    fn registry_has_us_senate_transaction_schema() {
        let schema = details_schema("us_senate", RecordType::Transaction)
            .unwrap()
            .unwrap();
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("amount_band_raw")),
            "raw band must be contractually required: {schema}"
        );
    }

    #[test]
    fn registry_has_uk_commons_register_interest_schema() {
        let schema = details_schema("uk_commons_register", RecordType::Interest)
            .unwrap()
            .unwrap();
        assert!(
            schema["required"]
                .as_array()
                .unwrap()
                .contains(&json!("value_source")),
            "value provenance must be contractually required: {schema}"
        );
    }

    #[test]
    fn registry_has_australia_register_interest_and_change_notification_schemas() {
        let interest = details_schema("australia_register", RecordType::Interest)
            .unwrap()
            .unwrap();
        assert!(
            interest["required"]
                .as_array()
                .unwrap()
                .contains(&json!("source_flavour")),
            "extraction provenance must be contractually required: {interest}"
        );
        let change = details_schema("australia_register", RecordType::ChangeNotification)
            .unwrap()
            .unwrap();
        assert!(
            change["required"]
                .as_array()
                .unwrap()
                .contains(&json!("addition_deletion")),
            "the alteration axis must be contractually required: {change}"
        );
    }

    #[test]
    fn registry_is_closed_for_unknown_pairs() {
        assert!(
            details_schema("fixture_fake", RecordType::Holding)
                .unwrap()
                .is_none()
        );
        assert!(
            details_schema("no_such_regime", RecordType::Transaction)
                .unwrap()
                .is_none()
        );
    }

    fn candidate_with_details(details: Value) -> GoldCandidate {
        use govfolio_core::domain::enums::{AssetClass, Side};
        GoldCandidate {
            filing_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse().unwrap(),
            politician_id: "01BX5ZZKBKACTAV9WEVGEMMVRZ".parse().unwrap(),
            regime_id: "01BX5ZZKBKACTAV9WEVGEMMVS0".parse().unwrap(),
            instrument_id: None,
            asset_description_raw: "Test Asset".to_owned(),
            record_type: RecordType::Transaction,
            asset_class: AssetClass::Equity,
            side: Some(Side::Buy),
            transaction_date: Some(chrono::NaiveDate::from_ymd_opt(2026, 3, 2).unwrap()),
            as_of_date: None,
            notified_date: None,
            value: None,
            owner: None,
            extraction_confidence: Some(1.0),
            extracted_by: "test@0".to_owned(),
            fingerprint: None,
            details,
        }
    }

    #[test]
    fn conforming_details_pass_the_contract() {
        let candidate = candidate_with_details(json!({
            "amount_band_raw": "$1,001 - $15,000",
            "source_ordinal": 0
        }));
        assert_eq!(
            check_details("fixture_fake", &candidate).unwrap(),
            Vec::<String>::new()
        );
    }

    #[test]
    fn nonconforming_details_are_rejected() {
        let candidate = candidate_with_details(json!({ "source_ordinal": 0 }));
        let failures = check_details("fixture_fake", &candidate).unwrap();
        assert!(
            failures.iter().any(|f| f.contains("amount_band_raw")),
            "missing required key must be reported: {failures:?}"
        );
    }

    #[test]
    fn unregistered_pair_fails_closed() {
        let mut candidate = candidate_with_details(json!({}));
        candidate.record_type = RecordType::Interest;
        candidate.side = None;
        candidate.transaction_date = None;
        let failures = check_details("fixture_fake", &candidate).unwrap();
        assert!(
            failures.iter().any(|f| f.contains("no details schema")),
            "missing contract must fail closed: {failures:?}"
        );
    }

    #[test]
    fn json_diff_is_none_for_deep_equal_values() {
        let a = json!({"x": [1, 2], "y": "z"});
        let b = json!({"y": "z", "x": [1, 2]});
        assert_eq!(json_diff("t", &a, &b), None);
    }

    #[test]
    fn json_diff_prints_a_unified_diff() {
        let expected = json!({"amount": "$1,001 - $14,000"});
        let actual = json!({"amount": "$1,001 - $15,000"});
        let diff = json_diff("expected.silver.json", &expected, &actual).unwrap();
        assert!(diff.contains("@@"), "unified hunk header expected: {diff}");
        assert!(
            diff.lines()
                .any(|l| l.starts_with('-') && l.contains("$1,001 - $14,000")),
            "expected side must show as removal: {diff}"
        );
        assert!(
            diff.lines()
                .any(|l| l.starts_with('+') && l.contains("$1,001 - $15,000")),
            "actual side must show as addition: {diff}"
        );
    }
}
