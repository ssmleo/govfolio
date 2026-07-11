//! Read-only factory/loop dashboard (drives `agents/monitor.sh` via the
//! `loop-board` bin). Assembles git + journal + goals + logs + procs +
//! registry into one human-text snapshot. Zero writes.

mod git_journal;
mod goals;
mod logs;
mod procs;
mod registry;
mod tripwire;

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

pub use git_journal::{CommitDigest, GitView, JournalDigest, rel_age, truncate};
pub use goals::GoalsView;
pub use logs::LogView;
pub use procs::ProcView;
pub use registry::{DoingLease, RegistryView};
pub use tripwire::{Alert, Severity};

/// Full one-shot monitor snapshot.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub now: DateTime<Utc>,
    pub repo: PathBuf,
    pub epoch_label: String,
    pub branch: String,
    pub head_sha: String,
    pub head_age_min: Option<i64>,
    pub registry: RegistryView,
    pub git: GitView,
    pub journal: Vec<JournalDigest>,
    pub goals: GoalsView,
    pub logs: Vec<LogView>,
    pub procs: ProcView,
    pub alerts: Vec<Alert>,
}

/// Collect a read-only snapshot rooted at `repo` (govfolio checkout).
pub async fn collect(repo: &Path) -> Snapshot {
    let now = Utc::now();
    let epoch_label = std::env::var("GOVFOLIO_EPOCH").unwrap_or_else(|_| "E2".to_owned());
    let epoch_num = parse_epoch_label(&epoch_label);

    let git = git_journal::collect_git(repo, now);
    let journal = git_journal::collect_journal(repo, now);
    let goals = goals::collect(repo);
    let logs = logs::collect(repo, now);
    let procs = procs::collect();
    let registry = registry::collect(epoch_num).await;

    let mut snap = Snapshot {
        now,
        repo: repo.to_path_buf(),
        epoch_label,
        branch: git.branch.clone(),
        head_sha: git.head_sha.clone(),
        head_age_min: git.head_age_min,
        registry,
        git,
        journal,
        goals,
        logs,
        procs,
        alerts: Vec::new(),
    };
    snap.alerts = tripwire::evaluate(&snap);
    snap
}

/// Render the human dashboard (section order fixed by design).
#[must_use]
pub fn render(snap: &Snapshot) -> String {
    let mut out = String::with_capacity(8 * 1024);
    render_header(&mut out, snap);
    out.push('\n');
    render_tripwires(&mut out, snap);
    out.push('\n');
    render_procs(&mut out, snap);
    out.push('\n');
    render_doing(&mut out, snap);
    out.push('\n');
    render_done_left(&mut out, snap);
    out.push('\n');
    render_goals(&mut out, snap);
    out.push('\n');
    render_journal(&mut out, snap);
    out.push('\n');
    render_commits(&mut out, snap);
    out.push('\n');
    render_logs(&mut out, snap);
    out
}

fn render_header(out: &mut String, snap: &Snapshot) {
    let head_age = snap.head_age_min.map_or_else(|| "?".to_owned(), rel_age);
    let _ = writeln!(
        out,
        "== govfolio loop-board | {} | branch: {} @ {} (HEAD {}) | epoch {} ==",
        snap.now.format("%Y-%m-%dT%H:%M:%SZ"),
        snap.branch,
        short_sha(&snap.head_sha),
        head_age,
        snap.epoch_label,
    );
}

fn render_tripwires(out: &mut String, snap: &Snapshot) {
    out.push_str("-- TRIPWIRES --\n");
    if snap.alerts.is_empty() {
        out.push_str("(none)\n");
        return;
    }
    for a in &snap.alerts {
        let tag = match a.severity {
            Severity::Stall => "STALL",
            Severity::Warn => "WARN",
        };
        let _ = writeln!(out, "{tag}: {}", a.message);
    }
}

fn render_procs(out: &mut String, snap: &Snapshot) {
    out.push_str("-- LIVE PROCS --\n");
    if snap.procs.unavailable {
        out.push_str("(unavailable: process list failed)\n");
        return;
    }
    let _ = writeln!(
        out,
        "host-wide (unscoped): claude={}  codex={}  run-loop-ish={}",
        snap.procs.claude, snap.procs.codex, snap.procs.run_loop_ish
    );
    if !snap.procs.samples.is_empty() {
        let joined = snap.procs.samples.join(", ");
        let _ = writeln!(out, "sample: {}", truncate(&joined, 160));
    }
}

fn render_doing(out: &mut String, snap: &Snapshot) {
    out.push_str("-- DOING (live leases) --\n");
    match &snap.registry {
        RegistryView::Unavailable(reason) => {
            let _ = writeln!(out, "(unavailable: {reason})");
        }
        RegistryView::Ok(board) => {
            if board.doing.is_empty() {
                out.push_str("(no-leases)\n");
            } else {
                for d in &board.doing {
                    let activity = match_activity_for_lease(snap, d);
                    if let Some(act) = activity {
                        let _ = writeln!(
                            out,
                            "lease id={} by={} phase={} age_min={} | {}",
                            d.id,
                            d.claimed_by,
                            d.coverage_phase,
                            d.age_min,
                            truncate(&act, 100)
                        );
                    } else {
                        let _ = writeln!(
                            out,
                            "lease id={} by={} phase={} age_min={}",
                            d.id, d.claimed_by, d.coverage_phase, d.age_min
                        );
                    }
                }
            }
        }
    }
}

