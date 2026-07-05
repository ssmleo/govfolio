//! Per-role mechanical scorers (goal 016). Every check is deterministic and
//! world-verifying: filesystem reads, hash comparisons, schema validation,
//! and real command invocations — never an LLM judgment. Scorers only READ
//! the reference artifacts; a defect found in one is surfaced as a failing
//! check detail (a finding), never fixed here.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr as _;

use serde_json::Value;

use govfolio_core::domain::enums::RecordType;
use govfolio_core::domain::gold::GoldCandidate;

use super::{Check, INNER_ENV, Outcome};
use crate::conformance::check_details;
use crate::factory;

/// `docs/regimes/us-house.md` — the E1 regime doc (spec-writer artifact).
fn regime_doc_path(root: &Path) -> PathBuf {
    root.join("docs").join("regimes").join("us-house.md")
}

/// `crates/adapters/us_house/fixtures`.
fn fixtures_root(root: &Path) -> PathBuf {
    root.join("crates")
        .join("adapters")
        .join("us_house")
        .join("fixtures")
}

// ---------------------------------------------------------------------------
// Roles without E1 reference artifacts (walking skeleton skipped the phases)
// ---------------------------------------------------------------------------

/// Scout: `docs/regimes/us_house/sources.yaml`. The walking skeleton never
/// ran a SCOUT phase (the source was founder-designated), so the artifact
/// does not exist → `NOT_APPLICABLE` (BLOCKING for E2 entry).
pub(super) fn scout(root: &Path) -> Outcome {
    let artifact = root
        .join("docs")
        .join("regimes")
        .join("us_house")
        .join("sources.yaml");
    if !artifact.is_file() {
        return Outcome::NotApplicable {
            reason: "no E1 reference artifact: docs/regimes/us_house/sources.yaml does not \
                     exist — the walking skeleton (goal 001) skipped the SCOUT phase (the \
                     us_house source was founder-designated). A scout run must produce a \
                     validated sources.yaml before scout can be scored."
                .to_owned(),
        };
    }
    validator_outcome(
        "sources_yaml_validates",
        &factory::validate_sources(root, "us_house"),
    )
}

/// Surveyor: `docs/regimes/us_house/AUTHORITY.md`. The E1 survey knowledge
/// lives in the regime doc (pre-dating the goal-015 AUTHORITY.md convention);
/// no artifact exists at the validator's canonical path → `NOT_APPLICABLE`.
pub(super) fn surveyor(root: &Path) -> Outcome {
    let artifact = root
        .join("docs")
        .join("regimes")
        .join("us_house")
        .join("AUTHORITY.md");
    if !artifact.is_file() {
        return Outcome::NotApplicable {
            reason: "no E1 reference artifact: docs/regimes/us_house/AUTHORITY.md does not \
                     exist — the walking skeleton folded survey knowledge into \
                     docs/regimes/us-house.md before the goal-015 AUTHORITY.md convention \
                     (and its {url, file} evidence schema) existed. A surveyor run must \
                     produce a validating AUTHORITY.md before surveyor can be scored."
                .to_owned(),
        };
    }
    validator_outcome(
        "authority_md_validates",
        &factory::validate_survey(root, "us_house"),
    )
}

/// Sampler: fixtures exist, but the capture manifest attributes them to
/// test-designer (goal 001 T8b) — the SAMPLE phase was skipped, so there is
/// no sampler-produced reference artifact → `NOT_APPLICABLE`.
pub(super) fn sampler(root: &Path) -> Outcome {
    let manifest = fixtures_root(root).join("MANIFEST.json");
    let captured_by = fs::read_to_string(&manifest)
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|doc| {
            doc.get("captured_by")
                .and_then(Value::as_str)
                .map(str::to_owned)
        });
    let attribution = captured_by.unwrap_or_else(|| "<missing captured_by>".to_owned());
    if !attribution.trim_start().starts_with("sampler") {
        return Outcome::NotApplicable {
            reason: format!(
                "no E1 reference artifact from a sampler run: the capture manifest \
                 attributes the fixtures to {attribution:?} — the walking skeleton \
                 skipped the SAMPLE phase (test-designer captured under goal 001 T8b). \
                 A sampler-attributed capture manifest must exist before sampler can \
                 be scored."
            ),
        };
    }
    validator_outcome(
        "capture_manifest_validates",
        &factory::validate_manifest(root, "us_house"),
    )
}

