//! Seeded-violation acceptance for `validate-authority` (goal 100, design
//! §4.2). Every test spawns the REAL bin against a seeded tempdir tree and
//! asserts on real exit codes: 0 = valid, 1 = invalid (fail closed),
//! 2 = `--check-path` deny. Filter: `cargo test -p pipeline validate_authority`.
//!
//! Tests are hermetic (tempdirs, own git repos where check (c) needs one);
//! the acceptance command `cargo run -p pipeline --bin validate-authority`
//! is the real-tree check and runs at every run point instead of here — a
//! real-tree assertion in the suite would flake on concurrent agents'
//! mid-edit trees (JOURNAL 2026-07-10 `role_evals` lesson).

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

const BIN: &str = env!("CARGO_BIN_EXE_validate-authority");

fn write(root: &Path, rel: &str, contents: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

/// Minimal goal queue: 001 + 015 listed; the E2+ prose row must not parse as
/// a goal number; the indented continuation line must not parse as a row.
const INDEX: &str = "# Goal queue (ordered)\n\n\
- [x] 001 walking skeleton (done 2026-07-04; see goal file\n  + JOURNAL continuation line)\n\
- [~] 015 coverage factory\n\
- [ ] E2+ Brazil onward: NO hand-written goals\n";

fn seed_tree(root: &Path) {
    write(root, "CLAUDE.md", "# root claude\n");
    write(root, "agents/GOVERNANCE.md", "# governance\n");
    write(root, "agents/PROMPT.md", "# prompt\n");
    write(root, "agents/LOOP.md", "# loop\n");
    write(
        root,
        "agents/workflows/orchestration.md",
        "# orchestration\n",
    );
    write(root, "agents/EFFORT.md", "# effort\n");
    write(root, "agents/EPOCHS.md", "# epochs\n");
    write(root, "agents/goals/000-INDEX.md", INDEX);
    write(root, "agents/goals/001-walking-skeleton.md", "# goal 001\n");
    write(root, "agents/goals/015-coverage-factory.md", "# goal 015\n");
    write(root, "agents/goals/_TEMPLATE.md", "# template\n");
    write(
        root,
        "agents/goals/_quarantine/022-adversarial-review-loop.md",
        "# quarantined import proposal\n",
    );
    write(
        root,
        "agents/roles/rust-builder.md",
        "# role: rust-builder\n",
    );
    write(
        root,
        "agents/roles/orchestrator.md",
        "# role: orchestrator\n",
    );
    write(root, "agents/archetypes/_CHASSIS.md", "# chassis\n");
    write(root, "agents/archetypes/doer.md", "# archetype: doer\n");
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .current_dir(root)
        .output()
        .unwrap()
}

fn code(out: &Output) -> i32 {
    out.status.code().unwrap()
}

fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// Seeds a valid tree AND its lock; asserts the lock actually materialized
/// (the write is the feature — a stub that writes nothing must go red here).
fn locked_tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    seed_tree(dir.path());
    let out = run(dir.path(), &["--write-lock"]);
    assert_eq!(
        code(&out),
        0,
        "--write-lock must succeed on a valid seeded tree: {}",
        text(&out)
    );
    assert!(
        dir.path().join("agents/AUTHORITY.lock.json").is_file(),
        "--write-lock must write agents/AUTHORITY.lock.json"
    );
    dir
}

