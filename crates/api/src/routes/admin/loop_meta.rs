//! `GET /v1/admin/loop` — section H, autonomous-loop meta (repo-root-gated):
//! goal queue parsed from `agents/goals/000-INDEX.md` with HALT detection
//! (H1), git activity via subprocess with git-failure → `git: null` (H2),
//! guardrail trips from `agents/JOURNAL.md` (H3). When
//! [`crate::ApiConfig::repo_root`] is `None` (the cloud posture) the
//! endpoint answers 503 Unavailable by design.
//!
//! READ-ONLY: filesystem reads and read-only `git` subcommands only — no DB,
//! no writes. All blocking work (fs + subprocess) runs inside
//! `tokio::task::spawn_blocking`.

use std::path::Path;
use std::process::Command;

use anyhow::Context as _;
use axum::Json;
use axum::extract::State;
use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;

use crate::AppState;
use crate::error::{ApiError, ErrorBody};

// ------------------------------------------------------------- wire shapes --

/// One goal-queue entry from `agents/goals/000-INDEX.md` (H1).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminLoopGoal {
    /// Goal number token as written (e.g. `080`, `E2+`).
    pub number: String,
    /// The rest of the line, verbatim.
    pub title: String,
    /// `done` (`[x]`) | `in_progress` (`[~]`) | `open` (`[ ]`).
    pub state: String,
    /// The line mentions `HALT` without a `HALT RESOLVED` marker.
    pub halted: bool,
}

/// One commit from `git log` (H2).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminLoopCommit {
    /// Full commit sha.
    pub sha: String,
    /// Commit timestamp.
    pub committed_at: DateTime<Utc>,
    /// Subject line, verbatim.
    pub subject: String,
}

/// Git activity of the mounted checkout (H2).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminLoopGit {
    /// Current branch; `null` on a detached HEAD.
    pub branch: Option<String>,
    /// `git status --porcelain` line count (dirty paths).
    pub dirty_files: u64,
    /// Last 20 commits, newest first.
    pub commits: Vec<AdminLoopCommit>,
}

/// One `BACKFILL_BUDGET skip:` guardrail trip from `agents/JOURNAL.md` (H3).
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminBudgetSkip {
    /// Journal entry date (the first `|`-separated field, trimmed).
    pub date: String,
    /// The journal line, verbatim.
    pub line: String,
}

/// Section H — autonomous-loop meta, one round trip.
#[derive(Debug, Serialize, ToSchema)]
pub struct AdminLoop {
    /// When this snapshot was computed.
    pub generated_at: DateTime<Utc>,
    /// The repo checkout the metadata was read from (`GOVFOLIO_REPO_ROOT`).
    pub repo_root: String,
    /// Goal queue, in file order.
    pub goals: Vec<AdminLoopGoal>,
    /// Git activity; `null` when any git subprocess failed (goals still
    /// render — git absence is honest, not fatal).
    pub git: Option<AdminLoopGit>,
    /// `BACKFILL_BUDGET skip:` trips from `agents/JOURNAL.md`, in file
    /// order; `null` when the journal could not be read (an unreadable
    /// journal is stated, not rendered as "no trips").
    pub budget_skips: Option<Vec<AdminBudgetSkip>>,
}

// ------------------------------------------------------------- H1: goals --

/// Parses one `- [x|~| ] <number> <title...>` line; `None` for anything else
/// (headers, prose, malformed lines are skipped, never a panic).
fn parse_goal_line(line: &str) -> Option<AdminLoopGoal> {
    let rest = line.strip_prefix("- [")?;
    let mut chars = rest.chars();
    let state = match chars.next()? {
        'x' => "done",
        '~' => "in_progress",
        ' ' => "open",
        _ => return None,
    };
    let rest = chars.as_str().strip_prefix("] ")?;
    let (number, title) = rest.split_once(' ')?;
    if number.is_empty() || title.trim().is_empty() {
        return None;
    }
    Some(AdminLoopGoal {
        number: number.to_owned(),
        title: title.trim().to_owned(),
        state: state.to_owned(),
        halted: line.contains("HALT") && !line.contains("HALT RESOLVED"),
    })
}