/// Collapses a factory validator run into a single pass/fail check: these
/// validators are already fail-closed gates, so clean = 1.0, else 0.0.
fn validator_outcome(name: &'static str, failures: &[String]) -> Outcome {
    let passed = failures.is_empty();
    let detail = if passed {
        "validator clean".to_owned()
    } else {
        failures.join("; ")
    };
    Outcome::Scored {
        checks: vec![Check::new(name, passed, detail)],
    }
}

// ---------------------------------------------------------------------------
// spec-writer: regime-doc structural completeness
// ---------------------------------------------------------------------------

/// Body sections the E1 regime doc establishes as the reference structure.
const SPEC_SECTIONS: &[&str] = &[
    "## 1.",
    "## 2.",
    "## 3.",
    "## 4.",
    "## 5.",
    "## 6.",
    "## 7.",
    "## 8.",
    "## Quirks log",
    "## Operational notes",
];

pub(super) fn spec_writer(root: &Path) -> Outcome {
    let path = regime_doc_path(root);
    let Ok(text) = fs::read_to_string(&path) else {
        return Outcome::Scored {
            checks: vec![Check::new(
                "regime_doc_readable",
                false,
                format!("missing regime doc at {}", path.display()),
            )],
        };
    };
    let mut checks = Vec::new();
    let front = front_matter_value(&text);
    checks.push(Check::new(
        "front_matter_parses",
        front.is_some(),
        "YAML front-matter delimited by --- lines parses",
    ));
    if let Some(front) = &front {
        checks.push(survey_keys_check(front));
        checks.push(record_types_check(front));
        checks.push(band_table_check(front));
    }
    checks.push(sections_check(&text));
    checks.push(mapping_table_check(
        &text,
        "owner_map_table",
        "### 3.3",
        4,
        &[],
    ));
    checks.push(mapping_table_check(
        &text,
        "side_map_table",
        "### 3.4",
        4,
        &["buy", "sell"],
    ));
    checks.push(mapping_table_check(
        &text,
        "asset_class_table",
        "### 3.6",
        5,
        &["equity", "other"],
    ));
    checks.push(fixture_pins_check(root, &text));
    checks.push(evidence_log_check(&text));
    Outcome::Scored { checks }
}

fn front_matter_value(text: &str) -> Option<Value> {
    factory::front_matter(text).and_then(|front| serde_norway::from_str::<Value>(front).ok())
}

fn survey_keys_check(front: &Value) -> Check {
    let missing: Vec<&str> = factory::SURVEY_KEYS
        .iter()
        .copied()
        .filter(|key| front.get(key).is_none())
        .collect();
    Check::new(
        "survey_keys_complete",
        missing.is_empty(),
        if missing.is_empty() {
            format!(
                "all {} RegimeSurvey keys present",
                factory::SURVEY_KEYS.len()
            )
        } else {
            format!("missing RegimeSurvey keys: {}", missing.join(", "))
        },
    )
}

fn record_types_check(front: &Value) -> Check {
    let types = front.get("record_types").and_then(Value::as_array);
    let (passed, detail) = match types {
        Some(items) if !items.is_empty() => {
            let bad: Vec<String> = items
                .iter()
                .filter(|item| serde_json::from_value::<RecordType>((*item).clone()).is_err())
                .map(std::string::ToString::to_string)
                .collect();
            if bad.is_empty() {
                (
                    true,
                    format!("{} record type(s), all in vocabulary", items.len()),
                )
            } else {
                (
                    false,
                    format!("out-of-vocabulary record types: {}", bad.join(", ")),
                )
            }
        }
        _ => (false, "record_types missing or empty".to_owned()),
    };
    Check::new("record_types_in_vocabulary", passed, detail)
}