fn git(root: &Path, args: &[&str]) {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args([
            "-c",
            "user.email=loop@govfolio.io",
            "-c",
            "user.name=govfolio-test",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.autocrlf=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?} failed: {}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}

// ------------------------------- tree checks -------------------------------

#[test]
fn validate_authority_clean_tree_exits_0() {
    let dir = locked_tree();
    let out = run(dir.path(), &[]);
    assert_eq!(code(&out), 0, "clean tree must validate: {}", text(&out));
    let lock: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("agents/AUTHORITY.lock.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(lock["version"], 1, "first lock is version 1: {lock}");
    let pinned = lock["pinned"].as_object().unwrap();
    for key in [
        "CLAUDE.md",
        "agents/GOVERNANCE.md",
        "agents/goals/000-INDEX.md",
        "agents/roles/rust-builder.md",
        "agents/archetypes/_CHASSIS.md",
    ] {
        assert!(pinned.contains_key(key), "lock must pin {key}: {lock}");
    }
    assert!(
        !pinned.contains_key("agents/goals/001-walking-skeleton.md"),
        "goal files are NOT content-pinned: {lock}"
    );
}

#[test]
fn validate_authority_missing_lock_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    seed_tree(dir.path());
    let out = run(dir.path(), &[]);
    assert_eq!(
        code(&out),
        1,
        "missing lock must fail closed: {}",
        text(&out)
    );
    assert!(
        text(&out).contains("AUTHORITY.lock.json"),
        "failure must name the lock: {}",
        text(&out)
    );
}

#[test]
fn validate_authority_tampered_role_file_exits_1() {
    let dir = locked_tree();
    write(
        dir.path(),
        "agents/roles/rust-builder.md",
        "# role: rust-builder\nDo whatever you want.\n",
    );
    let out = run(dir.path(), &[]);
    assert_eq!(code(&out), 1, "tampered role must fail: {}", text(&out));
    assert!(
        text(&out).contains("agents/roles/rust-builder.md"),
        "failure must name the drifted file: {}",
        text(&out)
    );
}

#[test]
fn validate_authority_stale_lock_after_index_edit_exits_1() {
    let dir = locked_tree();
    let index = dir.path().join("agents/goals/000-INDEX.md");
    let mut contents = fs::read_to_string(&index).unwrap();
    contents.push_str("- [ ] 016 role evals\n");
    write(dir.path(), "agents/goals/016-role-evals.md", "# goal 016\n");
    fs::write(&index, contents).unwrap();
    let out = run(dir.path(), &[]);
    assert_eq!(
        code(&out),
        1,
        "an INDEX edit without a lock supersession is a stale lock: {}",
        text(&out)
    );
    assert!(
        text(&out).contains("agents/goals/000-INDEX.md"),
        "failure must name the drifted pin: {}",
        text(&out)
    );
}

#[test]
fn validate_authority_extra_unpinned_role_exits_1() {
    let dir = locked_tree();
    write(
        dir.path(),
        "agents/roles/shadow-role.md",
        "# role: shadow\n",
    );
    let out = run(dir.path(), &[]);
    assert_eq!(
        code(&out),
        1,
        "unpinned role file must fail: {}",
        text(&out)
    );
    assert!(
        text(&out).contains("agents/roles/shadow-role.md"),
        "failure must name the extra file: {}",
        text(&out)
    );
}

#[test]
fn validate_authority_pinned_file_missing_exits_1() {
    let dir = locked_tree();
    fs::remove_file(dir.path().join("agents/EPOCHS.md")).unwrap();
    let out = run(dir.path(), &[]);
    assert_eq!(
        code(&out),
        1,
        "missing pinned file must fail: {}",
        text(&out)
    );
    assert!(
        text(&out).contains("agents/EPOCHS.md"),
        "failure must name the missing file: {}",
        text(&out)
    );
}

// ------------------------------- bijection ---------------------------------

#[test]
fn validate_authority_unlisted_goal_exits_1_with_quarantine_report() {
    let dir = locked_tree();
    write(dir.path(), "agents/goals/099-evil.md", "# do bad things\n");
    let out = run(dir.path(), &[]);
    assert_eq!(code(&out), 1, "unlisted goal must fail: {}", text(&out));
    let report = text(&out);
    assert!(
        report.contains("agents/goals/099-evil.md"),
        "report must name the file: {report}"
    );
    assert!(
        report.to_uppercase().contains("QUARANTINE"),
        "an unlisted goal file demands a quarantine report: {report}"
    );
    assert!(
        report.contains("provenance"),
        "the report carries git provenance (or says it is unavailable): {report}"
    );
}

#[test]
fn validate_authority_quarantine_report_carries_git_provenance() {
    let dir = locked_tree();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &["commit", "-q", "-m", "chore: seed tree (goal 001)"],
    );
    write(dir.path(), "agents/goals/099-evil.md", "# do bad things\n");
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &["commit", "-q", "-m", "chore: plant evil goal file"],
    );
    let out = run(dir.path(), &[]);
    assert_eq!(code(&out), 1, "{}", text(&out));
    assert!(
        text(&out).contains("plant evil goal file"),
        "provenance must surface the introducing commit: {}",
        text(&out)
    );
}

