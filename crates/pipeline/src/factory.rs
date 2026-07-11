//! Coverage-factory phase gates (design §5.8, goal 015): a phase is done when
//! its artifact validates. Three validators, one per gated artifact:
//!
//! - Scout:    `docs/regimes/<x>/sources.yaml`            → [`validate_sources`]
//! - Surveyor: `docs/regimes/<x>/AUTHORITY.md` front-matter → [`validate_survey`]
//! - Sampler:  `crates/adapters/<x>/fixtures/` + manifest  → [`validate_manifest`]
//!
//! All three are FAIL-CLOSED: a missing file, missing section, missing
//! evidence URL, or anything outside the documented schema rejects. `unknown`
//! claims are legal only with a what-was-tried log. The authoritative artifact
//! schemas live in `agents/workflows/source-exploration.md`.
//!
//! Each validator returns human-readable failure strings; empty means valid.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use govfolio_core::domain::enums::RecordType;

/// `disclosure_regime.value_precision` CHECK vocabulary (migration 0001).
const VALUE_PRECISION: &[&str] = &["exact", "banded", "categorical", "none"];

/// Every `RegimeSurvey` front-matter key (template:
/// `docs/regimes/_templates/AUTHORITY.template.md`). All required; anything
/// else rejects (fail closed). Shared with the role-eval harness (goal 016).
pub(crate) const SURVEY_KEYS: &[&str] = &[
    "jurisdiction",
    "bodies",
    "legal_basis",
    "who_files",
    "record_types",
    "value_precision",
    "band_table",
    "cadence_and_lag",
    "formats",
    "access",
    "historical_depth",
    "identifiers_available",
    "amendment_mechanism",
    "personal_data_to_redact",
    "tos_and_politeness",
    "language",
    "open_questions",
    "regime_versions",
];

/// Claim-shaped survey fields: `{claim, evidence, tried?}`.
const SURVEY_CLAIM_FIELDS: &[&str] = &[
    "legal_basis",
    "who_files",
    "cadence_and_lag",
    "amendment_mechanism",
    "tos_and_politeness",
];

/// Required `AUTHORITY.md` body sections (template headings; prefix-matched
/// so parenthetical subtitles may evolve).
const SURVEY_SECTIONS: &[&str] = &[
    "## Data catalog",
    "## Field mapping",
    "## Parse strategy",
    "## Quirks log",
    "## Operational notes",
];

/// Validates the Scout artifact `docs/regimes/<x>/sources.yaml`.
#[must_use]
pub fn validate_sources(root: &Path, jurisdiction: &str) -> Vec<String> {
    let mut fails = Vec::new();
    let dir = regime_dir(root, jurisdiction);
    let label = format!("docs/regimes/{jurisdiction}/sources.yaml");
    let Ok(text) = fs::read_to_string(dir.join("sources.yaml")) else {
        fails.push(format!("missing {label} (fail closed)"));
        return fails;
    };
    let Some(doc) = parse_yaml(&text, &label, &mut fails) else {
        return fails;
    };
    let Some(map) = as_object(&doc, &label, &mut fails) else {
        return fails;
    };
    reject_unknown_keys(
        map,
        &["jurisdiction", "candidates", "notes"],
        &label,
        &mut fails,
    );
    check_self_identification(map, jurisdiction, &mut fails);
    let evidence_dir = dir.join("evidence");
    let Some(candidates) = required(map, "candidates", "", &mut fails)
        .and_then(|v| as_array(v, "candidates", &mut fails))
    else {
        return fails;
    };
    if candidates.is_empty() {
        fails.push(
            "candidates: at least one official-source candidate required (fail closed)".to_owned(),
        );
    }
    for (i, candidate) in candidates.iter().enumerate() {
        let at = format!("candidates[{i}]");
        let Some(entry) = as_object(candidate, &at, &mut fails) else {
            continue;
        };
        reject_unknown_keys(
            entry,
            &["url", "contains", "official_rationale", "evidence", "notes"],
            &at,
            &mut fails,
        );
        if let Some(url) = required(entry, "url", &at, &mut fails) {
            http_url(url, &join_at(&at, "url"), &mut fails);
        }
        for key in ["contains", "official_rationale"] {
            if let Some(v) = required(entry, key, &at, &mut fails) {
                nonempty_str(v, &join_at(&at, key), &mut fails);
            }
        }
        let evidence_at = join_at(&at, "evidence");
        if let Some(evidence) = required(entry, "evidence", &at, &mut fails)
            .and_then(|v| as_array(v, &evidence_at, &mut fails))
        {
            if evidence.is_empty() {
                fails.push(format!(
                    "{evidence_at}: at least one archived-evidence file required (fail closed)"
                ));
            }
            for (k, item) in evidence.iter().enumerate() {
                let item_at = format!("{evidence_at}[{k}]");
                if let Some(name) = nonempty_str(item, &item_at, &mut fails) {
                    check_evidence_file(name, &evidence_dir, jurisdiction, &item_at, &mut fails);
                }
            }
        }
    }
    fails
}

/// Validates the Surveyor artifact: `docs/regimes/<x>/AUTHORITY.md`
/// front-matter against the `RegimeSurvey` schema, plus the required body
/// sections.
#[must_use]
pub fn validate_survey(root: &Path, jurisdiction: &str) -> Vec<String> {
    let mut fails = Vec::new();
    let dir = regime_dir(root, jurisdiction);
    let label = format!("docs/regimes/{jurisdiction}/AUTHORITY.md");
    let Ok(text) = fs::read_to_string(dir.join("AUTHORITY.md")) else {
        fails.push(format!("missing {label} (fail closed)"));
        return fails;
    };
    for section in SURVEY_SECTIONS {
        if !text.lines().any(|l| l.trim_start().starts_with(section)) {
            fails.push(format!(
                "body: missing required section `{section}` (template: docs/regimes/_templates/AUTHORITY.template.md)"
            ));
        }
    }
    let Some(front) = front_matter(&text) else {
        fails.push(format!(
            "{label}: missing YAML front-matter delimited by `---` lines (fail closed)"
        ));
        return fails;
    };
    let Some(doc) = parse_yaml(front, "front-matter", &mut fails) else {
        return fails;
    };
    let Some(map) = as_object(&doc, "front-matter", &mut fails) else {
        return fails;
    };
    reject_unknown_keys(map, SURVEY_KEYS, "front-matter", &mut fails);
    check_self_identification(map, jurisdiction, &mut fails);
    required_str_list(map, "bodies", &mut fails);
    let evidence_dir = dir.join("evidence");
    for field in SURVEY_CLAIM_FIELDS {
        check_claim(map, field, "claim", &evidence_dir, jurisdiction, &mut fails);
    }
    check_claim(
        map,
        "historical_depth",
        "from",
        &evidence_dir,
        jurisdiction,
        &mut fails,
    );
    check_record_types(map, &mut fails);
    check_value_precision(map, &mut fails);
    required_str_list(map, "formats", &mut fails);
    check_access(map, &mut fails);
    check_identifiers(map, &mut fails);
    if let Some(redact) = required(map, "personal_data_to_redact", "", &mut fails) {
        // May be empty (an affirmative "nothing to redact"), but must be a list.
        if let Some(items) = as_array(redact, "personal_data_to_redact", &mut fails) {
            for (i, item) in items.iter().enumerate() {
                nonempty_str(item, &format!("personal_data_to_redact[{i}]"), &mut fails);
            }
        }
    }
    required_str_list(map, "language", &mut fails);
    check_open_questions(map, &mut fails);
    check_regime_versions(map, &evidence_dir, jurisdiction, &mut fails);
    fails
}

/// Validates the Sampler artifact: `crates/adapters/<x>/fixtures/*/input.*`
/// plus the capture manifest (`manifest.yaml` or `MANIFEST.json`).
#[must_use]
pub fn validate_manifest(root: &Path, jurisdiction: &str) -> Vec<String> {
    let mut fails = Vec::new();
    let fixtures = root
        .join("crates")
        .join("adapters")
        .join(jurisdiction)
        .join("fixtures");
    let rel = format!("crates/adapters/{jurisdiction}/fixtures");
    if !fixtures.is_dir() {
        fails.push(format!("missing {rel}/ (fail closed)"));
        return fails;
    }
    let spellings = ["manifest.yaml", "manifest.yml", "MANIFEST.json"];
    let present: Vec<&str> = spellings
        .iter()
        .copied()
        .filter(|name| fixtures.join(name).is_file())
        .collect();
    let manifest_name = match present.as_slice() {
        [] => {
            fails.push(format!(
                "missing capture manifest: {rel}/manifest.yaml or MANIFEST.json (fail closed)"
            ));
            return fails;
        }
        [one] => *one,
        several => {
            fails.push(format!(
                "ambiguous capture manifest: found {several:?} under {rel}/ — keep exactly one (fail closed)"
            ));
            return fails;
        }
    };
    let label = format!("{rel}/{manifest_name}");
    let Ok(text) = fs::read_to_string(fixtures.join(manifest_name)) else {
        fails.push(format!("unreadable {label} (fail closed)"));
        return fails;
    };
    let doc = if manifest_name == "MANIFEST.json" {
        match serde_json::from_str::<Value>(&text) {
            Ok(doc) => Some(doc),
            Err(e) => {
                fails.push(format!("{label}: not parseable JSON: {e}"));
                None
            }
        }
    } else {
        parse_yaml(&text, &label, &mut fails)
    };
    let Some(doc) = doc else { return fails };
    let Some(map) = as_object(&doc, &label, &mut fails) else {
        return fails;
    };
    // Required keys only; extra provenance/notes keys are welcome here — the
    // manifest doubles as the capture record (see the us_house reference).
    if let Some(stamp) = required(map, "captured_at_utc", "", &mut fails)
        .and_then(|v| nonempty_str(v, "captured_at_utc", &mut fails))
        && chrono::DateTime::parse_from_rfc3339(stamp).is_err()
    {
        fails.push(format!(
            "captured_at_utc: {stamp:?} is not an RFC3339 timestamp"
        ));
    }
    if let Some(politeness) = required(map, "politeness", "", &mut fails)
        .and_then(|v| as_object(v, "politeness", &mut fails))
        && let Some(ua) = required(politeness, "user_agent", "politeness", &mut fails)
    {
        nonempty_str(ua, "politeness.user_agent", &mut fails);
    }
    let mut declared = BTreeSet::new();
    if let Some(cases) =
        required(map, "cases", "", &mut fails).and_then(|v| as_object(v, "cases", &mut fails))
    {
        if cases.len() < 3 {
            fails.push(format!(
                "cases: at least 3 representative filings required (typical, amendment/correction, edge case) — got {}",
                cases.len()
            ));
        }
        for (case_name, case_value) in cases {
            declared.insert(case_name.clone());
            let at = format!("cases.{case_name}");
            let Some(case) = as_object(case_value, &at, &mut fails) else {
                continue;
            };
            check_case(case, &at, &fixtures, &rel, case_name, &mut fails);
        }
    }
    // Bijection: every fixture case directory on disk must be declared.
    if let Ok(entries) = fs::read_dir(&fixtures) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !declared.contains(&name) {
                    fails.push(format!(
                        "{rel}/{name}/: fixture case on disk has no manifest entry (fail closed)"
                    ));
                }
            }
        }
    }
    fails
}