fn render_done_left(out: &mut String, snap: &Snapshot) {
    out.push_str("-- DONE / LEFT (registry) --\n");
    match &snap.registry {
        RegistryView::Unavailable(reason) => {
            let _ = writeln!(out, "(unavailable: {reason})");
        }
        RegistryView::Ok(board) => {
            out.push_str("phases:");
            if board.phase_counts.is_empty() {
                out.push_str(" (empty registry)\n");
            } else {
                for (phase, n) in &board.phase_counts {
                    let _ = write!(out, " {phase}={n}");
                }
                out.push('\n');
            }
            if !board.epoch_counts.is_empty() {
                out.push_str("epochs:");
                for (ep, n) in &board.epoch_counts {
                    let label = ep.map_or_else(|| "null".to_owned(), |e| format!("E{e}"));
                    let _ = write!(out, " {label}={n}");
                }
                out.push('\n');
            }
            let _ = writeln!(
                out,
                "claimable@{}: {}",
                snap.epoch_label, board.claimable_at_epoch
            );
            if !board.blocked_reasons.is_empty() {
                out.push_str("blocked:");
                for (reason, n) in &board.blocked_reasons {
                    let _ = write!(out, " {}×{}", truncate(reason, 40), n);
                }
                out.push('\n');
            }
            if !board.left_sample.is_empty() {
                out.push_str("left (top by priority):\n");
                for row in &board.left_sample {
                    let ep = row
                        .epoch
                        .map_or_else(|| "null".to_owned(), |e| e.to_string());
                    let pri = row
                        .priority_score
                        .map_or_else(|| "-".to_owned(), |p| p.to_string());
                    let _ = writeln!(
                        out,
                        "  {} phase={} epoch={} pri={}",
                        row.id, row.coverage_phase, ep, pri
                    );
                }
            }
        }
    }
}

fn render_goals(out: &mut String, snap: &Snapshot) {
    out.push_str("-- GOALS / BLOCKED --\n");
    if snap.goals.open.is_empty() {
        out.push_str("(no open goals in INDEX)\n");
    } else {
        out.push_str("open (next 8):\n");
        for g in snap.goals.open.iter().take(8) {
            let _ = writeln!(out, "  [{}] {}", g.mark, truncate(&g.line, 100));
        }
    }
    if snap.goals.blocked_human.is_empty() {
        out.push_str("blocked(human): (none)\n");
    } else {
        out.push_str("blocked(human):\n");
        for b in snap.goals.blocked_human.iter().take(12) {
            let _ = writeln!(out, "  {} | {}", b.goal, truncate(&b.snippet, 90));
        }
    }
}

fn render_journal(out: &mut String, snap: &Snapshot) {
    out.push_str("-- JOURNAL (digest, last 8) --\n");
    if snap.journal.is_empty() {
        out.push_str("(no iterations yet)\n");
        return;
    }
    for j in snap.journal.iter().take(8) {
        let _ = writeln!(out, "{} | {} | {}", j.rel_age, j.tag, j.summary);
    }
}

fn render_commits(out: &mut String, snap: &Snapshot) {
    out.push_str("-- COMMITS (last 8) --\n");
    if snap.git.commits.is_empty() {
        let err = snap.git.error.as_deref().unwrap_or("?");
        let _ = writeln!(out, "(unavailable: {err})");
        return;
    }
    for c in snap.git.commits.iter().take(8) {
        let _ = writeln!(out, "{} | {} | {}", c.rel_age, c.sha, c.subject);
    }
}

fn render_logs(out: &mut String, snap: &Snapshot) {
    out.push_str("-- LOG TAILS --\n");
    if snap.logs.is_empty() {
        out.push_str("(no loop logs under agents/)\n");
        return;
    }
    for log in &snap.logs {
        let _ = writeln!(
            out,
            "--- {} ({}, mtime {}) ---",
            log.name, log.stack, log.mtime_rel
        );
        if log.signals.is_empty() {
            out.push_str("  (empty)\n");
        } else {
            for s in &log.signals {
                let _ = writeln!(out, "  {}", truncate(s, 120));
            }
        }
    }
}

fn match_activity_for_lease(snap: &Snapshot, lease: &DoingLease) -> Option<String> {
    let id = lease.id.to_ascii_lowercase();
    let by = lease.claimed_by.to_ascii_lowercase();
    for log in &snap.logs {
        for sig in &log.signals {
            let low = sig.to_ascii_lowercase();
            if low.contains(&id) || low.contains(&by) {
                return Some(sig.clone());
            }
        }
        if log.name.to_ascii_lowercase().contains(&by) {
            return log.signals.first().cloned();
        }
    }
    None
}

fn short_sha(sha: &str) -> &str {
    if sha.len() >= 7 {
        &sha[..7]
    } else if sha.is_empty() {
        "?"
    } else {
        sha
    }
}

/// `2` or `E2` → 2. Invalid → 2 (monitor default epoch).
#[must_use]
pub fn parse_epoch_label(raw: &str) -> i16 {
    raw.trim()
        .trim_start_matches(['E', 'e'])
        .parse()
        .unwrap_or(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_epoch_variants() {
        assert_eq!(parse_epoch_label("E2"), 2);
        assert_eq!(parse_epoch_label("2"), 2);
        assert_eq!(parse_epoch_label("e3"), 3);
        assert_eq!(parse_epoch_label("nope"), 2);
    }

    #[test]
    fn short_sha_truncates() {
        assert_eq!(short_sha("abcdef012345"), "abcdef0");
        assert_eq!(short_sha(""), "?");
        assert_eq!(short_sha("ab"), "ab");
    }
}
