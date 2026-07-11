//! Dual-stack loop log discovery + semantic tails.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use crate::board::{rel_age, truncate};

#[derive(Debug, Clone)]
pub struct LogView {
    pub name: String,
    pub path: PathBuf,
    pub stack: &'static str,
    pub mtime_rel: String,
    pub mtime_age_min: i64,
    pub signals: Vec<String>,
}

/// High-signal substrings. Prefer word-ish forms so git `warning:` noise
/// does not dominate tails (substring `warn` alone is too loose).
const SIGNAL_MARKERS: &[&str] = &[
    "outcome",
    "claim",
    "stop",
    "iteration",
    "epoch gate",
    "no claimable",
    "error:",
    "warn:",
    " stall",
    "halt",
];

pub fn collect(repo: &Path, now: DateTime<Utc>) -> Vec<LogView> {
    let agents = repo.join("agents");
    let mut out = Vec::new();

    push_if_exists(&mut out, &agents, "loop.log", "claude", now);
    push_glob_lanes(&mut out, &agents, "loop.lane-", ".log", "claude", now);
    push_if_exists(&mut out, &agents, "codex-loop.log", "codex", now);
    push_glob_lanes(&mut out, &agents, "codex-loop.lane-", ".log", "codex", now);

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn push_if_exists(
    out: &mut Vec<LogView>,
    agents: &Path,
    name: &str,
    stack: &'static str,
    now: DateTime<Utc>,
) {
    let path = agents.join(name);
    if path.is_file() {
        out.push(read_log(name, path, stack, now));
    }
}

fn push_glob_lanes(
    out: &mut Vec<LogView>,
    agents: &Path,
    prefix: &str,
    suffix: &str,
    stack: &'static str,
    now: DateTime<Utc>,
) {
    let Ok(entries) = std::fs::read_dir(agents) else {
        return;
    };
    for ent in entries.flatten() {
        let name = ent.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with(prefix) && name.ends_with(suffix) && ent.path().is_file() {
            out.push(read_log(name, ent.path(), stack, now));
        }
    }
}

fn read_log(name: &str, path: PathBuf, stack: &'static str, now: DateTime<Utc>) -> LogView {
    let (mtime_age_min, mtime_rel) = file_age_min(&path, now);
    let signals = semantic_tail(&path);
    LogView {
        name: name.to_owned(),
        path,
        stack,
        mtime_rel,
        mtime_age_min,
        signals,
    }
}

fn file_age_min(path: &Path, now: DateTime<Utc>) -> (i64, String) {
    let Ok(meta) = std::fs::metadata(path) else {
        return (i64::MAX / 4, "?".into());
    };
    let Ok(modified) = meta.modified() else {
        return (i64::MAX / 4, "?".into());
    };
    let modified: DateTime<Utc> = modified.into();
    let age = (now - modified).num_minutes().max(0);
    (age, rel_age(age))
}

/// Last ~80 lines → up to 2 high-signal lines (newest first order preserved as file order: newest last).
pub fn semantic_tail(path: &Path) -> Vec<String> {
    let Ok(body) = std::fs::read_to_string(path) else {
        return vec!["(unreadable)".into()];
    };
    // Cap huge files: only keep last 256 KiB for scanning.
    let slice = if body.len() > 256 * 1024 {
        &body[body.len() - 256 * 1024..]
    } else {
        &body
    };
    let lines: Vec<&str> = slice.lines().collect();
    let start = lines.len().saturating_sub(80);
    let window = &lines[start..];

    let mut matched: Vec<String> = Vec::new();
    for line in window.iter().rev() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        let low = t.to_ascii_lowercase();
        if SIGNAL_MARKERS.iter().any(|m| low.contains(m)) {
            matched.push(truncate(t, 120));
            if matched.len() >= 2 {
                break;
            }
        }
    }
    if matched.is_empty() {
        // Fallback: last non-empty line.
        for line in window.iter().rev() {
            let t = line.trim();
            if !t.is_empty() {
                matched.push(truncate(t, 120));
                break;
            }
        }
    }
    matched.reverse(); // chronological within the pair
    matched
}

/// True when any log text mentions epoch-gate sleep thrash.
pub fn has_epoch_gate_thrash(logs: &[LogView]) -> bool {
    logs.iter().any(|l| {
        l.signals.iter().any(|s| {
            let low = s.to_ascii_lowercase();
            low.contains("epoch gate")
                && (low.contains("not green") || low.contains("sleeping") || low.contains("red"))
        })
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn semantic_prefers_markers() {
        let dir = std::env::temp_dir().join(format!("govfolio-board-logs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.log");
        std::fs::write(
            &path,
            "noise line one\n\
             more noise\n\
             Outcome: lane idle, nothing claimable\n\
             trailing chatter\n\
             Claim step returned none\n",
        )
        .unwrap();
        let sigs = semantic_tail(&path);
        assert_eq!(sigs.len(), 2);
        assert!(sigs[0].to_ascii_lowercase().contains("outcome"));
        assert!(sigs[1].to_ascii_lowercase().contains("claim"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn semantic_skips_git_warning_noise() {
        let dir = std::env::temp_dir().join(format!("govfolio-board-logs3-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.log");
        std::fs::write(
            &path,
            "warning: unable to access git ignore\n\
             Outcome: idle\n",
        )
        .unwrap();
        let sigs = semantic_tail(&path);
        assert_eq!(sigs.len(), 1);
        assert!(sigs[0].to_ascii_lowercase().contains("outcome"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn semantic_fallback_last_line() {
        let dir = std::env::temp_dir().join(format!("govfolio-board-logs2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.log");
        std::fs::write(&path, "only boring chatter here\n").unwrap();
        let sigs = semantic_tail(&path);
        assert_eq!(sigs.len(), 1);
        assert!(sigs[0].contains("boring"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