/// One manifest case: url + sha256 required; the fixture directory must hold
/// exactly one `input.*` whose bytes hash to the declared sha256 (raw is
/// sacred — invariant 2).
fn check_case(
    case: &Map<String, Value>,
    at: &str,
    fixtures: &Path,
    rel: &str,
    case_name: &str,
    fails: &mut Vec<String>,
) {
    if let Some(url) = required(case, "url", at, fails) {
        http_url(url, &join_at(at, "url"), fails);
    }
    let sha_at = join_at(at, "sha256");
    let declared_sha = required(case, "sha256", at, fails)
        .and_then(|v| nonempty_str(v, &sha_at, fails))
        .filter(|sha| {
            let well_formed = sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit());
            if !well_formed {
                fails.push(format!("{sha_at}: must be 64 hex chars"));
            }
            well_formed
        });
    let case_dir = fixtures.join(case_name);
    if !case_dir.is_dir() {
        fails.push(format!(
            "{at}: no fixture directory {rel}/{case_name}/ on disk (fail closed)"
        ));
        return;
    }
    let input = match single_input(&case_dir) {
        Ok(input) => input,
        Err(problem) => {
            fails.push(format!("{at}: {problem}"));
            return;
        }
    };
    if let Some(sha) = declared_sha {
        match fs::read(&input) {
            Ok(bytes) => {
                let actual = sha256_hex(&bytes);
                if !actual.eq_ignore_ascii_case(sha) {
                    fails.push(format!(
                        "{sha_at}: manifest says {sha} but the input file hashes to {actual} (fail closed)"
                    ));
                }
            }
            Err(e) => fails.push(format!("{at}: unreadable {}: {e}", input.display())),
        }
    }
}

// ---------------------------------------------------------------------------
// Survey field checks
// ---------------------------------------------------------------------------

/// A claim-shaped field: `{<value_key>, evidence, tried?}`. Evidence is
/// mandatory unless the claim is literally "unknown", which instead demands a
/// non-empty what-was-tried log (unknown beats confabulated, design §5.8).
fn check_claim(
    map: &Map<String, Value>,
    field: &str,
    value_key: &str,
    evidence_dir: &Path,
    jurisdiction: &str,
    fails: &mut Vec<String>,
) {
    let Some(obj) = required(map, field, "", fails).and_then(|v| as_object(v, field, fails)) else {
        return;
    };
    reject_unknown_keys(obj, &[value_key, "evidence", "tried"], field, fails);
    let claim = required(obj, value_key, field, fails)
        .and_then(|v| nonempty_str(v, &join_at(field, value_key), fails));
    let evidence_at = join_at(field, "evidence");
    let evidence =
        required(obj, "evidence", field, fails).and_then(|v| as_array(v, &evidence_at, fails));
    let is_unknown = claim.is_some_and(|c| c.eq_ignore_ascii_case("unknown"));
    if is_unknown {
        if !has_tried_log(obj) {
            fails.push(format!(
                "{field}: claim \"unknown\" is legal only with a non-empty `tried` log (design §5.8)"
            ));
        }
    } else if let Some(items) = evidence
        && items.is_empty()
    {
        fails.push(format!(
            "{evidence_at}: every claim carries evidence — at least one {{url, file}} entry (fail closed)"
        ));
    }
    if let Some(items) = evidence {
        for (i, item) in items.iter().enumerate() {
            check_evidence_ref(
                item,
                &format!("{evidence_at}[{i}]"),
                evidence_dir,
                jurisdiction,
                fails,
            );
        }
    }
}

/// True when `tried` is a non-empty list of non-empty strings.
fn has_tried_log(obj: &Map<String, Value>) -> bool {
    obj.get("tried")
        .and_then(Value::as_array)
        .is_some_and(|tried| {
            !tried.is_empty()
                && tried
                    .iter()
                    .all(|item| item.as_str().is_some_and(|s| !s.trim().is_empty()))
        })
}

/// A survey evidence item: `{url, file}` — the URL it came from and the
/// archived copy under `docs/regimes/<x>/evidence/`.
fn check_evidence_ref(
    item: &Value,
    at: &str,
    evidence_dir: &Path,
    jurisdiction: &str,
    fails: &mut Vec<String>,
) {
    let Some(obj) = item.as_object() else {
        fails.push(format!(
            "{at}: must be a {{url, file}} mapping (evidence or it didn't happen)"
        ));
        return;
    };
    reject_unknown_keys(obj, &["url", "file"], at, fails);
    if let Some(url) = required(obj, "url", at, fails) {
        http_url(url, &join_at(at, "url"), fails);
    }
    let file_at = join_at(at, "file");
    if let Some(name) =
        required(obj, "file", at, fails).and_then(|v| nonempty_str(v, &file_at, fails))
    {
        check_evidence_file(name, evidence_dir, jurisdiction, &file_at, fails);
    }
}

fn check_record_types(map: &Map<String, Value>, fails: &mut Vec<String>) {
    let Some(types) =
        required(map, "record_types", "", fails).and_then(|v| as_array(v, "record_types", fails))
    else {
        return;
    };
    if types.is_empty() {
        fails.push("record_types: at least one record type required (fail closed)".to_owned());
    }
    for (i, item) in types.iter().enumerate() {
        // Vocabulary single-sourced from the core domain enum (invariant 5).
        if serde_json::from_value::<RecordType>(item.clone()).is_err() {
            fails.push(format!(
                "record_types[{i}]: {item} is not a known record type \
                 (transaction|holding|interest|change_notification)"
            ));
        }
    }
}

fn check_value_precision(map: &Map<String, Value>, fails: &mut Vec<String>) {
    let precision = required(map, "value_precision", "", fails)
        .and_then(|v| nonempty_str(v, "value_precision", fails));
    if let Some(p) = precision
        && !VALUE_PRECISION.contains(&p)
    {
        fails.push(format!(
            "value_precision: {p:?} not in {}",
            VALUE_PRECISION.join("|")
        ));
    }
    if let Some(band_table) =
        required(map, "band_table", "", fails).and_then(|v| as_array(v, "band_table", fails))
        && precision == Some("banded")
        && band_table.is_empty()
    {
        fails.push(
            "band_table: value_precision is banded — the band table must be non-empty".to_owned(),
        );
    }
}

fn check_access(map: &Map<String, Value>, fails: &mut Vec<String>) {
    let Some(access) =
        required(map, "access", "", fails).and_then(|v| as_object(v, "access", fails))
    else {
        return;
    };
    reject_unknown_keys(
        access,
        &["method", "session_required", "captcha", "notes"],
        "access",
        fails,
    );
    if let Some(method) = required(access, "method", "access", fails) {
        nonempty_str(method, "access.method", fails);
    }
    if let Some(flag) = required(access, "session_required", "access", fails)
        && !flag.is_boolean()
    {
        fails.push("access.session_required: must be true or false".to_owned());
    }
    for key in ["captcha", "notes"] {
        if let Some(v) = access.get(key)
            && !v.is_string()
        {
            fails.push(format!("access.{key}: must be a string"));
        }
    }
}

fn check_identifiers(map: &Map<String, Value>, fails: &mut Vec<String>) {
    let Some(ids) = required(map, "identifiers_available", "", fails)
        .and_then(|v| as_object(v, "identifiers_available", fails))
    else {
        return;
    };
    reject_unknown_keys(
        ids,
        &["politician", "instrument"],
        "identifiers_available",
        fails,
    );
    for key in ["politician", "instrument"] {
        if let Some(v) = required(ids, key, "identifiers_available", fails) {
            nonempty_str(v, &join_at("identifiers_available", key), fails);
        }
    }
}