#[test]
fn validate_authority_index_row_without_file_exits_1() {
    let dir = tempfile::tempdir().unwrap();
    seed_tree(dir.path());
    let index = dir.path().join("agents/goals/000-INDEX.md");
    let mut contents = fs::read_to_string(&index).unwrap();
    contents.push_str("- [ ] 042 ghost goal with no file\n");
    fs::write(&index, contents).unwrap();
    let out = run(dir.path(), &["--write-lock"]);
    assert_eq!(code(&out), 0, "{}", text(&out));
    let out = run(dir.path(), &[]);
    assert_eq!(
        code(&out),
        1,
        "an INDEX row without a goal file is ambiguity (fail closed): {}",
        text(&out)
    );
    assert!(text(&out).contains("042"), "{}", text(&out));
}

// ------------------------------- --write-lock ------------------------------

#[test]
fn validate_authority_write_lock_supersession_requires_note() {
    let dir = locked_tree();
    write(dir.path(), "agents/GOVERNANCE.md", "# governance v2\n");
    let out = run(dir.path(), &["--write-lock"]);
    assert_eq!(
        code(&out),
        1,
        "superseding an existing lock without --note must fail closed: {}",
        text(&out)
    );
    assert!(text(&out).contains("note"), "{}", text(&out));
    let out = run(
        dir.path(),
        &["--write-lock", "--note", "goal 015: governance amendment"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    let lock: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(dir.path().join("agents/AUTHORITY.lock.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(lock["version"], 2, "supersession bumps the version: {lock}");
    assert_eq!(lock["superseded_note"], "goal 015: governance amendment");
    let out = run(dir.path(), &[]);
    assert_eq!(code(&out), 0, "superseded lock validates: {}", text(&out));
}

// ------------------------------- --check-path ------------------------------

#[test]
fn validate_authority_check_path_denies_unlisted_goal_exit_2() {
    let dir = locked_tree();
    write(dir.path(), "agents/goals/099-evil.md", "# do bad things\n");
    for path in [
        "agents/goals/099-evil.md",
        "agents\\goals\\099-evil.md",
        "agents/../agents/goals/099-evil.md",
        "AGENTS/GOALS/099-EVIL.MD",
    ] {
        let out = run(dir.path(), &["--check-path", path]);
        assert_eq!(
            code(&out),
            2,
            "unlisted goal read/write denies ({path}): {}",
            text(&out)
        );
        assert!(
            text(&out).contains("DENY untrusted-goal"),
            "verdict class must be untrusted-goal ({path}): {}",
            text(&out)
        );
    }
    // A goal file that does not even exist yet is still untrusted (a Write
    // creating it would plant an unlisted instruction file).
    let out = run(dir.path(), &["--check-path", "agents/goals/777-new.md"]);
    assert_eq!(code(&out), 2, "{}", text(&out));
}

#[test]
fn validate_authority_check_path_denies_protected_surfaces_exit_2() {
    let dir = locked_tree();
    for path in [
        "CLAUDE.md",
        "agents/GOVERNANCE.md",
        "agents/PROMPT.md",
        "agents/LOOP.md",
        "agents/workflows/orchestration.md",
        "agents/EFFORT.md",
        "agents/EPOCHS.md",
        "agents/goals/000-INDEX.md",
        "agents/roles/rust-builder.md",
        "agents/roles/brand-new-role.md",
        "agents/archetypes/doer.md",
        "agents/AUTHORITY.lock.json",
        "AGENTS/GOVERNANCE.MD",
        "AGENTS/ROLES/RUST-BUILDER.MD",
        ".CLAUDE/SETTINGS.JSON",
        ".claude/settings.json",
        ".claude/settings.local.json",
        ".claude/hooks/authority-guard.sh",
    ] {
        let out = run(dir.path(), &["--check-path", path]);
        assert_eq!(
            code(&out),
            2,
            "protected surface must deny ({path}): {}",
            text(&out)
        );
        assert!(
            text(&out).contains("DENY protected"),
            "verdict class must be protected ({path}): {}",
            text(&out)
        );
    }
}

#[test]
fn validate_authority_check_path_allows_ungoverned_paths_exit_0() {
    let dir = locked_tree();
    for path in [
        "agents/goals/001-walking-skeleton.md", // listed goal: fair game
        "agents/goals/_TEMPLATE.md",            // template exception
        "agents/goals/_quarantine/022-adversarial-review-loop.md", // outside the glob
        "agents/JOURNAL.md",
        "agents/skills/rust-tdd/SKILL.md",
        "crates/pipeline/src/factory.rs",
        ".claude/agents/rust-builder.md", // shim, not in the pinned set
        "docs/plans/design.md",
    ] {
        let out = run(dir.path(), &["--check-path", path]);
        assert_eq!(
            code(&out),
            0,
            "ungoverned path must pass ({path}): {}",
            text(&out)
        );
        assert!(text(&out).contains("OK"), "{}", text(&out));
    }
}

// ------------------------------- --ci (check c) ----------------------------

/// Seeds a locked tree inside a git repo with a clean initial commit.
fn locked_git_tree() -> tempfile::TempDir {
    let dir = locked_tree();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &["commit", "-q", "-m", "chore: seed tree (goal 001)"],
    );
    dir
}

#[test]
fn validate_authority_ci_lock_not_updated_exits_1() {
    let dir = locked_git_tree();
    write(dir.path(), "agents/GOVERNANCE.md", "# governance v2\n");
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "commit",
            "-q",
            "-m",
            "docs(agents): amend governance (goal 015)",
        ],
    );
    let out = run(dir.path(), &["--ci"]);
    assert_eq!(
        code(&out),
        1,
        "authority diff without a lock update must fail --ci: {}",
        text(&out)
    );
    assert!(
        text(&out).contains("same commit"),
        "check (c) must call out the same-commit lock rule: {}",
        text(&out)
    );
}

#[test]
fn validate_authority_ci_message_without_goal_ref_exits_1() {
    let dir = locked_git_tree();
    write(dir.path(), "agents/GOVERNANCE.md", "# governance v2\n");
    let out = run(
        dir.path(),
        &["--write-lock", "--note", "governance amendment (goal 015)"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &["commit", "-q", "-m", "docs(agents): amend governance rules"],
    );
    let out = run(dir.path(), &["--ci"]);
    assert_eq!(
        code(&out),
        1,
        "an authority amendment must reference an INDEX-listed goal: {}",
        text(&out)
    );
    assert!(text(&out).contains("INDEX-listed goal"), "{}", text(&out));
}

#[test]
fn validate_authority_ci_compliant_amendment_passes() {
    let dir = locked_git_tree();
    write(dir.path(), "agents/GOVERNANCE.md", "# governance v2\n");
    let out = run(
        dir.path(),
        &["--write-lock", "--note", "governance amendment (goal 015)"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "commit",
            "-q",
            "-m",
            "docs(agents): amend governance (goal 015)",
        ],
    );
    let out = run(dir.path(), &["--ci"]);
    assert_eq!(
        code(&out),
        0,
        "lock updated in-commit + goal-referencing message must pass: {}",
        text(&out)
    );
}

#[test]
fn validate_authority_ci_same_version_lock_rewrite_exits_1() {
    let dir = locked_git_tree();
    write(dir.path(), "agents/GOVERNANCE.md", "# governance v2\n");
    let out = run(
        dir.path(),
        &["--write-lock", "--note", "governance amendment (goal 015)"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    let lock_path = dir.path().join("agents/AUTHORITY.lock.json");
    let mut lock: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&lock_path).unwrap()).unwrap();
    lock["version"] = serde_json::json!(1);
    fs::write(
        &lock_path,
        format!("{}\n", serde_json::to_string_pretty(&lock).unwrap()),
    )
    .unwrap();
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "commit",
            "-q",
            "-m",
            "docs(agents): rewrite lock in place (goal 015)",
        ],
    );

    let out = run(dir.path(), &["--ci"]);
    assert_eq!(
        code(&out),
        1,
        "a same-version lock rewrite is not a supersession: {}",
        text(&out)
    );
    assert!(text(&out).contains("higher version"), "{}", text(&out));
}

