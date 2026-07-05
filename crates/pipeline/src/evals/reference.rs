//! The frozen E1 reference bundle: `docs/regimes/us-house/reference/E1.lock.json`
//! pins every ground-truth artifact by sha256. Verification is tamper-evident:
//! any missing or drifted file is a failure. Freeze = supersede, never mutate —
//! amending a pinned artifact requires superseding the lock (version bump +
//! founder gate; see `docs/decisions/role-eval-thresholds.md`).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Context as _;
use serde::Deserialize;

use crate::factory::sha256_hex;

/// Workspace-relative path of the E1 reference lock.
pub const LOCK_PATH: &str = "docs/regimes/us-house/reference/E1.lock.json";

/// The lock manifest: sha256 pins over the E1 ground-truth artifacts.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceLock {
    /// Lock version; superseding the freeze bumps this.
    pub version: u32,
    /// Epoch the bundle calibrates (`E1`).
    pub epoch: String,
    /// Reference corpus id (`us_house`).
    pub reference: String,
    /// When the bundle was frozen (RFC3339, UTC).
    pub frozen_at_utc: String,
    /// The supersede-never-mutate policy statement.
    pub policy: String,
    /// Workspace-relative path (forward slashes) → sha256 (64 lowercase hex).
    pub pins: BTreeMap<String, String>,
}

/// Loads and parses the lock file.
///
/// # Errors
/// Missing or unparseable lock (fail closed — a frozen bundle that cannot be
/// read cannot vouch for anything).
pub fn load_lock(root: &Path) -> anyhow::Result<ReferenceLock> {
    let path = lock_path(root);
    let text = fs::read_to_string(&path)
        .with_context(|| format!("reading {LOCK_PATH} (reference bundle not frozen?)"))?;
    serde_json::from_str(&text).with_context(|| format!("parsing {LOCK_PATH}"))
}

/// Verifies the frozen bundle: lock present, well-formed, and every pinned
/// file still hashes to its pin. Returns human-readable failures; empty
/// means the freeze holds.
#[must_use]
pub fn verify_lock(root: &Path) -> Vec<String> {
    let mut fails = Vec::new();
    let lock = match load_lock(root) {
        Ok(lock) => lock,
        Err(e) => {
            fails.push(format!("{e:#}"));
            return fails;
        }
    };
    if lock.epoch != "E1" {
        fails.push(format!("lock epoch must be \"E1\", got {:?}", lock.epoch));
    }
    if lock.reference != "us_house" {
        fails.push(format!(
            "lock reference must be \"us_house\", got {:?}",
            lock.reference
        ));
    }
    if lock.version == 0 {
        fails.push("lock version must be >= 1".to_owned());
    }
    if lock.pins.is_empty() {
        fails.push("lock pins are empty — nothing frozen (fail closed)".to_owned());
    }
    for (rel, pin) in &lock.pins {
        if !(pin.len() == 64 && pin.bytes().all(|b| b.is_ascii_hexdigit())) {
            fails.push(format!("{rel}: pin must be 64 hex chars, got {pin:?}"));
            continue;
        }
        let Some(path) = safe_join(root, rel) else {
            fails.push(format!(
                "{rel}: pin path must be workspace-relative with no traversal"
            ));
            continue;
        };
        match fs::read(&path) {
            Ok(bytes) => {
                let actual = sha256_hex(&bytes);
                if !actual.eq_ignore_ascii_case(pin) {
                    fails.push(format!(
                        "{rel}: frozen at {pin} but now hashes to {actual} — reference \
                         artifact drifted; supersede the lock, never mutate the artifact"
                    ));
                }
            }
            Err(e) => fails.push(format!("{rel}: pinned file unreadable: {e} (fail closed)")),
        }
    }
    fails
}

/// `root/<LOCK_PATH>`, segment-joined (Windows-safe).
fn lock_path(root: &Path) -> PathBuf {
    let mut path = root.to_path_buf();
    for segment in LOCK_PATH.split('/') {
        path.push(segment);
    }
    path
}

/// Joins a forward-slash relative pin path under `root`, rejecting absolute
/// paths, drive letters, backslashes, and traversal segments.
fn safe_join(root: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.starts_with('/') || rel.contains('\\') || rel.contains(':') {
        return None;
    }
    let mut path = root.to_path_buf();
    for segment in rel.split('/') {
        if segment.is_empty() || segment == "." || segment == ".." {
            return None;
        }
        path.push(segment);
    }
    Some(path)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn safe_join_rejects_traversal_and_absolute_paths() {
        let root = Path::new("wsroot");
        assert!(safe_join(root, "../etc/passwd").is_none());
        assert!(safe_join(root, "a/../b").is_none());
        assert!(safe_join(root, "/abs").is_none());
        assert!(safe_join(root, "C:/abs").is_none());
        assert!(safe_join(root, "a\\b").is_none());
        assert!(safe_join(root, "").is_none());
        assert!(safe_join(root, "docs/regimes/us-house.md").is_some());
    }

    #[test]
    fn verify_lock_fails_closed_without_a_lock_file() {
        let dir = tempfile::tempdir().unwrap();
        let fails = verify_lock(dir.path());
        assert!(
            fails.iter().any(|f| f.contains("E1.lock.json")),
            "missing lock must fail closed: {fails:?}"
        );
    }

    #[test]
    fn verify_lock_reports_drifted_pins() {
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("frozen.txt");
        fs::write(&artifact, "original bytes").unwrap();
        let lock_dir = dir.path().join("docs/regimes/us-house/reference");
        fs::create_dir_all(&lock_dir).unwrap();
        let lock = serde_json::json!({
            "version": 1,
            "epoch": "E1",
            "reference": "us_house",
            "frozen_at_utc": "2026-07-04T00:00:00Z",
            "policy": "supersede, never mutate",
            "pins": { "frozen.txt": sha256_hex(b"original bytes") }
        });
        fs::write(lock_dir.join("E1.lock.json"), lock.to_string()).unwrap();
        assert_eq!(verify_lock(dir.path()), Vec::<String>::new());
        fs::write(&artifact, "tampered bytes").unwrap();
        let fails = verify_lock(dir.path());
        assert!(
            fails.iter().any(|f| f.contains("drifted")),
            "tampering must be loud: {fails:?}"
        );
    }
}