fn check_open_questions(map: &Map<String, Value>, fails: &mut Vec<String>) {
    let Some(questions) = required(map, "open_questions", "", fails)
        .and_then(|v| as_array(v, "open_questions", fails))
    else {
        return;
    };
    for (i, item) in questions.iter().enumerate() {
        let at = format!("open_questions[{i}]");
        let Some(obj) = as_object(item, &at, fails) else {
            continue;
        };
        reject_unknown_keys(obj, &["question", "tried"], &at, fails);
        if let Some(q) = required(obj, "question", &at, fails) {
            nonempty_str(q, &join_at(&at, "question"), fails);
        }
        if required(obj, "tried", &at, fails).is_some() && !has_tried_log(obj) {
            fails.push(format!(
                "{at}.tried: the what-was-tried log must be a non-empty list (design §5.8)"
            ));
        }
    }
}

fn check_regime_versions(
    map: &Map<String, Value>,
    evidence_dir: &Path,
    jurisdiction: &str,
    fails: &mut Vec<String>,
) {
    let Some(versions) = required(map, "regime_versions", "", fails)
        .and_then(|v| as_array(v, "regime_versions", fails))
    else {
        return;
    };
    for (i, item) in versions.iter().enumerate() {
        let at = format!("regime_versions[{i}]");
        let Some(obj) = as_object(item, &at, fails) else {
            continue;
        };
        reject_unknown_keys(obj, &["effective_from", "change", "evidence"], &at, fails);
        for key in ["effective_from", "change"] {
            if let Some(v) = required(obj, key, &at, fails) {
                nonempty_str(v, &join_at(&at, key), fails);
            }
        }
        let evidence_at = join_at(&at, "evidence");
        if let Some(evidence) =
            required(obj, "evidence", &at, fails).and_then(|v| as_array(v, &evidence_at, fails))
        {
            if evidence.is_empty() {
                fails.push(format!(
                    "{evidence_at}: every regime-version claim carries evidence (fail closed)"
                ));
            }
            for (k, entry) in evidence.iter().enumerate() {
                check_evidence_ref(
                    entry,
                    &format!("{evidence_at}[{k}]"),
                    evidence_dir,
                    jurisdiction,
                    fails,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared building blocks
// ---------------------------------------------------------------------------

/// The artifact's `jurisdiction` field must match the one being validated.
fn check_self_identification(
    map: &Map<String, Value>,
    jurisdiction: &str,
    fails: &mut Vec<String>,
) {
    if let Some(claimed) = required(map, "jurisdiction", "", fails)
        .and_then(|v| nonempty_str(v, "jurisdiction", fails))
        && claimed != jurisdiction
    {
        fails.push(format!(
            "jurisdiction: artifact says {claimed:?}, expected {jurisdiction:?}"
        ));
    }
}

/// Required key holding a non-empty list of non-empty strings.
fn required_str_list(map: &Map<String, Value>, key: &str, fails: &mut Vec<String>) {
    let Some(items) = required(map, key, "", fails).and_then(|v| as_array(v, key, fails)) else {
        return;
    };
    if items.is_empty() {
        fails.push(format!("{key}: must be a non-empty list"));
    }
    for (i, item) in items.iter().enumerate() {
        nonempty_str(item, &format!("{key}[{i}]"), fails);
    }
}

/// Evidence file names must be bare names resolving inside
/// `docs/regimes/<x>/evidence/`, and the file must exist.
fn check_evidence_file(
    name: &str,
    evidence_dir: &Path,
    jurisdiction: &str,
    at: &str,
    fails: &mut Vec<String>,
) {
    if name.contains('/') || name.contains('\\') || name == "." || name == ".." {
        fails.push(format!(
            "{at}: evidence ref {name:?} must be a bare file name inside docs/regimes/{jurisdiction}/evidence/"
        ));
        return;
    }
    if !evidence_dir.join(name).is_file() {
        fails.push(format!(
            "{at}: evidence file not found at docs/regimes/{jurisdiction}/evidence/{name} (fail closed)"
        ));
    }
}

/// The YAML front-matter between the leading `---` line and the next `---`
/// line, or `None` when the document has no front-matter block.
pub(crate) fn front_matter(text: &str) -> Option<&str> {
    let rest = text.strip_prefix("---")?;
    let rest = rest
        .strip_prefix("\r\n")
        .or_else(|| rest.strip_prefix('\n'))?;
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == "---" {
            return Some(&rest[..offset]);
        }
        offset += line.len();
    }
    None
}

/// Exactly one `input.*` file inside a fixture case directory.
pub(crate) fn single_input(case_dir: &Path) -> Result<PathBuf, String> {
    let entries = fs::read_dir(case_dir)
        .map_err(|e| format!("unreadable fixture directory {}: {e}", case_dir.display()))?;
    let mut inputs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.file_stem().is_some_and(|stem| stem == "input") {
            inputs.push(path);
        }
    }
    match inputs.as_slice() {
        [one] => Ok(one.clone()),
        other => Err(format!(
            "expected exactly one input.* file, found {}",
            other.len()
        )),
    }
}

fn parse_yaml(text: &str, label: &str, fails: &mut Vec<String>) -> Option<Value> {
    match serde_norway::from_str::<Value>(text) {
        Ok(doc) => Some(doc),
        Err(e) => {
            fails.push(format!("{label}: not parseable YAML: {e}"));
            None
        }
    }
}

fn as_object<'v>(
    value: &'v Value,
    at: &str,
    fails: &mut Vec<String>,
) -> Option<&'v Map<String, Value>> {
    let obj = value.as_object();
    if obj.is_none() {
        fails.push(format!("{at}: must be a mapping"));
    }
    obj
}

fn as_array<'v>(value: &'v Value, at: &str, fails: &mut Vec<String>) -> Option<&'v Vec<Value>> {
    let arr = value.as_array();
    if arr.is_none() {
        fails.push(format!("{at}: must be a list"));
    }
    arr
}

/// Fetches a required key, recording a failure when absent.
fn required<'v>(
    map: &'v Map<String, Value>,
    key: &str,
    at: &str,
    fails: &mut Vec<String>,
) -> Option<&'v Value> {
    let value = map.get(key);
    if value.is_none() {
        fails.push(format!(
            "{}: missing required key (fail closed)",
            join_at(at, key)
        ));
    }
    value
}

fn nonempty_str<'v>(value: &'v Value, at: &str, fails: &mut Vec<String>) -> Option<&'v str> {
    match value.as_str() {
        Some(s) if !s.trim().is_empty() => Some(s),
        Some(_) => {
            fails.push(format!("{at}: must be a non-empty string"));
            None
        }
        None => {
            fails.push(format!("{at}: must be a string"));
            None
        }
    }
}

fn http_url(value: &Value, at: &str, fails: &mut Vec<String>) {
    if let Some(s) = nonempty_str(value, at, fails)
        && !(s.starts_with("https://") || s.starts_with("http://"))
    {
        fails.push(format!("{at}: must be an http(s) URL, got {s:?}"));
    }
}

/// Anything outside the documented schema rejects (fail closed).
fn reject_unknown_keys(
    map: &Map<String, Value>,
    allowed: &[&str],
    label: &str,
    fails: &mut Vec<String>,
) {
    for key in map.keys() {
        if !allowed.contains(&key.as_str()) {
            fails.push(format!(
                "{label}: unexpected key `{key}` — outside the documented schema \
                 (fail closed; see agents/workflows/source-exploration.md)"
            ));
        }
    }
}

fn join_at(at: &str, key: &str) -> String {
    if at.is_empty() {
        key.to_owned()
    } else {
        format!("{at}.{key}")
    }
}

/// Digest bytes → 64 lowercase hex chars.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

/// `docs/regimes/<x>` under the workspace root.
fn regime_dir(root: &Path, jurisdiction: &str) -> PathBuf {
    root.join("docs").join("regimes").join(jurisdiction)
}

/// Which phase gate a `validate-*` bin runs.
#[derive(Clone, Copy, Debug)]
pub enum Gate {
    /// Scout: `docs/regimes/<x>/sources.yaml`.
    Sources,
    /// Surveyor: `docs/regimes/<x>/AUTHORITY.md` front-matter.
    Survey,
    /// Sampler: `crates/adapters/<x>/fixtures/` + capture manifest.
    Manifest,
}

impl Gate {
    fn bin_name(self) -> &'static str {
        match self {
            Self::Sources => "validate-sources",
            Self::Survey => "validate-survey",
            Self::Manifest => "validate-manifest",
        }
    }

    fn artifact(self) -> &'static str {
        match self {
            Self::Sources => "sources.yaml (Scout artifact)",
            Self::Survey => "AUTHORITY.md front-matter (Surveyor artifact)",
            Self::Manifest => "fixture capture manifest (Sampler artifact)",
        }
    }

    fn validate(self, root: &Path, jurisdiction: &str) -> Vec<String> {
        match self {
            Self::Sources => validate_sources(root, jurisdiction),
            Self::Survey => validate_survey(root, jurisdiction),
            Self::Manifest => validate_manifest(root, jurisdiction),
        }
    }
}