/// Banded regime: every band row carries a verbatim `raw` string and
/// decimal-string bounds (`high` null only for the open-ended band).
fn band_table_check(front: &Value) -> Check {
    let Some(rows) = front.get("band_table").and_then(Value::as_array) else {
        return Check::new("band_table_well_formed", false, "band_table missing");
    };
    if rows.is_empty() {
        return Check::new("band_table_well_formed", false, "band_table empty");
    }
    let mut problems = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let raw_ok = row
            .get("raw")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty());
        if !raw_ok {
            problems.push(format!("band_table[{i}].raw missing/empty"));
        }
        if !decimal_string(row.get("low")) {
            problems.push(format!("band_table[{i}].low is not a decimal string"));
        }
        let high_ok = row
            .get("high")
            .is_some_and(|h| h.is_null() || decimal_string(Some(h)));
        if !high_ok {
            problems.push(format!(
                "band_table[{i}].high is not a decimal string or null"
            ));
        }
    }
    Check::new(
        "band_table_well_formed",
        problems.is_empty(),
        if problems.is_empty() {
            format!("{} bands, decimal-string bounds", rows.len())
        } else {
            problems.join("; ")
        },
    )
}

fn decimal_string(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .is_some_and(|s| rust_decimal::Decimal::from_str(s).is_ok())
}

fn sections_check(text: &str) -> Check {
    let missing: Vec<&str> = SPEC_SECTIONS
        .iter()
        .copied()
        .filter(|prefix| !text.lines().any(|l| l.trim_start().starts_with(prefix)))
        .collect();
    Check::new(
        "body_sections_present",
        missing.is_empty(),
        if missing.is_empty() {
            format!("all {} reference sections present", SPEC_SECTIONS.len())
        } else {
            format!("missing sections: {}", missing.join(", "))
        },
    )
}

/// A mapping table under `heading` parses with at least `min_rows` data rows
/// and contains every required token.
fn mapping_table_check(
    text: &str,
    name: &'static str,
    heading: &str,
    min_rows: usize,
    required_tokens: &[&str],
) -> Check {
    let Some(section_text) = section(text, heading) else {
        return Check::new(name, false, format!("section {heading} not found"));
    };
    let rows = table_data_rows(&section_text);
    if rows.len() < min_rows {
        return Check::new(
            name,
            false,
            format!("{heading}: {} data row(s), need >= {min_rows}", rows.len()),
        );
    }
    let missing: Vec<&str> = required_tokens
        .iter()
        .copied()
        .filter(|token| !rows.iter().flatten().any(|cell| cell.contains(token)))
        .collect();
    Check::new(
        name,
        missing.is_empty(),
        if missing.is_empty() {
            format!(
                "{heading}: {} data rows, tokens {required_tokens:?} present",
                rows.len()
            )
        } else {
            format!("{heading}: missing mapping tokens: {}", missing.join(", "))
        },
    )
}

