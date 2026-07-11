//! Goal queue + BLOCKED(human) surface (filesystem only).

use std::path::Path;

use crate::board::truncate;

#[derive(Debug, Clone, Default)]
pub struct GoalsView {
    pub open: Vec<OpenGoal>,
    pub blocked_human: Vec<BlockedHit>,
}

#[derive(Debug, Clone)]
pub struct OpenGoal {
    /// ` ` or `~` from the checkbox.
    pub mark: char,
    pub line: String,
}

#[derive(Debug, Clone)]
pub struct BlockedHit {
    pub goal: String,
    pub snippet: String,
}

pub fn collect(repo: &Path) -> GoalsView {
    GoalsView {
        open: collect_open(repo),
        blocked_human: collect_blocked(repo),
    }
}

fn collect_open(repo: &Path) -> Vec<OpenGoal> {
    let path = repo.join("agents/goals/000-INDEX.md");
    let Ok(body) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for line in body.lines() {
        let trimmed = line.trim_start();
        let mark = if trimmed.starts_with("- [ ]") {
            ' '
        } else if trimmed.starts_with("- [~]") {
            '~'
        } else {
            continue;
        };
        // Strip the checkbox prefix for display.
        let rest = trimmed
            .trim_start_matches("- [ ]")
            .trim_start_matches("- [~]")
            .trim();
        out.push(OpenGoal {
            mark,
            line: truncate(rest, 120),
        });
        if out.len() >= 8 {
            break;
        }
    }
    out
}

fn collect_blocked(repo: &Path) -> Vec<BlockedHit> {
    let dir = repo.join("agents/goals");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for ent in entries.flatten() {
        let path = ent.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_owned();
        if name == "000-INDEX.md" || name == "_TEMPLATE.md" {
            continue;
        }
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        if let Some(hit) = find_blocked_section(&name, &body) {
            hits.push(hit);
        }
        if hits.len() >= 12 {
            break;
        }
    }
    hits.sort_by(|a, b| a.goal.cmp(&b.goal));
    hits
}

fn find_blocked_section(filename: &str, body: &str) -> Option<BlockedHit> {
    let mut lines = body.lines().peekable();
    while let Some(line) = lines.next() {
        if !line.contains("BLOCKED (human)") && !line.contains("BLOCKED(human)") {
            continue;
        }
        // Prefer the next non-empty content line as snippet.
        let mut snippet = line.trim().to_owned();
        for next in lines.by_ref().take(6) {
            let t = next.trim();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            t.clone_into(&mut snippet);
            break;
        }
        // Skip historical/empty/superseded gates — only surface live asks.
        if is_inert_blocked_snippet(&snippet) {
            return None;
        }
        let goal = goal_id_from_filename(filename);
        return Some(BlockedHit {
            goal,
            snippet: truncate(&snippet, 100),
        });
    }
    None
}

fn is_inert_blocked_snippet(snippet: &str) -> bool {
    let t = snippet.trim();
    if t.is_empty() || t == "(empty)" {
        return true;
    }
    let low = t.to_ascii_lowercase();
    // Struck-through or explicitly none/superseded.
    if t.contains("~~") || low.contains("superseded") {
        return true;
    }
    if low.starts_with("(none") || low.starts_with("none ") || low.starts_with("none—") {
        return true;
    }
    if low.contains("full autonomy") || low.contains("nothing blocks") {
        return true;
    }
    false
}

fn goal_id_from_filename(name: &str) -> String {
    // `105-codex-parallel-loop-discovery.md` → `105`
    name.split('-')
        .next()
        .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
        .map_or_else(|| name.trim_end_matches(".md").to_owned(), str::to_owned)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn goal_id_parse() {
        assert_eq!(goal_id_from_filename("105-codex-foo.md"), "105");
        assert_eq!(goal_id_from_filename("_TEMPLATE.md"), "_TEMPLATE");
    }

    #[test]
    fn blocked_section_finds_body() {
        let body = "# x\n\n## BLOCKED (human)\n\nNeed founder ADC login.\n\n## Next\n";
        let hit = find_blocked_section("020-cloud.md", body).expect("hit");
        assert_eq!(hit.goal, "020");
        assert!(hit.snippet.contains("founder ADC"));
    }

    #[test]
    fn blocked_empty_skipped() {
        let body = "## BLOCKED (human)\n\n(empty)\n";
        assert!(find_blocked_section("080-x.md", body).is_none());
    }

    #[test]
    fn blocked_superseded_skipped() {
        let body = "## BLOCKED (human)\n\n- ~~expected.*.json~~ SUPERSEDED by policy\n";
        assert!(find_blocked_section("060-x.md", body).is_none());
    }

    #[test]
    fn open_goals_from_index_snippet() {
        let dir = tempfile_dir();
        let goals = dir.join("agents/goals");
        std::fs::create_dir_all(&goals).unwrap();
        std::fs::write(
            goals.join("000-INDEX.md"),
            "# idx\n- [x] 001 done\n- [ ] 104 idle backoff\n- [~] 105 codex\n- [ ] 106 open epoch\n",
        )
        .unwrap();
        let open = collect_open(&dir);
        assert_eq!(open.len(), 3);
        assert_eq!(open[0].mark, ' ');
        assert!(open[0].line.contains("104"));
        assert_eq!(open[1].mark, '~');
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("govfolio-board-goals-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }
}