/// Shared driver for the three `validate-*` bins: parses the `<jurisdiction>`
/// argument, runs the gate against the source tree, prints human-readable
/// problems, exits nonzero on anything invalid (fail closed).
#[must_use]
pub fn bin_main(gate: Gate) -> std::process::ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(jurisdiction), None) = (args.next(), args.next()) else {
        eprintln!(
            "usage: cargo run -p pipeline --bin {} -- <jurisdiction>",
            gate.bin_name()
        );
        return std::process::ExitCode::FAILURE;
    };
    if jurisdiction.is_empty()
        || !jurisdiction
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        eprintln!(
            "{}: jurisdiction must be [a-z0-9_-]+, got {jurisdiction:?}",
            gate.bin_name()
        );
        return std::process::ExitCode::FAILURE;
    }
    let root = crate::conformance::workspace_root();
    let failures = gate.validate(&root, &jurisdiction);
    if failures.is_empty() {
        println!("OK: {} for {jurisdiction} validates", gate.artifact());
        std::process::ExitCode::SUCCESS
    } else {
        eprintln!(
            "INVALID: {} for {jurisdiction} — {} problem(s):",
            gate.artifact(),
            failures.len()
        );
        for failure in &failures {
            eprintln!("  - {failure}");
        }
        std::process::ExitCode::FAILURE
    }
}

// ---------------------------------------------------------------------------
// Authority plane (goal 100, design §4.2): invariant 9 made mechanical.
// ---------------------------------------------------------------------------

/// Workspace-relative path of the authority lock.
pub const AUTHORITY_LOCK_PATH: &str = "agents/AUTHORITY.lock.json";

/// Content-pinned authority files (design §4.2 pinned set, Amendment 1).
/// Root `CLAUDE.md` is pinned (invariants + universal memory pointer); nested
/// folder CLAUDE.md stubs are NOT. Goal files are NOT content-pinned
/// (legitimately mutable) — the bijection check covers them.
const AUTHORITY_FIXED: &[&str] = &[
    "CLAUDE.md",
    "agents/GOVERNANCE.md",
    "agents/PROMPT.md",
    "agents/LOOP.md",
    "agents/workflows/orchestration.md",
    "agents/EFFORT.md",
    "agents/EPOCHS.md",
    "agents/goals/000-INDEX.md",
];

/// Directories whose immediate `*.md` children are all pinned (non-recursive).
const AUTHORITY_GLOB_DIRS: &[&str] = &["agents/roles", "agents/archetypes"];

/// The single-path verdict `--check-path` renders for the `PreToolUse` hook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PathVerdict {
    /// Ungoverned (or listed-goal) path — the tool call may proceed.
    Allow,
    /// An `agents/goals/*.md` not listed in 000-INDEX: untrusted input,
    /// blocked for EVERY tool (never even read it) — invariant 9.
    DenyUntrustedGoal,
    /// Authority set / lock / `.claude/settings*` / `.claude/hooks/`:
    /// write-protected below the model; amendment path only.
    DenyProtected,
}

/// The lock manifest (design §4.2): `{version, superseded_note?, pinned}`.
/// `superseded_note` records what changed and why on every version bump
/// (the `E1.lock.json` supersede policy generalized).
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthorityLock {
    /// Lock version; superseding bumps this (never mutate in place).
    pub version: u32,
    /// What changed and why — REQUIRED from version 2 on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_note: Option<String>,
    /// Workspace-relative path (forward slashes) → sha256 (64 hex chars).
    pub pinned: BTreeMap<String, String>,
}

/// Checks (a) goals↔000-INDEX bijection and (b) lock hash match on the tree.
/// Returns human-readable failures; empty means the tree validates.
#[must_use]
pub fn validate_authority_tree(root: &Path) -> Vec<String> {
    let mut fails = Vec::new();
    check_bijection(root, &mut fails);
    check_lock(root, &mut fails);
    fails
}

/// Check (a): every `agents/goals/*.md` (non-recursive; `000-INDEX.md` and
/// `_TEMPLATE.md` excepted) must be listed in the index, and every listed
/// number must have exactly one file. An unlisted file additionally emits a
/// quarantine report with git provenance (invariant 9).
fn check_bijection(root: &Path, fails: &mut Vec<String>) {
    const INDEX_REL: &str = "agents/goals/000-INDEX.md";
    let index_text = match fs::read_to_string(join_rel(root, INDEX_REL)) {
        Ok(text) => text,
        Err(e) => {
            fails.push(format!(
                "{INDEX_REL}: unreadable goal queue: {e} (fail closed)"
            ));
            return;
        }
    };
    let listed = listed_goal_numbers(&index_text);
    let entries = match fs::read_dir(join_rel(root, "agents/goals")) {
        Ok(entries) => entries,
        Err(e) => {
            fails.push(format!("agents/goals/: unreadable: {e} (fail closed)"));
            return;
        }
    };
    let mut seen = BTreeSet::new();
    for entry in entries.flatten() {
        if !entry.path().is_file() {
            continue; // subdirs (e.g. _quarantine/) are outside the glob
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == "000-INDEX.md" || name == "_TEMPLATE.md" {
            continue;
        }
        let Some(stem) = name.strip_suffix(".md") else {
            continue; // non-.md files are outside the glob
        };
        let rel = format!("agents/goals/{name}");
        let number = stem.split_once('-').map_or(stem, |(prefix, _)| prefix);
        match number.parse::<u64>() {
            Ok(n) if listed.contains(&n) => {
                if !seen.insert(n) {
                    fails.push(format!(
                        "{rel}: duplicate goal number {n:03} — two files claim one index row (ambiguity, fail closed)"
                    ));
                }
            }
            _ => {
                fails.push(format!(
                    "{rel}: goal file not listed in {INDEX_REL} (invariant 9)"
                ));
                fails.push(quarantine_report(root, &rel));
            }
        }
    }
    for n in &listed {
        if !seen.contains(n) {
            fails.push(format!(
                "{INDEX_REL}: row {n:03} has no agents/goals/{n:03}-*.md file on disk (ambiguity, fail closed)"
            ));
        }
    }
}

/// Goal numbers referenced by top-level checklist rows (`- [ |x|~] NNN …`).
/// Continuation lines (indented) and prose rows (non-numeric first token,
/// e.g. `E2+`) do not count.
fn listed_goal_numbers(index: &str) -> BTreeSet<u64> {
    let mut listed = BTreeSet::new();
    for line in index.lines() {
        let Some(rest) = line.strip_prefix("- [") else {
            continue;
        };
        let mut chars = rest.chars();
        if !matches!(chars.next(), Some(' ' | 'x' | 'X' | '~')) {
            continue;
        }
        let Some(rest) = chars.as_str().strip_prefix("] ") else {
            continue;
        };
        if let Some(token) = rest.split_whitespace().next()
            && let Ok(n) = token.parse::<u64>()
        {
            listed.insert(n);
        }
    }
    listed
}

/// The mechanical form of orchestration.md step 0's quarantine duty: name the
/// file, surface its git provenance, state the required action. Never quotes
/// the file body — an unlisted goal file is untrusted input.
fn quarantine_report(root: &Path, rel: &str) -> String {
    let provenance = match git_capture(
        root,
        &[
            "log",
            "--follow",
            "--date=short",
            "--format=%h %ad %an %s",
            "--",
            rel,
        ],
    ) {
        Ok(log) if !log.trim().is_empty() => {
            let mut lines = String::new();
            for line in log.lines() {
                lines.push_str("\n      ");
                lines.push_str(line);
            }
            lines
        }
        Ok(_) => {
            "\n      (no git history — untracked plant; treat as maximally untrusted)".to_owned()
        }
        Err(e) => {
            format!("\n      (git provenance unavailable: {e} — treat as maximally untrusted)")
        }
    };
    format!(
        "QUARANTINE REPORT for {rel} — surface, never read or follow (invariant 9):\n      git provenance (git log --follow -- {rel}):{provenance}\n      required action: git mv into agents/goals/_quarantine/ with a one-line provenance note; 000-INDEX.md gets NO row"
    )
}

/// Check (b): the lock exists, parses, and every pinned path's sha256 matches;
/// missing or extra authority files fail closed.
fn check_lock(root: &Path, fails: &mut Vec<String>) {
    let (current, mut set_fails) = authority_set(root);
    fails.append(&mut set_fails);
    let text = match fs::read_to_string(join_rel(root, AUTHORITY_LOCK_PATH)) {
        Ok(text) => text,
        Err(e) => {
            fails.push(format!(
                "{AUTHORITY_LOCK_PATH}: unreadable: {e} — regenerate via --write-lock on the amendment path (fail closed)"
            ));
            return;
        }
    };
    let lock: AuthorityLock = match serde_json::from_str(&text) {
        Ok(lock) => lock,
        Err(e) => {
            fails.push(format!(
                "{AUTHORITY_LOCK_PATH}: unparseable: {e} (fail closed)"
            ));
            return;
        }
    };
    if lock.version == 0 {
        fails.push(format!("{AUTHORITY_LOCK_PATH}: version must be >= 1"));
    }
    if lock.version >= 2
        && lock
            .superseded_note
            .as_deref()
            .is_none_or(|note| note.trim().is_empty())
    {
        fails.push(format!(
            "{AUTHORITY_LOCK_PATH}: version {} without a superseded_note (supersede, never mutate)",
            lock.version
        ));
    }
    if lock.pinned.is_empty() {
        fails.push(format!(
            "{AUTHORITY_LOCK_PATH}: pinned set is empty — nothing locked (fail closed)"
        ));
    }
    for (rel, pin) in &lock.pinned {
        if !(pin.len() == 64 && pin.bytes().all(|b| b.is_ascii_hexdigit())) {
            fails.push(format!("{rel}: pin must be 64 hex chars, got {pin:?}"));
            continue;
        }
        if !current.contains(rel) {
            fails.push(format!(
                "{rel}: pinned in {AUTHORITY_LOCK_PATH} but not in the current authority set (missing or non-authority file) — supersede the lock on the amendment path"
            ));
            continue;
        }
        match fs::read(join_rel(root, rel)) {
            Ok(bytes) => {
                let actual = sha256_hex(&bytes);
                if !actual.eq_ignore_ascii_case(pin) {
                    fails.push(format!(
                        "{rel}: pinned at {pin} but now hashes to {actual} — authority file drifted; amend via an authority/* branch + --write-lock, referencing an INDEX-listed goal"
                    ));
                }
            }
            Err(e) => fails.push(format!("{rel}: pinned file unreadable: {e} (fail closed)")),
        }
    }
    for rel in &current {
        if !lock.pinned.contains_key(rel) {
            fails.push(format!(
                "{rel}: authority file on disk is not pinned in {AUTHORITY_LOCK_PATH} — supersede the lock on the amendment path (fail closed)"
            ));
        }
    }
}

/// Enumerates the authority set on disk: the fixed paths (missing → failure)
/// plus the immediate `*.md` children of the glob directories.
fn authority_set(root: &Path) -> (BTreeSet<String>, Vec<String>) {
    let mut set = BTreeSet::new();
    let mut fails = Vec::new();
    for rel in AUTHORITY_FIXED {
        if join_rel(root, rel).is_file() {
            set.insert((*rel).to_owned());
        } else {
            fails.push(format!(
                "{rel}: authority file missing on disk (fail closed)"
            ));
        }
    }
    for dir_rel in AUTHORITY_GLOB_DIRS {
        match fs::read_dir(join_rel(root, dir_rel)) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if entry.path().is_file() && name.strip_suffix(".md").is_some() {
                        set.insert(format!("{dir_rel}/{name}"));
                    }
                }
            }
            Err(e) => fails.push(format!(
                "{dir_rel}/: unreadable authority directory: {e} (fail closed)"
            )),
        }
    }
    (set, fails)
}