/// Every on-disk fixture input hashes to a sha256 pinned in regime doc §7.
fn fixture_pins_check(root: &Path, text: &str) -> Check {
    let Some(section_text) = section(text, "## 7.") else {
        return Check::new("fixture_pins_verified", false, "section ## 7. not found");
    };
    let pins = hex64_tokens(&section_text);
    let dirs = fixture_case_dirs(root);
    if dirs.is_empty() {
        return Check::new(
            "fixture_pins_verified",
            false,
            "no fixture case directories on disk",
        );
    }
    let mut problems = Vec::new();
    for dir in &dirs {
        let case = dir.file_name().map_or_else(
            || dir.display().to_string(),
            |n| n.to_string_lossy().into_owned(),
        );
        match factory::single_input(dir).map(|input| fs::read(&input)) {
            Ok(Ok(bytes)) => {
                let sha = factory::sha256_hex(&bytes);
                if !pins.contains(&sha) {
                    problems.push(format!("{case}: input hashes to {sha}, not pinned in §7"));
                }
            }
            Ok(Err(e)) => problems.push(format!("{case}: input unreadable: {e}")),
            Err(e) => problems.push(format!("{case}: {e}")),
        }
    }
    Check::new(
        "fixture_pins_verified",
        problems.is_empty(),
        if problems.is_empty() {
            format!(
                "{} fixture input(s) all pinned among {} §7 hash(es)",
                dirs.len(),
                pins.len()
            )
        } else {
            problems.join("; ")
        },
    )
}

/// §8 evidence log: a populated table with archived-source URLs.
fn evidence_log_check(text: &str) -> Check {
    let Some(section_text) = section(text, "## 8.") else {
        return Check::new("evidence_log_populated", false, "section ## 8. not found");
    };
    let rows = table_data_rows(&section_text);
    let url_count = section_text.matches("https://").count();
    let sha_count = hex64_tokens(&section_text).len();
    let passed = rows.len() >= 12 && url_count >= 4 && sha_count >= 4;
    Check::new(
        "evidence_log_populated",
        passed,
        format!(
            "{} evidence rows, {url_count} https URLs, {sha_count} sha256 pins \
             (need >=12 rows, >=4 URLs, >=4 pins)",
            rows.len()
        ),
    )
}

// ---------------------------------------------------------------------------
// test-designer: fixtures/manifest validity + expected.* contract validity
// ---------------------------------------------------------------------------

pub(super) fn test_designer(root: &Path) -> Outcome {
    let mut checks = Vec::new();
    checks.push(validator_check(
        "manifest_validates",
        &factory::validate_manifest(root, "us_house"),
    ));
    let manifest_cases = load_manifest_cases(root);
    let Some(cases) = manifest_cases else {
        checks.push(Check::new(
            "manifest_cases_readable",
            false,
            "MANIFEST.json missing, unparseable, or without a cases object",
        ));
        return Outcome::Scored { checks };
    };
    checks.push(silver_shape_check(root, &cases));
    checks.push(silver_counts_check(root, &cases));
    checks.extend(gold_checks(root, &cases));
    checks.push(pins_match_regime_doc_check(root, &cases));
    Outcome::Scored { checks }
}

fn validator_check(name: &'static str, failures: &[String]) -> Check {
    let passed = failures.is_empty();
    let detail = if passed {
        "validator clean".to_owned()
    } else {
        failures.join("; ")
    };
    Check::new(name, passed, detail)
}

/// Manifest `cases` map: case name → case object.
fn load_manifest_cases(root: &Path) -> Option<serde_json::Map<String, Value>> {
    let text = fs::read_to_string(fixtures_root(root).join("MANIFEST.json")).ok()?;
    let doc: Value = serde_json::from_str(&text).ok()?;
    doc.get("cases")?.as_object().cloned()
}

fn expected_json(root: &Path, case: &str, file: &str) -> Result<Vec<Value>, String> {
    let path = fixtures_root(root).join(case).join(file);
    let text = fs::read_to_string(&path).map_err(|e| format!("{case}/{file}: unreadable: {e}"))?;
    let doc: Value =
        serde_json::from_str(&text).map_err(|e| format!("{case}/{file}: not JSON: {e}"))?;
    match doc {
        Value::Array(items) if !items.is_empty() => Ok(items),
        Value::Array(_) => Err(format!("{case}/{file}: empty array (fail closed)")),
        _ => Err(format!("{case}/{file}: not a JSON array")),
    }
}