fn read_goals(root: &Path) -> anyhow::Result<Vec<AdminLoopGoal>> {
    let path = root.join("agents").join("goals").join("000-INDEX.md");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading goal index {}", path.display()))?;
    Ok(text.lines().filter_map(parse_goal_line).collect())
}

// --------------------------------------------------------------- H2: git --

/// Runs one read-only git subcommand against the checkout; `None` on any
/// spawn failure, non-zero exit, or non-UTF-8 output.
fn git_stdout(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// `%H<TAB>%ct<TAB>%s` → commit; malformed lines are skipped.
fn parse_log_line(line: &str) -> Option<AdminLoopCommit> {
    let mut parts = line.splitn(3, '\t');
    let sha = parts.next()?;
    let epoch: i64 = parts.next()?.parse().ok()?;
    let subject = parts.next()?;
    let committed_at = DateTime::from_timestamp(epoch, 0)?;
    Some(AdminLoopCommit {
        sha: sha.to_owned(),
        committed_at,
        subject: subject.to_owned(),
    })
}

/// Collects branch + dirty count + last 20 commits. Any git failure yields
/// `None` for the whole block — goals must still render without git.
fn git_activity(root: &Path) -> Option<AdminLoopGit> {
    let log = git_stdout(root, &["log", "--format=%H%x09%ct%x09%s", "-n", "20"])?;
    let branch_raw = git_stdout(root, &["branch", "--show-current"])?;
    let status = git_stdout(root, &["status", "--porcelain"])?;

    let branch = {
        let trimmed = branch_raw.trim();
        // Empty output = detached HEAD (the command still succeeds).
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    };
    let dirty = status.lines().filter(|l| !l.trim().is_empty()).count();
    Some(AdminLoopGit {
        branch,
        dirty_files: u64::try_from(dirty).unwrap_or(u64::MAX),
        commits: log.lines().filter_map(parse_log_line).collect(),
    })
}

// ----------------------------------------------------------- H3: journal --

/// Journal entry date: the first `|`-separated field, trimmed.
fn journal_date(line: &str) -> String {
    line.split('|').next().unwrap_or("").trim().to_owned()
}

/// `None` when `agents/JOURNAL.md` cannot be read (stated, not faked as "no
/// trips"); `Some(vec)` otherwise, possibly empty.
fn read_budget_skips(root: &Path) -> Option<Vec<AdminBudgetSkip>> {
    let path = root.join("agents").join("JOURNAL.md");
    let text = std::fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .filter(|line| line.contains("BACKFILL_BUDGET skip:"))
            .map(|line| AdminBudgetSkip {
                date: journal_date(line),
                line: line.to_owned(),
            })
            .collect(),
    )
}

// ------------------------------------------------------------- the handler --