/// Check (c), `--ci` mode: a HEAD whose diff touches authority files must
/// update the lock in the same commit AND reference an INDEX-listed goal
/// (evaluated against HEAD's own tree). Scope choice per design §4.2: HEAD
/// only; for a merge HEAD the diff is vs the first parent and messages are
/// harvested from the merge plus all merged-side commits.
#[must_use]
pub fn check_amendment_discipline(root: &Path) -> Vec<String> {
    let mut fails = Vec::new();
    let parents_line = match git_capture(root, &["rev-list", "--parents", "-n", "1", "HEAD"]) {
        Ok(line) => line,
        Err(e) => {
            fails.push(format!(
                "amendment discipline: cannot read HEAD ({e}) — fail closed"
            ));
            return fails;
        }
    };
    let parent_count = parents_line.split_whitespace().count().saturating_sub(1);
    let (changed, messages) = match parent_count {
        0 => (
            git_capture(root, &["show", "--format=", "--name-only", "HEAD"]),
            git_capture(root, &["log", "-1", "--format=%B", "HEAD"]),
        ),
        1 => (
            git_capture(
                root,
                &["diff-tree", "--no-commit-id", "--name-only", "-r", "HEAD"],
            ),
            git_capture(root, &["log", "-1", "--format=%B", "HEAD"]),
        ),
        _ => (
            git_capture(root, &["diff", "--name-only", "HEAD^1", "HEAD"]),
            git_capture(root, &["log", "--format=%B", "HEAD^1..HEAD"]),
        ),
    };
    let changed = match changed {
        Ok(changed) => changed,
        Err(e) => {
            fails.push(format!(
                "amendment discipline: cannot diff HEAD ({e}) — fail closed (shallow clone? fetch full history)"
            ));
            return fails;
        }
    };
    let messages = match messages {
        Ok(messages) => messages,
        Err(e) => {
            fails.push(format!(
                "amendment discipline: cannot read commit messages ({e}) — fail closed"
            ));
            return fails;
        }
    };
    let touched: Vec<&str> = changed
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && is_authority_path(line))
        .collect();
    if touched.is_empty() {
        return fails;
    }
    if !touched.contains(&AUTHORITY_LOCK_PATH) {
        fails.push(format!(
            "amendment discipline: HEAD touches authority file(s) [{}] without updating {AUTHORITY_LOCK_PATH} in the same commit (design §4.2 check (c))",
            touched.join(", ")
        ));
    } else if parent_count > 0 {
        fails.extend(check_lock_supersession(root));
    }
    let listed = match git_capture(root, &["show", "HEAD:agents/goals/000-INDEX.md"]) {
        Ok(index) => listed_goal_numbers(&index),
        Err(e) => {
            fails.push(format!(
                "amendment discipline: cannot read 000-INDEX.md at HEAD ({e}) — fail closed"
            ));
            return fails;
        }
    };
    let refs = goal_refs_in(&messages);
    if !refs.iter().any(|n| listed.contains(n)) {
        fails.push(format!(
            "amendment discipline: HEAD touches authority file(s) [{}] but no commit message in scope references an INDEX-listed goal (write e.g. \"(goal NNN)\") (design §4.2 check (c))",
            touched.join(", ")
        ));
    }
    fails
}

/// True for members of the pinned authority surface (fixed set, glob
/// children by NAME — deleted files still match — and the lock itself).
fn is_authority_path(rel: &str) -> bool {
    if rel.eq_ignore_ascii_case(AUTHORITY_LOCK_PATH)
        || AUTHORITY_FIXED
            .iter()
            .any(|fixed| rel.eq_ignore_ascii_case(fixed))
    {
        return true;
    }
    AUTHORITY_GLOB_DIRS.iter().any(|dir| {
        strip_prefix_ascii_ci(rel, dir)
            .and_then(|rest| rest.strip_prefix('/'))
            .is_some_and(|name| !name.contains('/') && has_md_suffix(name))
    })
}

fn has_md_suffix(name: &str) -> bool {
    name.get(name.len().saturating_sub(3)..)
        .is_some_and(|suffix| suffix.eq_ignore_ascii_case(".md"))
}

/// A committed lock update is a supersession, never an in-place rewrite.
/// The first lock has no first-parent predecessor; every later lock must raise
/// the version and carry a new, non-empty explanation.
fn check_lock_supersession(root: &Path) -> Vec<String> {
    let mut fails = Vec::new();
    let old_text = match git_file_at(root, "HEAD^1", AUTHORITY_LOCK_PATH) {
        Ok(Some(text)) => text,
        Ok(None) => return fails,
        Err(e) => {
            fails.push(format!(
                "amendment discipline: cannot read first-parent {AUTHORITY_LOCK_PATH} ({e}) — fail closed"
            ));
            return fails;
        }
    };
    let new_text = match git_file_at(root, "HEAD", AUTHORITY_LOCK_PATH) {
        Ok(Some(text)) => text,
        Ok(None) => {
            fails.push(format!(
                "amendment discipline: {AUTHORITY_LOCK_PATH} is absent at HEAD — fail closed"
            ));
            return fails;
        }
        Err(e) => {
            fails.push(format!(
                "amendment discipline: cannot read HEAD {AUTHORITY_LOCK_PATH} ({e}) — fail closed"
            ));
            return fails;
        }
    };
    let old: AuthorityLock = match serde_json::from_str(&old_text) {
        Ok(lock) => lock,
        Err(e) => {
            fails.push(format!(
                "amendment discipline: first-parent {AUTHORITY_LOCK_PATH} is unparseable ({e}) — fail closed"
            ));
            return fails;
        }
    };
    let new: AuthorityLock = match serde_json::from_str(&new_text) {
        Ok(lock) => lock,
        Err(e) => {
            fails.push(format!(
                "amendment discipline: HEAD {AUTHORITY_LOCK_PATH} is unparseable ({e}) — fail closed"
            ));
            return fails;
        }
    };
    if new.version <= old.version {
        fails.push(format!(
            "amendment discipline: {AUTHORITY_LOCK_PATH} must genuinely supersede first-parent version {} with a higher version, got {}",
            old.version, new.version
        ));
    }
    let new_note = new.superseded_note.as_deref().map(str::trim);
    if new_note.is_none_or(str::is_empty) {
        fails.push(format!(
            "amendment discipline: superseding {AUTHORITY_LOCK_PATH} requires a non-empty superseded_note"
        ));
    } else if old.superseded_note.as_deref().map(str::trim) == new_note {
        fails.push(format!(
            "amendment discipline: superseding {AUTHORITY_LOCK_PATH} requires a changed superseded_note"
        ));
    }
    fails
}

/// Goal numbers referenced in commit-message text: `goal <n>` (any of
/// ` /#-_:` between the word and the digits) and the `authority/<n>` branch
/// form that merge messages carry.
fn goal_refs_in(text: &str) -> BTreeSet<u64> {
    let lower = text.to_lowercase();
    let bytes = lower.as_bytes();
    let mut refs = BTreeSet::new();
    for pat in ["goal", "authority/"] {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(pat) {
            let mut i = from + pos + pat.len();
            while i < bytes.len()
                && matches!(bytes[i], b' ' | b'\t' | b'/' | b'#' | b'-' | b'_' | b':')
            {
                i += 1;
            }
            let digits = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i > digits
                && let Ok(n) = lower[digits..i].parse::<u64>()
            {
                refs.insert(n);
            }
            from = from + pos + pat.len();
        }
    }
    refs
}

