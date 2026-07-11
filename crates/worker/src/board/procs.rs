//! Best-effort process snapshot (Windows tasklist / Unix ps).

use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct ProcView {
    pub unavailable: bool,
    pub claude: usize,
    pub codex: usize,
    pub run_loop_ish: usize,
    pub samples: Vec<String>,
}

pub fn collect() -> ProcView {
    #[cfg(windows)]
    {
        collect_windows()
    }
    #[cfg(not(windows))]
    {
        collect_unix()
    }
}

#[cfg(windows)]
fn collect_windows() -> ProcView {
    // CSV: "Image Name","PID","Session Name","Session#","Mem Usage"
    let out = match Command::new("tasklist")
        .args(["/FO", "CSV", "/NH"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => {
            return ProcView {
                unavailable: true,
                ..ProcView::default()
            };
        }
    };
    let mut view = ProcView::default();
    for line in out.lines() {
        let name = csv_first_field(line).to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        classify(&mut view, &name);
    }
    view
}

#[cfg(not(windows))]
fn collect_unix() -> ProcView {
    let out = match Command::new("ps").args(["-A", "-o", "comm="]).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => {
            // Fallback: try ps -ax -o comm=
            match Command::new("ps").args(["-ax", "-o", "comm="]).output() {
                Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
                _ => {
                    return ProcView {
                        unavailable: true,
                        ..ProcView::default()
                    };
                }
            }
        }
    };
    let mut view = ProcView::default();
    for line in out.lines() {
        let name = line.trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        // Strip path: /usr/bin/claude → claude
        let base = name.rsplit('/').next().unwrap_or(&name);
        classify(&mut view, base);
    }
    view
}

fn classify(view: &mut ProcView, name: &str) {
    let base = name.trim_matches('"');
    if base == "claude" || base == "claude.exe" {
        view.claude += 1;
        push_sample(view, base);
    } else if base == "codex" || base == "codex.exe" {
        view.codex += 1;
        push_sample(view, base);
    } else if base.contains("run-loop") {
        view.run_loop_ish += 1;
        push_sample(view, base);
    }
}

fn push_sample(view: &mut ProcView, name: &str) {
    if view.samples.len() < 6 && !view.samples.iter().any(|s| s == name) {
        view.samples.push(name.to_owned());
    }
}

#[cfg(windows)]
fn csv_first_field(line: &str) -> String {
    // "Image Name","PID",...
    let line = line.trim();
    if let Some(rest) = line.strip_prefix('"')
        && let Some(end) = rest.find('"')
    {
        return rest[..end].to_owned();
    }
    line.split(',').next().unwrap_or("").trim().to_owned()
}

/// True when at least one agent process for the dual stack is alive.
pub fn any_agent_alive(p: &ProcView) -> bool {
    !p.unavailable && (p.claude > 0 || p.codex > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_counts() {
        let mut v = ProcView::default();
        classify(&mut v, "claude.exe");
        classify(&mut v, "claude.exe");
        classify(&mut v, "codex");
        assert_eq!(v.claude, 2);
        assert_eq!(v.codex, 1);
        assert!(any_agent_alive(&v));
    }
}
