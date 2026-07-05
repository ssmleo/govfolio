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

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use govfolio_core::domain::enums::RecordType;

/// `disclosure_regime.value_precision` CHECK vocabulary (migration 0001).
const VALUE_PRECISION: &[&str] = &["exact", "banded", "categorical", "none"];

/// Every `RegimeSurvey` front-matter key (template:
/// `docs/regimes/_templates/AUTHORITY.template.md`). All required; anything
/// else rejects (fail closed).
const SURVEY_KEYS: &[&str] = &[
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
fn front_matter(text: &str) -> Option<&str> {
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
fn single_input(case_dir: &Path) -> Result<PathBuf, String> {
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
fn sha256_hex(bytes: &[u8]) -> String {
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
}
