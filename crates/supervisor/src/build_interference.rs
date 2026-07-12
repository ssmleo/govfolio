use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
#[cfg(windows)]
use std::process::Command;

use anyhow::Context as _;
#[cfg(windows)]
use anyhow::bail;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObservedProcess {
    pub pid: u32,
    pub parent_pid: u32,
    pub name: String,
    pub command_line: String,
}

pub fn observe_processes() -> anyhow::Result<Vec<ObservedProcess>> {
    observe_processes_platform()
}

#[must_use]
pub fn foreign_govfolio_processes(
    processes: &[ObservedProcess],
    repository: &Path,
    worktree: &Path,
    supervisor_pid: u32,
    supervised_roots: &[u32],
    supervised_target_dirs: &[std::path::PathBuf],
) -> Vec<ObservedProcess> {
    let parents = processes
        .iter()
        .map(|process| (process.pid, process.parent_pid))
        .collect::<BTreeMap<_, _>>();
    let mut excluded = BTreeSet::from([supervisor_pid]);
    let mut ancestor = supervisor_pid;
    while let Some(parent) = parents
        .get(&ancestor)
        .copied()
        .filter(|parent| *parent != 0)
    {
        if !excluded.insert(parent) {
            break;
        }
        ancestor = parent;
    }
    for &root in supervised_roots {
        excluded.insert(root);
        for process in processes {
            if is_descendant(process.pid, root, &parents) {
                excluded.insert(process.pid);
            }
        }
    }
    let repository = normalized(repository.to_string_lossy().as_ref());
    let worktree = normalized(worktree.to_string_lossy().as_ref());
    let supervised_targets = supervised_target_dirs
        .iter()
        .map(|target| normalized(target.to_string_lossy().as_ref()))
        .collect::<Vec<_>>();
    let mut foreign = processes
        .iter()
        .filter(|process| !excluded.contains(&process.pid))
        .filter(|process| is_rust_build_process(&process.name))
        .filter(|process| {
            let command = normalized(&process.command_line);
            if !is_cargo_process(&process.name)
                && supervised_targets
                    .iter()
                    .any(|target| command.contains(target))
            {
                return false;
            }
            command.contains(&repository)
                || command.contains(&worktree)
                || is_cargo_process(&process.name)
        })
        .cloned()
        .collect::<Vec<_>>();
    foreign.sort_by_key(|process| process.pid);
    foreign
}

fn is_descendant(pid: u32, root: u32, parents: &BTreeMap<u32, u32>) -> bool {
    let mut seen = BTreeSet::new();
    let mut current = pid;
    while let Some(parent) = parents.get(&current).copied().filter(|parent| *parent != 0) {
        if parent == root {
            return true;
        }
        if !seen.insert(parent) {
            return false;
        }
        current = parent;
    }
    false
}

fn is_rust_build_process(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let stem = name.strip_suffix(".exe").unwrap_or(&name);
    matches!(stem, "cargo" | "rustc")
}

fn is_cargo_process(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.strip_suffix(".exe").unwrap_or(&name) == "cargo"
}

fn normalized(value: &str) -> String {
    let mut value = value.replace('\\', "/").to_ascii_lowercase();
    while value.contains("//") {
        value = value.replace("//", "/");
    }
    value
}

#[cfg(windows)]
fn observe_processes_platform() -> anyhow::Result<Vec<ObservedProcess>> {
    let script = "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,Name,CommandLine | ConvertTo-Json -Compress";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .context("enumerate Windows processes")?;
    if !output.status.success() {
        bail!(
            "Windows process enumeration failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let rows = match value {
        serde_json::Value::Array(rows) => rows,
        serde_json::Value::Null => Vec::new(),
        row => vec![row],
    };
    rows.into_iter()
        .map(|row| {
            let pid = json_u32(&row, "ProcessId")?;
            let parent_pid = json_u32(&row, "ParentProcessId")?;
            Ok(ObservedProcess {
                pid,
                parent_pid,
                name: row
                    .get("Name")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                command_line: row
                    .get("CommandLine")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            })
        })
        .collect()
}

#[cfg(windows)]
fn json_u32(value: &serde_json::Value, key: &str) -> anyhow::Result<u32> {
    value
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .with_context(|| format!("Windows process row has invalid {key}"))
}

#[cfg(unix)]
fn observe_processes_platform() -> anyhow::Result<Vec<ObservedProcess>> {
    let mut rows = Vec::new();
    for entry in std::fs::read_dir("/proc").context("enumerate /proc")? {
        let entry = entry?;
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|name| name.parse::<u32>().ok())
        else {
            continue;
        };
        let status = std::fs::read_to_string(entry.path().join("status")).unwrap_or_default();
        let name = status
            .lines()
            .find_map(|line| line.strip_prefix("Name:\t"))
            .unwrap_or_default()
            .to_owned();
        let parent_pid = status
            .lines()
            .find_map(|line| line.strip_prefix("PPid:\t"))
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(0);
        let command_line = std::fs::read(entry.path().join("cmdline"))
            .map(|bytes| {
                String::from_utf8_lossy(&bytes)
                    .replace('\0', " ")
                    .trim()
                    .to_owned()
            })
            .unwrap_or_default();
        rows.push(ObservedProcess {
            pid,
            parent_pid,
            name,
            command_line,
        });
    }
    Ok(rows)
}