/// Every expected.silver.json row is a `{payload: object, confidence: number}`
/// wrapper (the MANIFEST.json conformance convention).
fn silver_shape_check(root: &Path, cases: &serde_json::Map<String, Value>) -> Check {
    let mut problems = Vec::new();
    for case in cases.keys() {
        match expected_json(root, case, "expected.silver.json") {
            Ok(rows) => {
                for (i, row) in rows.iter().enumerate() {
                    let payload_ok = row.get("payload").is_some_and(Value::is_object);
                    let confidence_ok = row.get("confidence").is_some_and(Value::is_number);
                    if !(payload_ok && confidence_ok) {
                        problems.push(format!(
                            "{case}/expected.silver.json[{i}]: not a {{payload, confidence}} wrapper"
                        ));
                    }
                }
            }
            Err(e) => problems.push(e),
        }
    }
    Check::new(
        "silver_wrapper_shape",
        problems.is_empty(),
        if problems.is_empty() {
            format!(
                "{} case(s): every silver row is a {{payload, confidence}} wrapper",
                cases.len()
            )
        } else {
            problems.join("; ")
        },
    )
}

/// expected.silver.json row counts match the manifest's declared `rows`.
fn silver_counts_check(root: &Path, cases: &serde_json::Map<String, Value>) -> Check {
    let mut problems = Vec::new();
    let mut total = 0_usize;
    for (case, spec) in cases {
        let declared = spec.get("rows").and_then(Value::as_u64);
        match (expected_json(root, case, "expected.silver.json"), declared) {
            (Ok(rows), Some(declared)) => {
                total += rows.len();
                if rows.len() as u64 != declared {
                    problems.push(format!(
                        "{case}: manifest declares {declared} row(s), expected.silver.json has {}",
                        rows.len()
                    ));
                }
            }
            (Ok(_), None) => problems.push(format!("{case}: manifest has no rows field")),
            (Err(e), _) => problems.push(e),
        }
    }
    Check::new(
        "silver_counts_match_manifest",
        problems.is_empty(),
        if problems.is_empty() {
            format!(
                "{total} silver row(s) across {} case(s) match manifest counts",
                cases.len()
            )
        } else {
            problems.join("; ")
        },
    )
}

/// expected.gold.json contract validity: schema-valid vs the committed
/// `GoldCandidate` snapshot, deserializes into the domain type, passes domain
/// validation, and satisfies the (`us_house`, transaction) details contract.
fn gold_checks(root: &Path, cases: &serde_json::Map<String, Value>) -> Vec<Check> {
    let mut schema_problems = Vec::new();
    let mut domain_problems = Vec::new();
    let mut details_problems = Vec::new();
    let validator = load_gold_schema_validator(root);
    if let Err(e) = &validator {
        schema_problems.push(format!("GoldCandidate schema snapshot unusable: {e}"));
    }
    let mut candidates = 0_usize;
    for case in cases.keys() {
        let items = match expected_json(root, case, "expected.gold.json") {
            Ok(items) => items,
            Err(e) => {
                schema_problems.push(e);
                continue;
            }
        };
        for (i, item) in items.iter().enumerate() {
            candidates += 1;
            let at = format!("{case}/expected.gold.json[{i}]");
            if let Ok(validator) = &validator {
                for err in validator.iter_errors(item) {
                    schema_problems.push(format!("{at}: schema: {err}"));
                }
            }
            match serde_json::from_value::<GoldCandidate>(item.clone()) {
                Ok(candidate) => {
                    if let Err(e) = candidate.validate() {
                        domain_problems.push(format!("{at}: domain validation: {e}"));
                    }
                    match check_details("us_house", &candidate) {
                        Ok(errors) => details_problems
                            .extend(errors.into_iter().map(|e| format!("{at}: {e}"))),
                        Err(e) => details_problems.push(format!("{at}: {e:#}")),
                    }
                }
                Err(e) => domain_problems.push(format!("{at}: does not deserialize: {e}")),
            }
        }
    }
    vec![
        summarize(
            "gold_schema_snapshot_valid",
            &schema_problems,
            format!("{candidates} candidate(s) valid vs crates/core/schemas/gold_candidate.json"),
        ),
        summarize(
            "gold_candidates_deserialize_and_validate",
            &domain_problems,
            format!("{candidates} candidate(s) deserialize + pass domain validation"),
        ),
        summarize(
            "details_contract_valid",
            &details_problems,
            format!(
                "{candidates} candidate(s) satisfy the (us_house, transaction) details contract"
            ),
        ),
    ]
}

