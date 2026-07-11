//! Aggressive stall / warn rules over an assembled snapshot (pure).

use crate::board::logs::has_epoch_gate_thrash;
use crate::board::procs::any_agent_alive;
use crate::board::{RegistryView, Snapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Stall,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alert {
    pub severity: Severity,
    pub message: String,
}

// Aggressive thresholds (minutes) — design v1 fixed.
const LEASE_STALL_MIN: i64 = 60;
const LEASE_WARN_MIN: i64 = 30;
const LOG_QUIET_MIN: i64 = 10;
const NO_COMMIT_ACTIVE_MIN: i64 = 30;
const FRESH_LOG_MIN: i64 = 15;
const JOURNAL_COMMIT_DESYNC_MIN: i64 = 5;

pub fn evaluate(snap: &Snapshot) -> Vec<Alert> {
    let mut alerts = Vec::new();

    lease_rules(snap, &mut alerts);
    log_quiet_on_lease(snap, &mut alerts);
    proc_dead_with_lease(snap, &mut alerts);
    active_no_commit(snap, &mut alerts);
    claimable_all_idle(snap, &mut alerts);
    journal_commit_desync(snap, &mut alerts);
    epoch_gate_thrash(snap, &mut alerts);

    alerts
}

fn lease_rules(snap: &Snapshot, alerts: &mut Vec<Alert>) {
    let RegistryView::Ok(board) = &snap.registry else {
        return;
    };
    for d in &board.doing {
        if d.age_min > LEASE_STALL_MIN {
            let recent_signal =
                journal_or_commit_mentions(snap, &d.id, &d.claimed_by, LEASE_STALL_MIN);
            if !recent_signal {
                alerts.push(Alert {
                    severity: Severity::Stall,
                    message: format!(
                        "lease {} by={} age_min={} with no journal/commit signal in window",
                        d.id, d.claimed_by, d.age_min
                    ),
                });
            }
        } else if d.age_min > LEASE_WARN_MIN {
            alerts.push(Alert {
                severity: Severity::Warn,
                message: format!(
                    "lease {} by={} aging age_min={}",
                    d.id, d.claimed_by, d.age_min
                ),
            });
        }
    }
}

fn log_quiet_on_lease(snap: &Snapshot, alerts: &mut Vec<Alert>) {
    let RegistryView::Ok(board) = &snap.registry else {
        return;
    };
    if board.doing.is_empty() {
        return;
    }
    // Any lease held while the freshest log is quieter than threshold.
    let Some(freshest) = snap.logs.iter().map(|l| l.mtime_age_min).min() else {
        // No logs at all while leases held → stall.
        alerts.push(Alert {
            severity: Severity::Stall,
            message: "lease(s) held but no loop logs under agents/".into(),
        });
        return;
    };
    if freshest > LOG_QUIET_MIN {
        alerts.push(Alert {
            severity: Severity::Stall,
            message: format!(
                "lease(s) held but freshest loop log quiet {freshest}m (>{LOG_QUIET_MIN}m)"
            ),
        });
    }
}

fn proc_dead_with_lease(snap: &Snapshot, alerts: &mut Vec<Alert>) {
    let RegistryView::Ok(board) = &snap.registry else {
        return;
    };
    if board.doing.is_empty() || snap.procs.unavailable {
        return;
    }
    if !any_agent_alive(&snap.procs) {
        alerts.push(Alert {
            severity: Severity::Stall,
            message: "lease(s) held but no claude/codex process live".into(),
        });
    }
}

fn active_no_commit(snap: &Snapshot, alerts: &mut Vec<Alert>) {
    let active =
        any_agent_alive(&snap.procs) || snap.logs.iter().any(|l| l.mtime_age_min <= FRESH_LOG_MIN);
    if !active {
        return;
    }
    let Some(head_age) = snap.head_age_min else {
        return;
    };
    if head_age > NO_COMMIT_ACTIVE_MIN {
        alerts.push(Alert {
            severity: Severity::Stall,
            message: format!(
                "stack active (proc or fresh log) but HEAD age {head_age}m (>{NO_COMMIT_ACTIVE_MIN}m)"
            ),
        });
    }
}

fn claimable_all_idle(snap: &Snapshot, alerts: &mut Vec<Alert>) {
    let RegistryView::Ok(board) = &snap.registry else {
        return;
    };
    if board.claimable_at_epoch <= 0 {
        return;
    }
    let no_leases = board.doing.is_empty();
    let no_proc = snap.procs.unavailable || !any_agent_alive(&snap.procs);
    let no_fresh_log =
        snap.logs.iter().all(|l| l.mtime_age_min > FRESH_LOG_MIN) || snap.logs.is_empty();
    if no_leases && no_proc && no_fresh_log {
        alerts.push(Alert {
            severity: Severity::Stall,
            message: format!(
                "claimable={} at {} but all factory lanes idle (no proc, no fresh log, no leases)",
                board.claimable_at_epoch, snap.epoch_label
            ),
        });
    }
}

fn journal_commit_desync(snap: &Snapshot, alerts: &mut Vec<Alert>) {
    // If newest journal line is dated today and HEAD is older than journal day
    // by more than the desync window in wall-clock terms: use HEAD age vs "now"
    // when journal has a line today and head is stale.
    // Practical rule: newest journal entry's calendar day == today (UTC) AND
    // HEAD age > JOURNAL_COMMIT_DESYNC_MIN AND no commit subject/sha appears
    // after that journal day — simplified: journal non-empty newest is "today"
    // and head_age > 5m while journal was written same day (agents usually
    // commit same iteration).
    let Some(newest) = snap.journal.first() else {
        return;
    };
    let Some(j_when) = newest.when else {
        return;
    };
    let Some(head_age) = snap.head_age_min else {
        return;
    };
    // Only fire when journal day is today and HEAD is older than the desync gap.
    if j_when.date_naive() == snap.now.date_naive() && head_age > JOURNAL_COMMIT_DESYNC_MIN {
        // If newest commit subject is reflected in journal summary, skip.
        let journal_blob = newest.raw.to_ascii_lowercase();
        let commit_linked = snap.git.commits.first().is_some_and(|c| {
            journal_blob.contains(&c.sha.to_ascii_lowercase())
                || (!c.subject.is_empty()
                    && journal_blob
                        .contains(&c.subject.to_ascii_lowercase()[..c.subject.len().min(20)]))
        });
        if !commit_linked && head_age > JOURNAL_COMMIT_DESYNC_MIN {
            // Require stronger signal: head older than 60m while journal is today
            // to avoid constant false STALL on multi-hour work without journal-sha.
            if head_age > 60 {
                alerts.push(Alert {
                    severity: Severity::Stall,
                    message: format!(
                        "journal advanced today (tag={}) but HEAD age {head_age}m with no commit link in newest journal line",
                        newest.tag
                    ),
                });
            }
        }
    }
}

fn epoch_gate_thrash(snap: &Snapshot, alerts: &mut Vec<Alert>) {
    if has_epoch_gate_thrash(&snap.logs) {
        alerts.push(Alert {
            severity: Severity::Warn,
            message: "epoch-gate sleep/red thrash detected in log tails".into(),
        });
    }
}

fn journal_or_commit_mentions(snap: &Snapshot, id: &str, by: &str, _window_min: i64) -> bool {
    let id_l = id.to_ascii_lowercase();
    let by_l = by.to_ascii_lowercase();
    for j in &snap.journal {
        let raw = j.raw.to_ascii_lowercase();
        if raw.contains(&id_l) || raw.contains(&by_l) {
            return true;
        }
    }
    for c in &snap.git.commits {
        // Only recent commits (within stall window roughly via age).
        if c.age_min > LEASE_STALL_MIN {
            continue;
        }
        let sub = c.subject.to_ascii_lowercase();
        if sub.contains(&id_l) || sub.contains(&by_l) {
            return true;
        }
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::board::GoalsView;
    use crate::board::git_journal::{CommitDigest, GitView, JournalDigest};
    use crate::board::logs::LogView;
    use crate::board::procs::ProcView;
    use crate::board::registry::{DoingLease, RegistryBoard, RegistryView};
    use chrono::Utc;
    use std::path::PathBuf;

    fn base_snap() -> Snapshot {
        Snapshot {
            now: Utc::now(),
            repo: PathBuf::from("."),
            epoch_label: "E2".into(),
            branch: "loop/main".into(),
            head_sha: "abcdef0".into(),
            head_age_min: Some(5),
            registry: RegistryView::Ok(RegistryBoard::default()),
            git: GitView {
                branch: "loop/main".into(),
                head_sha: "abcdef0".into(),
                head_age_min: Some(5),
                commits: vec![CommitDigest {
                    sha: "abcdef0".into(),
                    subject: "wip".into(),
                    rel_age: "5m".into(),
                    age_min: 5,
                    when: Utc::now(),
                }],
                error: None,
            },
            journal: vec![],
            goals: GoalsView::default(),
            logs: vec![],
            procs: ProcView::default(),
            alerts: vec![],
        }
    }

    #[test]
    fn lease_aging_warn() {
        let mut snap = base_snap();
        if let RegistryView::Ok(b) = &mut snap.registry {
            b.doing.push(DoingLease {
                id: "br".into(),
                coverage_phase: "sampled".into(),
                claimed_by: "lane-1".into(),
                claimed_at: Utc::now(),
                age_min: 45,
            });
        }
        let alerts = evaluate(&snap);
        assert!(
            alerts
                .iter()
                .any(|a| a.severity == Severity::Warn && a.message.contains("aging")),
            "{alerts:?}"
        );
    }

    #[test]
    fn lease_stall_no_signal() {
        let mut snap = base_snap();
        if let RegistryView::Ok(b) = &mut snap.registry {
            b.doing.push(DoingLease {
                id: "br".into(),
                coverage_phase: "sampled".into(),
                claimed_by: "lane-1".into(),
                claimed_at: Utc::now(),
                age_min: 90,
            });
        }
        // Fresh log so log-quiet doesn't dominate; empty journal/commits re id.
        snap.logs.push(LogView {
            name: "loop.lane-1.log".into(),
            path: PathBuf::from("x"),
            stack: "claude",
            mtime_rel: "1m".into(),
            mtime_age_min: 1,
            signals: vec!["idle".into()],
        });
        snap.procs.claude = 1;
        let alerts = evaluate(&snap);
        assert!(
            alerts
                .iter()
                .any(|a| a.severity == Severity::Stall && a.message.contains("no journal/commit")),
            "{alerts:?}"
        );
    }

    #[test]
    fn proc_dead_with_lease_stalls() {
        let mut snap = base_snap();
        if let RegistryView::Ok(b) = &mut snap.registry {
            b.doing.push(DoingLease {
                id: "br".into(),
                coverage_phase: "sampled".into(),
                claimed_by: "lane-1".into(),
                claimed_at: Utc::now(),
                age_min: 5,
            });
        }
        snap.logs.push(LogView {
            name: "loop.lane-1.log".into(),
            path: PathBuf::from("x"),
            stack: "claude",
            mtime_rel: "1m".into(),
            mtime_age_min: 1,
            signals: vec![],
        });
        snap.procs = ProcView {
            unavailable: false,
            claude: 0,
            codex: 0,
            run_loop_ish: 0,
            samples: vec![],
        };
        let alerts = evaluate(&snap);
        assert!(
            alerts
                .iter()
                .any(|a| a.message.contains("no claude/codex process")),
            "{alerts:?}"
        );
    }

    #[test]
    fn claimable_idle_stalls() {
        let mut snap = base_snap();
        if let RegistryView::Ok(b) = &mut snap.registry {
            b.claimable_at_epoch = 3;
            b.doing.clear();
        }
        snap.procs = ProcView::default();
        snap.logs.clear();
        let alerts = evaluate(&snap);
        assert!(
            alerts
                .iter()
                .any(|a| a.message.contains("claimable") && a.message.contains("idle")),
            "{alerts:?}"
        );
    }

    #[test]
    fn active_no_commit_stalls() {
        let mut snap = base_snap();
        snap.head_age_min = Some(90);
        snap.git.head_age_min = Some(90);
        snap.procs.claude = 1;
        let alerts = evaluate(&snap);
        assert!(
            alerts
                .iter()
                .any(|a| a.message.contains("HEAD age") && a.severity == Severity::Stall),
            "{alerts:?}"
        );
    }

    #[test]
    fn log_quiet_with_lease() {
        let mut snap = base_snap();
        if let RegistryView::Ok(b) = &mut snap.registry {
            b.doing.push(DoingLease {
                id: "br".into(),
                coverage_phase: "sampled".into(),
                claimed_by: "lane-1".into(),
                claimed_at: Utc::now(),
                age_min: 5,
            });
        }
        snap.logs.push(LogView {
            name: "loop.lane-1.log".into(),
            path: PathBuf::from("x"),
            stack: "claude",
            mtime_rel: "20m".into(),
            mtime_age_min: 20,
            signals: vec![],
        });
        snap.procs.claude = 1;
        let alerts = evaluate(&snap);
        assert!(
            alerts.iter().any(|a| a.message.contains("quiet")),
            "{alerts:?}"
        );
    }

    #[test]
    fn epoch_gate_warn() {
        let mut snap = base_snap();
        snap.logs.push(LogView {
            name: "loop.lane-2.log".into(),
            path: PathBuf::from("x"),
            stack: "claude",
            mtime_rel: "1m".into(),
            mtime_age_min: 1,
            signals: vec!["epoch gate E2 NOT GREEN — sleeping 3600s".into()],
        });
        let alerts = evaluate(&snap);
        assert!(
            alerts
                .iter()
                .any(|a| a.severity == Severity::Warn && a.message.contains("epoch-gate")),
            "{alerts:?}"
        );
    }

    #[test]
    fn none_when_quiet_idle_empty() {
        let snap = base_snap();
        let alerts = evaluate(&snap);
        assert!(alerts.is_empty(), "{alerts:?}");
    }

    #[test]
    fn journal_desync_uses_today() {
        let mut snap = base_snap();
        snap.head_age_min = Some(120);
        snap.git.head_age_min = Some(120);
        snap.journal.push(JournalDigest {
            rel_age: "now".into(),
            tag: "105".into(),
            summary: "did stuff without sha".into(),
            when: Some(snap.now),
            raw: "2026-07-11 | 105 | did stuff without sha | note".into(),
        });
        // No proc/fresh log → active_no_commit may not fire; desync should.
        let alerts = evaluate(&snap);
        assert!(
            alerts
                .iter()
                .any(|a| a.message.contains("journal advanced today")),
            "{alerts:?}"
        );
    }
}
