//! Git status + JOURNAL.md structured digests (read-only, via `git` CLI).

use std::path::Path;
use std::process::Command;

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

#[derive(Debug, Clone, Default)]
pub struct GitView {
    pub branch: String,
    pub head_sha: String,
    pub head_age_min: Option<i64>,
    pub commits: Vec<CommitDigest>,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CommitDigest {
    pub sha: String,
    pub subject: String,
    pub rel_age: String,
    pub age_min: i64,
    pub when: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct JournalDigest {
    pub rel_age: String,
    pub tag: String,
    pub summary: String,
    /// Parsed journal date at midnight UTC when available.
    pub when: Option<DateTime<Utc>>,
    pub raw: String,
}

pub fn collect_git(repo: &Path, now: DateTime<Utc>) -> GitView {
    let mut view = GitView::default();

    match run_git(repo, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        Ok(b) => view.branch = b,
        Err(e) => {
            view.branch = "?".into();
            view.error = Some(e);
            return view;
        }
    }
    match run_git(repo, &["rev-parse", "HEAD"]) {
        Ok(sha) => view.head_sha = sha,
        Err(e) => {
            view.error = Some(e);
            return view;
        }
    }
    if let Ok(ts) = run_git(repo, &["log", "-1", "--format=%ct"])
        && let Ok(secs) = ts.parse::<i64>()
        && let Some(when) = Utc.timestamp_opt(secs, 0).single()
    {
        let age = (now - when).num_minutes().max(0);
        view.head_age_min = Some(age);
    }

    // %h %ct %s — one commit per line
    match run_git(repo, &["log", "-10", "--format=%h%x09%ct%x09%s"]) {
        Ok(body) => {
            for line in body.lines() {
                let mut parts = line.splitn(3, '\t');
                let (Some(sha), Some(ct), Some(subject)) =
                    (parts.next(), parts.next(), parts.next())
                else {
                    continue;
                };
                let Ok(secs) = ct.parse::<i64>() else {
                    continue;
                };
                let Some(when) = Utc.timestamp_opt(secs, 0).single() else {
                    continue;
                };
                let age_min = (now - when).num_minutes().max(0);
                view.commits.push(CommitDigest {
                    sha: sha.to_owned(),
                    subject: truncate(subject, 100),
                    rel_age: rel_age(age_min),
                    age_min,
                    when,
                });
            }
        }
        Err(e) => view.error = Some(e),
    }
    view
}

pub fn collect_journal(repo: &Path, now: DateTime<Utc>) -> Vec<JournalDigest> {
    let path = repo.join("agents/JOURNAL.md");
    let Ok(body) = std::fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut rows: Vec<JournalDigest> = body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter(|l| !l.starts_with('#'))
        .map(|line| parse_journal_line(line, now))
        .collect();
    // Newest last in file → reverse for digest (newest first).
    rows.reverse();
    rows.truncate(8);
    rows
}

/// Parse `YYYY-MM-DD | tag | summary | note` (note optional).
#[must_use]
pub fn parse_journal_line(line: &str, now: DateTime<Utc>) -> JournalDigest {
    let raw = line.to_owned();
    let parts: Vec<&str> = line.splitn(4, " | ").collect();
    if parts.len() < 3 {
        return JournalDigest {
            rel_age: "?".into(),
            tag: "?".into(),
            summary: truncate(line, 100),
            when: None,
            raw,
        };
    }
    let date_s = parts[0].trim();
    let tag = parts[1].trim().to_owned();
    let summary = truncate(parts[2].trim(), 100);
    let when = NaiveDate::parse_from_str(date_s, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|ndt| DateTime::<Utc>::from_naive_utc_and_offset(ndt, Utc));
    let rel = when.map_or_else(|| "?".into(), |w| rel_age((now - w).num_minutes().max(0)));
    JournalDigest {
        rel_age: rel,
        tag,
        summary,
        when,
        raw,
    }
}

fn run_git(repo: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .map_err(|e| format!("git spawn: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git {}: {}", args.join(" "), err.trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Human relative age from whole minutes.
#[must_use]
pub fn rel_age(age_min: i64) -> String {
    if age_min < 1 {
        "now".into()
    } else if age_min < 60 {
        format!("{age_min}m")
    } else if age_min < 60 * 48 {
        format!("{}h", age_min / 60)
    } else {
        format!("{}d", age_min / (60 * 24))
    }
}

#[must_use]
pub fn truncate(s: &str, max: usize) -> String {
    let t = s.trim();
    if t.chars().count() <= max {
        return t.to_owned();
    }
    let mut out: String = t.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn rel_age_buckets() {
        assert_eq!(rel_age(0), "now");
        assert_eq!(rel_age(12), "12m");
        assert_eq!(rel_age(90), "1h");
        assert_eq!(rel_age(60 * 50), "2d");
    }

    #[test]
    fn truncate_ellipsis() {
        assert_eq!(truncate("abc", 10), "abc");
        let long = "x".repeat(50);
        let t = truncate(&long, 10);
        assert_eq!(t.chars().count(), 10);
        assert!(t.ends_with('…'));
    }

    #[test]
    fn journal_parse_happy() {
        let now = Utc.with_ymd_and_hms(2026, 7, 11, 12, 0, 0).unwrap();
        let j = parse_journal_line(
            "2026-07-11 | 105 | Queue saturated — no unclaimed | in-flight",
            now,
        );
        assert_eq!(j.tag, "105");
        assert!(j.summary.starts_with("Queue saturated"));
        assert!(j.when.is_some());
    }

    #[test]
    fn journal_parse_fallback() {
        let now = Utc::now();
        let j = parse_journal_line("not a real journal line at all", now);
        assert_eq!(j.tag, "?");
        assert!(j.when.is_none());
    }
}