/// Autonomous-loop meta (section H): goal queue with HALT detection, git
/// activity, guardrail trips. Only available where the repo checkout is
/// mounted (`GOVFOLIO_REPO_ROOT`).
///
/// # Errors
/// `503` when `GOVFOLIO_REPO_ROOT` is not configured (the cloud posture — by
/// design); `500` when the goal index cannot be read or parsed — consistent
/// error envelope.
#[utoipa::path(
    get,
    path = "/v1/admin/loop",
    tag = "admin",
    responses(
        (status = 200, description = "Autonomous-loop meta snapshot", body = AdminLoop),
        (status = 401, description = "Missing or invalid admin token", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
        (status = 503, description = "GOVFOLIO_REPO_ROOT is not set (cloud posture)", body = ErrorBody),
    ),
)]
pub async fn admin_loop(State(state): State<AppState>) -> Result<Json<AdminLoop>, ApiError> {
    let Some(root) = state.config.repo_root.clone() else {
        return Err(ApiError::Unavailable {
            code: "repo_root_unset",
            message: "GOVFOLIO_REPO_ROOT is not set; loop metadata is only available where \
                      the repo is mounted"
                .to_owned(),
        });
    };
    let repo_root = root.display().to_string();
    let (goals, git, budget_skips) = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
        let goals = read_goals(&root)?;
        let git = git_activity(&root);
        let budget_skips = read_budget_skips(&root);
        Ok((goals, git, budget_skips))
    })
    .await
    .map_err(|e| anyhow::anyhow!("loop metadata task failed: {e}"))??;
    Ok(Json(AdminLoop {
        generated_at: Utc::now(),
        repo_root,
        goals,
        git,
        budget_skips,
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{journal_date, parse_goal_line, parse_log_line};

    // Real lines from agents/goals/000-INDEX.md (2026-07-08) — the parser is
    // validated against the actual file format, not an invented one.
    const LINE_080: &str = "- [~] 080 US backfill to 2012 + launch checklist — BUILDABLE HALF \
         DONE 2026-07-06 commit a078fac (dry-run diff over 7,544 real PTRs 2012→2026, zero \
         writes proven, launch-checklist.md); HALT (human/infra): real prod backfill needs \
         ADC+terraform+founder diff go/no-go; legal/methodology PUBLIC pages = human lane; \
         launch go/no-go = human";
    const LINE_021: &str = "- [~] 021 LLM extraction fallback (schema-constrained, sha-cached, \
         confidence) — RE-OPENED 2026-07-07 as Phase 2 consensus expansion; HALT RESOLVED \
         2026-07-08: HARD CAP = USD 200/month (founder; Task 9 [budget] 20M/80M tokens)";

    #[test]
    fn goal_080_is_in_progress_and_halted() {
        let goal = parse_goal_line(LINE_080).unwrap();
        assert_eq!(goal.number, "080");
        assert_eq!(goal.state, "in_progress");
        assert!(goal.halted);
    }

    #[test]
    fn goal_021_halt_resolved_is_not_halted() {
        let goal = parse_goal_line(LINE_021).unwrap();
        assert_eq!(goal.number, "021");
        assert_eq!(goal.state, "in_progress");
        assert!(!goal.halted);
    }

    #[test]
    fn done_open_and_nonnumeric_goal_lines_parse() {
        let done = parse_goal_line("- [x] 030 alerts (outbox dispatcher) (done)").unwrap();
        assert_eq!((done.state.as_str(), done.halted), ("done", false));
        let open = parse_goal_line("- [ ] 081 US backfill real write execution").unwrap();
        assert_eq!(open.state, "open");
        let epoch = parse_goal_line("- [ ] E2+ Brazil onward: NO hand-written goals").unwrap();
        assert_eq!(epoch.number, "E2+");
    }

    #[test]
    fn non_goal_lines_are_skipped() {
        assert!(parse_goal_line("# Goal queue (ordered — loop picks first unchecked)").is_none());
        assert!(parse_goal_line("").is_none());
        assert!(parse_goal_line("- [?] 999 bogus marker").is_none());
    }

    #[test]
    fn journal_date_is_first_pipe_field() {
        let line = "2026-07-07 | 081/T4 | BACKFILL_BUDGET skip: br 2022 record_delta=904 \
             exceeds budget=500 | none — nothing blocks; a later invocation retries 2022";
        assert_eq!(journal_date(line), "2026-07-07");
    }

    #[test]
    fn log_line_parses_and_malformed_is_skipped() {
        let commit = parse_log_line("0185453ab\t1751932800\tdocs(agents): goal done").unwrap();
        assert_eq!(commit.sha, "0185453ab");
        assert_eq!(commit.subject, "docs(agents): goal done");
        assert!(parse_log_line("not-a-log-line").is_none());
    }
}