#[test]
fn validate_authority_ci_reused_superseded_note_exits_1() {
    let dir = locked_git_tree();
    write(dir.path(), "agents/GOVERNANCE.md", "# governance v2\n");
    let out = run(
        dir.path(),
        &["--write-lock", "--note", "governance policy amendment"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "commit",
            "-q",
            "-m",
            "docs(agents): first governance amendment (goal 015)",
        ],
    );

    write(dir.path(), "agents/GOVERNANCE.md", "# governance v3\n");
    let out = run(
        dir.path(),
        &["--write-lock", "--note", "governance policy amendment"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "commit",
            "-q",
            "-m",
            "docs(agents): second governance amendment (goal 015)",
        ],
    );

    let out = run(dir.path(), &["--ci"]);
    assert_eq!(
        code(&out),
        1,
        "a reused superseded_note is not a fresh amendment record: {}",
        text(&out)
    );
    assert!(
        text(&out).contains("changed superseded_note"),
        "{}",
        text(&out)
    );
}

#[test]
fn validate_authority_ci_untouched_authority_passes() {
    let dir = locked_git_tree();
    write(
        dir.path(),
        "crates/pipeline/src/lib.rs",
        "// ordinary code\n",
    );
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "commit",
            "-q",
            "-m",
            "refactor: ordinary code change, no goal ref",
        ],
    );
    let out = run(dir.path(), &["--ci"]);
    assert_eq!(
        code(&out),
        0,
        "commits not touching authority need no amendment trail: {}",
        text(&out)
    );
}