fn load_gold_schema_validator(root: &Path) -> Result<jsonschema::Validator, String> {
    let path = root
        .join("crates")
        .join("core")
        .join("schemas")
        .join("gold_candidate.json");
    let text = fs::read_to_string(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let schema: Value =
        serde_json::from_str(&text).map_err(|e| format!("parsing {}: {e}", path.display()))?;
    jsonschema::validator_for(&schema).map_err(|e| format!("compiling gold schema: {e}"))
}

fn summarize(name: &'static str, problems: &[String], ok_detail: String) -> Check {
    let passed = problems.is_empty();
    Check::new(
        name,
        passed,
        if passed {
            ok_detail
        } else {
            problems.join("; ")
        },
    )
}

/// Every manifest case sha256 appears among the regime doc §7 pins.
fn pins_match_regime_doc_check(root: &Path, cases: &serde_json::Map<String, Value>) -> Check {
    let doc = fs::read_to_string(regime_doc_path(root)).unwrap_or_default();
    let pins = section(&doc, "## 7.")
        .map(|s| hex64_tokens(&s))
        .unwrap_or_default();
    let mut problems = Vec::new();
    for (case, spec) in cases {
        match spec.get("sha256").and_then(Value::as_str) {
            Some(sha) if pins.contains(&sha.to_ascii_lowercase()) => {}
            Some(sha) => problems.push(format!("{case}: manifest sha {sha} not pinned in §7")),
            None => problems.push(format!("{case}: manifest has no sha256")),
        }
    }
    Check::new(
        "manifest_pins_match_regime_doc",
        problems.is_empty(),
        if problems.is_empty() {
            format!("{} case pin(s) all present in regime doc §7", cases.len())
        } else {
            problems.join("; ")
        },
    )
}

// ---------------------------------------------------------------------------
// rust-builder: conformance 4/4 + the full gate command block, for real
// ---------------------------------------------------------------------------

pub(super) fn rust_builder(root: &Path) -> Outcome {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned());
    // The nested workspace test run gets its own target dir: when this scorer
    // executes inside `cargo test` (the role_evals suite), the default target
    // dir would make the nested build relink the currently RUNNING test exe,
    // which Windows rejects (locked file). Deterministic isolation beats
    // environment sniffing; the extra dir lives under target/ (gitignored).
    let nested_target = root
        .join("target")
        .join("role-evals-nested")
        .to_string_lossy()
        .into_owned();
    let checks = vec![
        run_gate_command(
            root,
            "conformance_us_house_4_of_4",
            &cargo,
            &[
                "run",
                "--quiet",
                "-p",
                "pipeline",
                "--bin",
                "conformance",
                "--",
                "us_house",
            ],
            &[],
            Some("4/4 cases green"),
        ),
        run_gate_command(
            root,
            "cargo_fmt_check",
            &cargo,
            &["fmt", "--check"],
            &[],
            None,
        ),
        run_gate_command(
            root,
            "cargo_clippy_deny_warnings",
            &cargo,
            &["clippy", "--all-targets", "--", "-D", "warnings"],
            &[],
            None,
        ),
        // INNER_ENV stops the role_evals gate test from recursing.
        run_gate_command(
            root,
            "cargo_test_workspace",
            &cargo,
            &["test", "--workspace"],
            &[
                (INNER_ENV, "1"),
                ("CARGO_TARGET_DIR", nested_target.as_str()),
            ],
            None,
        ),
    ];
    Outcome::Scored { checks }
}