/// Fast single-path mode for the hook (no repo walk; reads at most the goal
/// index). Unlisted `agents/goals/*.md` → untrusted (deny every tool);
/// authority set + lock + `.claude/settings*` + `.claude/hooks/` → protected
/// (the hook allows reads and amendment-branch writes itself). Paths outside
/// the repo are ungoverned.
#[must_use]
pub fn check_path(root: &Path, raw: &str) -> PathVerdict {
    let Some(rel) = normalize_rel(root, raw) else {
        return PathVerdict::Allow;
    };
    if let Some(name) = strip_prefix_ascii_ci(&rel, "agents/goals/")
        && !name.contains('/')
    {
        if name.eq_ignore_ascii_case("000-INDEX.md") {
            return PathVerdict::DenyProtected;
        }
        if name.eq_ignore_ascii_case("_TEMPLATE.md") {
            return PathVerdict::Allow;
        }
        if has_md_suffix(name) {
            let stem = &name[..name.len() - 3];
            let number = stem.split_once('-').map_or(stem, |(prefix, _)| prefix);
            let listed = fs::read_to_string(join_rel(root, "agents/goals/000-INDEX.md"))
                .map(|text| listed_goal_numbers(&text));
            return match (number.parse::<u64>(), listed) {
                (Ok(n), Ok(listed)) if listed.contains(&n) => PathVerdict::Allow,
                // Unlisted, non-numeric, or unreadable index: untrusted
                // input — fail closed (invariant 9).
                _ => PathVerdict::DenyUntrustedGoal,
            };
        }
        return PathVerdict::Allow; // non-.md under agents/goals/ is outside the glob
    }
    if is_authority_path(&rel) {
        return PathVerdict::DenyProtected;
    }
    if strip_prefix_ascii_ci(&rel, ".claude/").is_some_and(|rest| {
        strip_prefix_ascii_ci(rest, "hooks/").is_some()
            || strip_prefix_ascii_ci(rest, "settings").is_some()
    }) {
        return PathVerdict::DenyProtected;
    }
    PathVerdict::Allow
}

/// Normalizes a raw tool path to a forward-slash repo-relative path:
/// backslashes → slashes, `.`/`..` resolved textually, absolute paths
/// stripped of the repo root (ASCII-case-insensitively — Windows). `None`
/// means the path is outside the repo (ungoverned).
fn normalize_rel(root: &Path, raw: &str) -> Option<String> {
    let slashes = raw.replace('\\', "/");
    let root_str = root.to_string_lossy().replace('\\', "/");
    let trimmed_root = root_str.trim_end_matches('/');
    let is_abs = slashes.starts_with('/') || slashes.get(1..2) == Some(":");
    let rel_raw = if is_abs {
        match strip_prefix_ascii_ci(&slashes, trimmed_root) {
            Some(rest) if rest.starts_with('/') => rest[1..].to_owned(),
            _ => return None,
        }
    } else {
        slashes
    };
    let mut segments: Vec<&str> = Vec::new();
    for seg in rel_raw.split('/') {
        match seg {
            "" | "." => {}
            // Popping past the top escapes the repo → ungoverned.
            ".." => {
                segments.pop()?;
            }
            other => segments.push(other),
        }
    }
    if segments.is_empty() {
        return None;
    }
    Some(segments.join("/"))
}

/// `strip_prefix`, ASCII-case-insensitive (Windows paths).
fn strip_prefix_ascii_ci<'s>(s: &'s str, prefix: &str) -> Option<&'s str> {
    if s.len() < prefix.len() || !s.is_char_boundary(prefix.len()) {
        return None;
    }
    let (head, tail) = s.split_at(prefix.len());
    head.eq_ignore_ascii_case(prefix).then_some(tail)
}

/// Regenerates the lock over the current authority set (`--write-lock`).
/// First lock is version 1; superseding an existing lock bumps the version
/// and REQUIRES a `--note` saying what changed and why (supersede, never
/// mutate — the `E1.lock.json` policy).
///
/// # Errors
/// Missing authority files, unreadable tree, unparseable existing lock, or a
/// supersession without a note (fail closed).
pub fn write_authority_lock(root: &Path, note: Option<&str>) -> anyhow::Result<u32> {
    use anyhow::Context as _;
    let (set, fails) = authority_set(root);
    if !fails.is_empty() {
        anyhow::bail!("authority set incomplete:\n  - {}", fails.join("\n  - "));
    }
    let lock_file = join_rel(root, AUTHORITY_LOCK_PATH);
    let version = if lock_file.is_file() {
        let text = fs::read_to_string(&lock_file)
            .with_context(|| format!("reading existing {AUTHORITY_LOCK_PATH}"))?;
        let old: AuthorityLock = serde_json::from_str(&text)
            .with_context(|| format!("parsing existing {AUTHORITY_LOCK_PATH}"))?;
        if note.is_none_or(|n| n.trim().is_empty()) {
            anyhow::bail!(
                "superseding {AUTHORITY_LOCK_PATH} v{} requires --note <what changed and why> (supersede, never mutate)",
                old.version
            );
        }
        old.version + 1
    } else {
        1
    };
    let mut pinned = BTreeMap::new();
    for rel in &set {
        let bytes = fs::read(join_rel(root, rel)).with_context(|| format!("reading {rel}"))?;
        pinned.insert(rel.clone(), sha256_hex(&bytes));
    }
    let lock = AuthorityLock {
        version,
        superseded_note: note.map(str::to_owned),
        pinned,
    };
    let mut text = serde_json::to_string_pretty(&lock).context("serializing the authority lock")?;
    text.push('\n');
    fs::write(&lock_file, text).with_context(|| format!("writing {AUTHORITY_LOCK_PATH}"))?;
    Ok(version)
}

/// Runs one read-only git subcommand against the checkout; any spawn
/// failure, non-zero exit, or non-UTF-8 output is an `Err` (fail closed).
fn git_capture(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git not runnable: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git {} exited {}: {}",
            args.join(" "),
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Reads one file from a git tree. A path absent from an otherwise-readable
/// tree is `Ok(None)`; repository/history ambiguity remains an error.
fn git_file_at(root: &Path, revision: &str, rel: &str) -> Result<Option<String>, String> {
    let listed = git_capture(root, &["ls-tree", "--name-only", revision, "--", rel])?;
    if listed.lines().any(|line| line.trim() == rel) {
        let spec = format!("{revision}:{rel}");
        git_capture(root, &["show", &spec]).map(Some)
    } else {
        Ok(None)
    }
}

/// Joins a forward-slash workspace-relative path under `root` (Windows-safe).
fn join_rel(root: &Path, rel: &str) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in rel.split('/') {
        path.push(segment);
    }
    path
}