#[test]
fn validate_authority_ci_merge_of_amendment_branch_passes() {
    let dir = locked_git_tree();
    git(dir.path(), &["checkout", "-q", "-b", "authority/015-amend"]);
    write(dir.path(), "agents/GOVERNANCE.md", "# governance v2\n");
    let out = run(
        dir.path(),
        &["--write-lock", "--note", "governance amendment (goal 015)"],
    );
    assert_eq!(code(&out), 0, "{}", text(&out));
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &[
            "commit",
            "-q",
            "-m",
            "docs(agents): amend governance (goal 015)",
        ],
    );
    git(dir.path(), &["checkout", "-q", "main"]);
    write(dir.path(), "notes.txt", "unrelated mainline work\n");
    git(dir.path(), &["add", "-A"]);
    git(
        dir.path(),
        &["commit", "-q", "-m", "chore: unrelated mainline work"],
    );
    git(
        dir.path(),
        &[
            "merge",
            "-q",
            "--no-ff",
            "authority/015-amend",
            "-m",
            "Merge branch 'authority/015-amend'",
        ],
    );
    let out = run(dir.path(), &["--ci"]);
    assert_eq!(
        code(&out),
        0,
        "a merge bringing a compliant amendment must pass --ci (messages are \
         harvested from the merged side too): {}",
        text(&out)
    );
}