/// Runs one real gate command; passes on exit 0 (and, when given, a required
/// stdout marker). Failure detail carries the output tail.
fn run_gate_command(
    root: &Path,
    name: &'static str,
    program: &str,
    args: &[&str],
    envs: &[(&str, &str)],
    stdout_marker: Option<&str>,
) -> Check {
    let mut command = Command::new(program);
    command.args(args).current_dir(root);
    for (key, value) in envs {
        command.env(key, value);
    }
    match command.output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let marker_ok = stdout_marker.is_none_or(|marker| stdout.contains(marker));
            let passed = output.status.success() && marker_ok;
            let detail = if passed {
                format!("`{program} {}` exit 0", args.join(" "))
            } else {
                format!(
                    "`{program} {}` status {:?}, marker {stdout_marker:?} found: {marker_ok}\n\
                     stdout tail: {}\nstderr tail: {}",
                    args.join(" "),
                    output.status.code(),
                    tail(&stdout),
                    tail(&stderr),
                )
            };
            Check::new(name, passed, detail)
        }
        Err(e) => Check::new(name, false, format!("failed to spawn {program}: {e}")),
    }
}

/// Last ~2000 chars of command output (failure evidence without flooding).
fn tail(text: &str) -> &str {
    let len = text.len();
    if len <= 2000 {
        return text.trim_end();
    }
    let mut start = len - 2000;
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

// ---------------------------------------------------------------------------
// auditor: audit journal line + goal-file findings sections
// ---------------------------------------------------------------------------

pub(super) fn auditor(root: &Path) -> Outcome {
    let mut checks = Vec::new();
    let journal = fs::read_to_string(root.join("agents").join("JOURNAL.md")).unwrap_or_default();
    let audit_line = journal
        .lines()
        .find(|l| l.contains("AUDIT") && l.contains("T8d"));
    checks.push(Check::new(
        "journal_audit_line_exists",
        audit_line.is_some(),
        audit_line.map_or_else(
            || "no `AUDIT ... T8d` line in agents/JOURNAL.md".to_owned(),
            |l| format!("found: {}", tail_line(l)),
        ),
    ));
    checks.push(Check::new(
        "journal_verdict_explicit",
        audit_line.is_some_and(|l| l.contains("PASS") || l.contains("BOUNCE")),
        "journal audit line carries an explicit PASS/BOUNCE verdict",
    ));
    let goal = fs::read_to_string(
        root.join("agents")
            .join("goals")
            .join("001-walking-skeleton.md"),
    )
    .unwrap_or_default();
    let block = t8d_block(&goal);
    checks.push(Check::new(
        "goal_findings_block_exists",
        block.is_some(),
        "goal 001 has a ticked `- [x] T8d` findings block",
    ));
    let block_text = block.unwrap_or_default();
    checks.push(Check::new(
        "goal_verdict_pass",
        block_text.contains("PASS"),
        "T8d block records the PASS verdict (reference corpus must be audit-green)",
    ));
    checks.push(Check::new(
        "goal_integrity_evidence_recorded",
        block_text.contains("independent") && block_text.contains("predates"),
        "T8d block records independent re-derivation + fixture-commit-order integrity \
         (adversarial-verification discipline)",
    ));
    checks.push(Check::new(
        "goal_findings_notes_present",
        block_text.contains("Non-blocking notes"),
        "T8d block surfaces findings (non-blocking notes) instead of suppressing them",
    ));
    Outcome::Scored { checks }
}

/// The `- [x] T8d ...` block: from its line to the next `## ` heading or
/// next top-level checklist item.
fn t8d_block(goal: &str) -> Option<String> {
    let lines: Vec<&str> = goal.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim_start().starts_with("- [x] T8d"))?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| l.starts_with("## ") || l.trim_start().starts_with("- [x] T"))
        .map_or(lines.len(), |(i, _)| i);
    Some(lines[start..end].join("\n"))
}