/// Driver for the `validate-authority` bin. Modes:
/// no flags = tree checks (a+b); `--ci` adds amendment discipline (c);
/// `--write-lock [--note <text>]` regenerates the lock;
/// `--check-path <p>` renders the hook verdict (deny → exit 2).
#[must_use]
pub fn authority_bin_main() -> std::process::ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage =
        "usage: validate-authority [--ci | --write-lock [--note <text>] | --check-path <path>]";
    let Some(root) = authority_root() else {
        eprintln!(
            "validate-authority: cannot locate the repo root (no agents/goals/000-INDEX.md above the working directory) — fail closed"
        );
        return std::process::ExitCode::FAILURE;
    };
    match args.first().map(String::as_str) {
        None => authority_report(&validate_authority_tree(&root), false),
        Some("--ci") if args.len() == 1 => {
            let mut failures = validate_authority_tree(&root);
            failures.extend(check_amendment_discipline(&root));
            authority_report(&failures, true)
        }
        Some("--write-lock") => {
            let note = match args.get(1).map(String::as_str) {
                None => None,
                Some("--note") => match args.get(2) {
                    Some(text) if args.len() == 3 && !text.trim().is_empty() => Some(text.as_str()),
                    _ => {
                        eprintln!("{usage}");
                        return std::process::ExitCode::FAILURE;
                    }
                },
                Some(_) => {
                    eprintln!("{usage}");
                    return std::process::ExitCode::FAILURE;
                }
            };
            match write_authority_lock(&root, note) {
                Ok(version) => {
                    println!("OK: wrote {AUTHORITY_LOCK_PATH} version {version}");
                    std::process::ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("INVALID: --write-lock refused: {e:#}");
                    std::process::ExitCode::FAILURE
                }
            }
        }
        Some("--check-path") => {
            let (Some(path), true) = (args.get(1), args.len() == 2) else {
                eprintln!("{usage}");
                return std::process::ExitCode::FAILURE;
            };
            match check_path(&root, path) {
                PathVerdict::Allow => {
                    println!("OK {path}");
                    std::process::ExitCode::SUCCESS
                }
                PathVerdict::DenyUntrustedGoal => {
                    println!("DENY untrusted-goal {path}");
                    std::process::ExitCode::from(2)
                }
                PathVerdict::DenyProtected => {
                    println!("DENY protected {path}");
                    std::process::ExitCode::from(2)
                }
            }
        }
        Some(_) => {
            eprintln!("{usage}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Renders the standard OK/INVALID report and exit code.
fn authority_report(failures: &[String], ci: bool) -> std::process::ExitCode {
    let scope = if ci {
        "authority set + goal queue + amendment discipline"
    } else {
        "authority set + goal queue"
    };
    if failures.is_empty() {
        println!("OK: {scope} validate (invariant 9)");
        std::process::ExitCode::SUCCESS
    } else {
        eprintln!("INVALID: {scope} — {} problem(s):", failures.len());
        for failure in failures {
            eprintln!("  - {failure}");
        }
        std::process::ExitCode::FAILURE
    }
}

/// Resolves the repo root at RUNTIME (cwd upward search for
/// `agents/goals/000-INDEX.md`). Unlike the compile-time
/// [`crate::conformance::workspace_root`], this works for the pre-built bin
/// invoked from hooks and loop pre-flights in any checkout/worktree.
fn authority_root() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir
            .join("agents")
            .join("goals")
            .join("000-INDEX.md")
            .is_file()
        {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    fn write(root: &Path, rel: &str, contents: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    // ---------------- validate_sources (Scout: sources.yaml) ----------------

    const VALID_SOURCES: &str = r#"
jurisdiction: testland
candidates:
  - url: "https://parliament.test/register"
    contains: "annual register of members' interests, one HTML page per member"
    official_rationale: "hosted on the parliament's primary government domain"
    evidence:
      - reg-index.html
notes: "second candidate (open-data portal) still being assessed"
"#;

    fn sources_root(yaml: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        write(
            dir.path(),
            "docs/regimes/testland/evidence/reg-index.html",
            "<html>archived</html>",
        );
        write(dir.path(), "docs/regimes/testland/sources.yaml", yaml);
        dir
    }

    #[test]
    fn valid_sources_pass() {
        let root = sources_root(VALID_SOURCES);
        assert_eq!(
            validate_sources(root.path(), "testland"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn missing_sources_file_rejects() {
        let dir = tempfile::tempdir().unwrap();
        let failures = validate_sources(dir.path(), "testland");
        assert!(
            failures
                .iter()
                .any(|f| f.contains("missing") && f.contains("sources.yaml")),
            "missing artifact must fail closed: {failures:?}"
        );
    }

    #[test]
    fn unparseable_yaml_rejects() {
        let root = sources_root("jurisdiction: [unclosed");
        assert!(!validate_sources(root.path(), "testland").is_empty());
    }

    #[test]
    fn jurisdiction_mismatch_rejects() {
        let root = sources_root(
            &VALID_SOURCES.replace("jurisdiction: testland", "jurisdiction: otherland"),
        );
        let failures = validate_sources(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("jurisdiction")),
            "artifact must self-identify: {failures:?}"
        );
    }

    #[test]
    fn empty_candidates_reject() {
        let root = sources_root("jurisdiction: testland\ncandidates: []\n");
        let failures = validate_sources(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("candidates")),
            ">=1 candidate required: {failures:?}"
        );
    }

    #[test]
    fn candidate_missing_rationale_rejects() {
        let root = sources_root(
            &VALID_SOURCES.replace("    official_rationale:", "    # official_rationale:"),
        );
        let failures = validate_sources(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("official_rationale")),
            "why-official is required: {failures:?}"
        );
    }

    #[test]
    fn non_http_url_rejects() {
        let root = sources_root(&VALID_SOURCES.replace(
            "https://parliament.test/register",
            "ftp://parliament.test/register",
        ));
        let failures = validate_sources(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("url")),
            "candidate url must be http(s): {failures:?}"
        );
    }

    #[test]
    fn absent_evidence_file_rejects() {
        let root = sources_root(&VALID_SOURCES.replace("reg-index.html", "never-archived.html"));
        let failures = validate_sources(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("never-archived.html")),
            "evidence must exist on disk: {failures:?}"
        );
    }

    #[test]
    fn evidence_path_traversal_rejects() {
        let root = sources_root(&VALID_SOURCES.replace("reg-index.html", "../../../etc/passwd"));
        let failures = validate_sources(root.path(), "testland");
        assert!(
            !failures.is_empty(),
            "path traversal must reject: {failures:?}"
        );
    }

    #[test]
    fn unknown_top_level_key_rejects() {
        let root = sources_root(&format!("{VALID_SOURCES}confidence: high\n"));
        let failures = validate_sources(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("confidence")),
            "outside-schema keys fail closed: {failures:?}"
        );
    }

    // ------------- validate_survey (Surveyor: AUTHORITY.md) -------------

    const VALID_SURVEY: &str = r#"---
jurisdiction: testland
bodies: [Parliament]
legal_basis:
  claim: "Financial Disclosure Act 2010, s. 12"
  evidence:
    - {url: "https://gov.test/act", file: act.html}
who_files:
  claim: "all sitting members, within 30 days of a change"
  evidence:
    - {url: "https://gov.test/act", file: act.html}
record_types: [transaction, holding]
value_precision: banded
band_table:
  - {code: "A", low: "1", high: "1000"}
cadence_and_lag:
  claim: "register updated monthly; ~45 day lag observed"
  evidence:
    - {url: "https://gov.test/register", file: register.html}
formats: [html_table]
access:
  method: "public web, no session"
  session_required: false
  captcha: "none observed"
  notes: ""
historical_depth:
  from: "2012"
  evidence:
    - {url: "https://gov.test/archive", file: archive.html}
identifiers_available:
  politician: "stable member id in profile URL"
  instrument: "none"
amendment_mechanism:
  claim: unknown
  evidence: []
  tried:
    - "searched register FAQ for 'amend' / 'correction' (2026-07-04)"
    - "read act ss. 12-19; silent on amendments"
personal_data_to_redact: []
tos_and_politeness:
  claim: "robots.txt allows /register; no ToS restriction found"
  evidence:
    - {url: "https://gov.test/robots.txt", file: robots.txt}
language: [en]
open_questions:
  - question: "how are amendments published?"
    tried: ["register FAQ", "act ss. 12-19"]
regime_versions:
  - effective_from: "2012-01-01"
    change: "initial disclosure regime"
    evidence:
      - {url: "https://gov.test/act", file: act.html}
---
# Testland — Source Authority File

## Data catalog
## Field mapping (source → gold)
## Parse strategy & rationale
## Quirks log (append-only, dated)
## Operational notes (politeness incidents, outages)
"#;

    fn survey_root(authority: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for file in ["act.html", "register.html", "archive.html", "robots.txt"] {
            write(
                dir.path(),
                &format!("docs/regimes/testland/evidence/{file}"),
                "archived",
            );
        }
        write(dir.path(), "docs/regimes/testland/AUTHORITY.md", authority);
        dir
    }

    #[test]
    fn valid_survey_passes() {
        let root = survey_root(VALID_SURVEY);
        assert_eq!(
            validate_survey(root.path(), "testland"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn missing_authority_file_rejects() {
        let dir = tempfile::tempdir().unwrap();
        let failures = validate_survey(dir.path(), "testland");
        assert!(
            failures
                .iter()
                .any(|f| f.contains("missing") && f.contains("AUTHORITY.md")),
            "missing artifact must fail closed: {failures:?}"
        );
    }

    #[test]
    fn missing_front_matter_rejects() {
        let root = survey_root("# Testland\nno front matter here\n");
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("front-matter")),
            "front-matter is the artifact: {failures:?}"
        );
    }

    #[test]
    fn missing_required_field_rejects() {
        // Drop the whole tos_and_politeness block (claim + evidence lines).
        let mut lines: Vec<&str> = VALID_SURVEY.lines().collect();
        let start = lines
            .iter()
            .position(|l| l.starts_with("tos_and_politeness:"))
            .unwrap();
        lines.drain(start..start + 4);
        let root = survey_root(&lines.join("\n"));
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("tos_and_politeness")),
            "every RegimeSurvey field is required: {failures:?}"
        );
    }

    #[test]
    fn unknown_claim_without_tried_log_rejects() {
        let survey = VALID_SURVEY.replace(
            "amendment_mechanism:\n  claim: unknown\n  evidence: []\n  tried:\n    - \"searched register FAQ for 'amend' / 'correction' (2026-07-04)\"\n    - \"read act ss. 12-19; silent on amendments\"",
            "amendment_mechanism:\n  claim: unknown\n  evidence: []",
        );
        assert_ne!(survey, VALID_SURVEY, "replacement must have matched");
        let root = survey_root(&survey);
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures
                .iter()
                .any(|f| f.contains("amendment_mechanism") && f.contains("tried")),
            "unknown is legal ONLY with a tried-log (§5.8): {failures:?}"
        );
    }

    #[test]
    fn claim_without_evidence_rejects() {
        let survey = VALID_SURVEY.replace(
            "who_files:\n  claim: \"all sitting members, within 30 days of a change\"\n  evidence:\n    - {url: \"https://gov.test/act\", file: act.html}",
            "who_files:\n  claim: \"all sitting members, within 30 days of a change\"\n  evidence: []",
        );
        assert_ne!(survey, VALID_SURVEY, "replacement must have matched");
        let root = survey_root(&survey);
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures
                .iter()
                .any(|f| f.contains("who_files") && f.contains("evidence")),
            "evidence or it didn't happen: {failures:?}"
        );
    }

    #[test]
    fn evidence_without_url_rejects() {
        let survey = VALID_SURVEY.replace(
            "    - {url: \"https://gov.test/robots.txt\", file: robots.txt}",
            "    - {file: robots.txt}",
        );
        assert_ne!(survey, VALID_SURVEY, "replacement must have matched");
        let root = survey_root(&survey);
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures
                .iter()
                .any(|f| f.contains("tos_and_politeness") && f.contains("url")),
            "evidence items carry {{url, file}}: {failures:?}"
        );
    }

    #[test]
    fn evidence_file_absent_on_disk_rejects() {
        let survey = VALID_SURVEY.replace("file: archive.html", "file: never-archived.html");
        let root = survey_root(&survey);
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("never-archived.html")),
            "evidence must exist on disk: {failures:?}"
        );
    }

    #[test]
    fn out_of_vocabulary_record_type_rejects() {
        let survey = VALID_SURVEY.replace(
            "record_types: [transaction, holding]",
            "record_types: [transaction, gift]",
        );
        let root = survey_root(&survey);
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures
                .iter()
                .any(|f| f.contains("record_types") && f.contains("gift")),
            "record_types is a closed vocabulary: {failures:?}"
        );
    }

    #[test]
    fn banded_without_band_table_rejects() {
        let survey = VALID_SURVEY.replace(
            "band_table:\n  - {code: \"A\", low: \"1\", high: \"1000\"}",
            "band_table: []",
        );
        assert_ne!(survey, VALID_SURVEY, "replacement must have matched");
        let root = survey_root(&survey);
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("band_table")),
            "banded precision requires the band table: {failures:?}"
        );
    }

    #[test]
    fn open_question_without_tried_rejects() {
        let survey = VALID_SURVEY.replace(
            "    tried: [\"register FAQ\", \"act ss. 12-19\"]",
            "    tried: []",
        );
        assert_ne!(survey, VALID_SURVEY, "replacement must have matched");
        let root = survey_root(&survey);
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures
                .iter()
                .any(|f| f.contains("open_questions") && f.contains("tried")),
            "open questions carry a tried-log: {failures:?}"
        );
    }

    #[test]
    fn missing_body_section_rejects() {
        let survey = VALID_SURVEY.replace("## Quirks log (append-only, dated)\n", "");
        let root = survey_root(&survey);
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("Quirks log")),
            "template sections are required: {failures:?}"
        );
    }

    #[test]
    fn unknown_survey_key_rejects() {
        let survey = VALID_SURVEY.replace("language: [en]", "language: [en]\nvibes: good");
        let root = survey_root(&survey);
        let failures = validate_survey(root.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("vibes")),
            "outside-schema keys fail closed: {failures:?}"
        );
    }

    // ------------- validate_manifest (Sampler: fixtures + manifest) -------------

    /// Writes three fixture cases and returns YAML `cases:` entries with real hashes.
    fn write_cases(root: &Path, names: &[&str]) -> String {
        use std::fmt::Write as _;
        let mut cases = String::from("cases:\n");
        for name in names {
            let contents = format!("<filing case={name}/>");
            write(
                root,
                &format!("crates/adapters/testland/fixtures/{name}/input.xml"),
                &contents,
            );
            let sha = sha256_hex(contents.as_bytes());
            writeln!(
                cases,
                "  {name}:\n    url: \"https://gov.test/filings/{name}.xml\"\n    sha256: \"{sha}\""
            )
            .unwrap();
        }
        cases
    }

    const MANIFEST_HEADER: &str = concat!(
        "captured_at_utc: \"2026-07-04T12:00:00Z\"\n",
        "politeness:\n",
        "  user_agent: \"govfolio.io research (contact: ssm.leo@outlook.com)\"\n",
        "  concurrency: 1\n",
        "  min_interval_seconds: 2\n",
    );

    fn manifest_root(names: &[&str]) -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        let cases = write_cases(dir.path(), names);
        (dir, format!("{MANIFEST_HEADER}{cases}"))
    }

    #[test]
    fn valid_manifest_passes() {
        let (dir, manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &manifest,
        );
        assert_eq!(
            validate_manifest(dir.path(), "testland"),
            Vec::<String>::new()
        );
    }

    #[test]
    fn missing_manifest_rejects() {
        let (dir, _manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("manifest")),
            "missing manifest must fail closed: {failures:?}"
        );
    }

    #[test]
    fn two_manifests_reject_as_ambiguous() {
        let (dir, manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &manifest,
        );
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/MANIFEST.json",
            "{}",
        );
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("ambiguous")),
            "two manifests = ambiguity = halt: {failures:?}"
        );
    }

    #[test]
    fn fewer_than_three_cases_reject() {
        let (dir, manifest) = manifest_root(&["typical", "amendment"]);
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &manifest,
        );
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains('3')),
            ">=3 representative filings required: {failures:?}"
        );
    }

    #[test]
    fn hash_mismatch_rejects() {
        let (dir, manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        let tampered = manifest.replacen(
            &sha256_hex(b"<filing case=typical/>"),
            &sha256_hex(b"something else entirely"),
            1,
        );
        assert_ne!(tampered, manifest, "replacement must have matched");
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &tampered,
        );
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures
                .iter()
                .any(|f| f.contains("typical") && f.contains("sha256")),
            "raw is sacred — hash must match: {failures:?}"
        );
    }

    #[test]
    fn undeclared_fixture_dir_on_disk_rejects() {
        let (dir, manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/stray/input.xml",
            "<x/>",
        );
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &manifest,
        );
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("stray")),
            "every fixture case needs a manifest entry: {failures:?}"
        );
    }

    #[test]
    fn manifest_case_without_fixture_dir_rejects() {
        let (dir, manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        let manifest = format!(
            "{manifest}  ghost:\n    url: \"https://gov.test/filings/ghost.xml\"\n    sha256: \"{}\"\n",
            sha256_hex(b"ghost")
        );
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &manifest,
        );
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("ghost")),
            "manifest entries need fixtures on disk: {failures:?}"
        );
    }

    #[test]
    fn case_dir_without_input_file_rejects() {
        let (dir, manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        fs::remove_file(
            dir.path()
                .join("crates/adapters/testland/fixtures/typical/input.xml"),
        )
        .unwrap();
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/typical/notes.txt",
            "empty",
        );
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &manifest,
        );
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures
                .iter()
                .any(|f| f.contains("typical") && f.contains("input")),
            "exactly one input.* per case: {failures:?}"
        );
    }

    #[test]
    fn missing_user_agent_rejects() {
        let (dir, manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        let manifest = manifest.replace(
            "  user_agent: \"govfolio.io research (contact: ssm.leo@outlook.com)\"\n",
            "",
        );
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &manifest,
        );
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("user_agent")),
            "identified UA is invariant 10: {failures:?}"
        );
    }

    #[test]
    fn bad_timestamp_rejects() {
        let (dir, manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        let manifest = manifest.replace("2026-07-04T12:00:00Z", "yesterday-ish");
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &manifest,
        );
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("captured_at_utc")),
            "capture date must be a real timestamp: {failures:?}"
        );
    }

    #[test]
    fn non_http_case_url_rejects() {
        let (dir, manifest) = manifest_root(&["typical", "amendment", "edge_case"]);
        let manifest = manifest.replace(
            "https://gov.test/filings/typical.xml",
            "file:///tmp/typical.xml",
        );
        write(
            dir.path(),
            "crates/adapters/testland/fixtures/manifest.yaml",
            &manifest,
        );
        let failures = validate_manifest(dir.path(), "testland");
        assert!(
            failures.iter().any(|f| f.contains("url")),
            "capture provenance must be a fetchable http(s) URL: {failures:?}"
        );
    }

    #[test]
    fn us_house_reference_manifest_validates() {
        // The real E1 reference artifact (MANIFEST.json spelling, extra
        // provenance keys) must satisfy the derived schema.
        let root = crate::conformance::workspace_root();
        assert_eq!(validate_manifest(&root, "us_house"), Vec::<String>::new());
    }

    // ------------- validate_authority helpers (goal 100, §4.2) -------------

    #[test]
    fn validate_authority_index_rows_parse_like_the_real_queue() {
        let index = concat!(
            "# Goal queue (ordered)\n\n",
            "- [x] 001 M0-M1 walking skeleton (done)\n",
            "- [~] 021 LLM extraction fallback - RE-OPENED\n",
            "- [x] 081 US backfill real write execution - built\n",
            "  (874 real Gold rows, zero alerts) continuation line with 999 numbers\n",
            "- [ ] 104 lane idle backoff (founder-directed)\n",
            "- [ ] E2+ Brazil onward: NO hand-written goals\n",
            "  - [x] 555 indented pseudo-row must not count\n",
        );
        let listed = listed_goal_numbers(index);
        assert_eq!(
            listed.iter().copied().collect::<Vec<u64>>(),
            vec![1, 21, 81, 104],
            "continuation lines, prose rows, and indented rows never list a goal"
        );
    }

    #[test]
    fn validate_authority_goal_refs_scan_commit_message_shapes() {
        let refs = goal_refs_in(
            "feat(pipeline): authority lock (goal 100)\n\
             Merge branch 'authority/015-amend'\n\
             see goal/104-lane-idle-backoff and Goal-021 notes\n\
             fix 404 handling; renumbered from 0011\n\
             agents/goals/000-INDEX.md row untouched",
        );
        assert_eq!(
            refs.iter().copied().collect::<Vec<u64>>(),
            vec![15, 21, 100, 104],
            "goal/authority forms parse; bare numbers (404, 0011) and \
             agents/goals/ path mentions never count"
        );
    }

    #[test]
    fn validate_authority_normalize_rel_handles_windows_and_traversal() {
        let root = Path::new("C:/repo");
        let n = |raw: &str| normalize_rel(root, raw);
        assert_eq!(
            n("agents/goals/099.md").as_deref(),
            Some("agents/goals/099.md")
        );
        assert_eq!(
            n("agents\\goals\\099.md").as_deref(),
            Some("agents/goals/099.md")
        );
        assert_eq!(
            n("agents/../agents/goals/099.md").as_deref(),
            Some("agents/goals/099.md")
        );
        assert_eq!(
            n("C:/repo/CLAUDE.md").as_deref(),
            Some("CLAUDE.md"),
            "absolute under root strips"
        );
        assert_eq!(
            n("c:\\REPO\\CLAUDE.md").as_deref(),
            Some("CLAUDE.md"),
            "windows paths compare case-insensitively"
        );
        assert_eq!(n("C:/elsewhere/CLAUDE.md"), None, "outside the repo");
        assert_eq!(n("../escape.md"), None, "escaping the repo is ungoverned");
    }
}