fn tail_line(line: &str) -> String {
    let trimmed = line.trim();
    if trimmed.len() <= 120 {
        trimmed.to_owned()
    } else {
        let mut end = 120;
        while !trimmed.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &trimmed[..end])
    }
}

// ---------------------------------------------------------------------------
// Shared markdown/file helpers
// ---------------------------------------------------------------------------

/// Fixture case directories on disk.
fn fixture_case_dirs(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(fixtures_root(root))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs
}

/// The section starting at the first line prefixed `heading`, ending before
/// the next heading of the same or shallower level.
fn section(text: &str, heading: &str) -> Option<String> {
    let level = heading.chars().take_while(|c| *c == '#').count();
    let lines: Vec<&str> = text.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.trim_start().starts_with(heading))?;
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, l)| {
            let lv = l.trim_start().chars().take_while(|c| *c == '#').count();
            lv >= 1 && lv <= level
        })
        .map_or(lines.len(), |(i, _)| i);
    Some(lines[start..end].join("\n"))
}

/// Markdown table data rows (header + separator rows dropped).
fn table_data_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = trimmed
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_owned())
            .collect();
        let separator = cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| matches!(ch, '-' | ':')));
        if !separator {
            rows.push(cells);
        }
    }
    if rows.is_empty() {
        rows
    } else {
        rows.split_off(1)
    }
}

/// Every distinct 64-char lowercase-hex token in `text`.
fn hex64_tokens(text: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut run = String::new();
    for ch in text.chars() {
        if ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase() {
            run.push(ch);
        } else {
            if run.len() == 64 {
                out.insert(run.clone());
            }
            run.clear();
        }
    }
    if run.len() == 64 {
        out.insert(run);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn section_ends_at_same_or_shallower_heading() {
        let text = "## 7. Fixtures\n| a | b |\n|---|---|\n| 1 | 2 |\n## 8. Evidence\n| c |\n";
        let seven = section(text, "## 7.").unwrap();
        assert!(seven.contains("| 1 | 2 |"));
        assert!(!seven.contains("Evidence"));
        let sub = "## 3\n### 3.3 Map\n| x | y |\n|---|---|\n| p | q |\n### 3.4 Next\n";
        let map = section(sub, "### 3.3").unwrap();
        assert!(map.contains("| p | q |"));
        assert!(!map.contains("3.4"));
    }

    #[test]
    fn table_data_rows_drop_header_and_separator() {
        let text = "| H1 | H2 |\n|---|---|\n| a | b |\n| c | d |\n";
        let rows = table_data_rows(text);
        assert_eq!(rows, vec![vec!["a", "b"], vec!["c", "d"]]);
    }

    #[test]
    fn hex64_tokens_extracts_exact_length_runs() {
        let sha = "4a12b888c2c89ebbfad5c280fa8a6af52489218dbec402ca2abc803436d8fa3f";
        let text = format!(
            "pin `{sha}` and short deadbeef and upper {}",
            sha.to_uppercase()
        );
        let tokens = hex64_tokens(&text);
        assert_eq!(tokens.len(), 1);
        assert!(tokens.contains(sha));
    }

    #[test]
    fn t8d_block_stops_at_next_checklist_item() {
        let goal = "- [x] T8c built\n- [x] T8d audit — PASS\n  - independent, predates\n  - Non-blocking notes: x\n- [x] T9 next\n";
        let block = t8d_block(goal).unwrap();
        assert!(block.contains("PASS"));
        assert!(block.contains("Non-blocking notes"));
        assert!(!block.contains("T9"));
    }
}
