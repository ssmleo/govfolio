//! `cargo run -p pipeline --bin capture_fixture -- <adapter> <case> <url> [sha256]`
//! — politely fetches one source document into the adapter's fixture layout:
//! `crates/adapters/<adapter>/fixtures/<case>/input.<ext>` (plan Task 8).
//!
//! Re-running against an existing fixture re-fetches and confirms the sha;
//! any drift is a hard stop, never an overwrite (regime doc §7 — fixtures are
//! test-designer ground truth). An optional expected sha256 pins the download
//! before anything is written.

use std::process::ExitCode;
use std::time::Duration;

use anyhow::Context as _;

use pipeline::adapter::{PoliteClient, PolitenessCfg};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("capture_fixture: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let (Some(adapter), Some(case), Some(url)) = (args.next(), args.next(), args.next()) else {
        anyhow::bail!("usage: capture_fixture <adapter> <case> <url> [expected_sha256]");
    };
    let expected_sha = args.next();
    anyhow::ensure!(args.next().is_none(), "unexpected extra argument");
    for name in [&adapter, &case] {
        anyhow::ensure!(
            !name.is_empty()
                && name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'),
            "adapter/case names must be [a-z0-9_]+, got {name:?}"
        );
    }

    let bytes = download(&url)?;
    let sha256 = sha256_hex(&bytes);
    if let Some(expected) = &expected_sha {
        anyhow::ensure!(
            expected.eq_ignore_ascii_case(&sha256),
            "sha256 mismatch for {url}: expected {expected}, fetched {sha256} — \
             source drift, stopping (regime doc §7)"
        );
    }

    let case_dir = pipeline::conformance::fixtures_dir(&adapter).join(&case);
    let target = case_dir.join(format!("input.{}", extension_of(&url)));
    if let Some(existing) = existing_input(&case_dir)? {
        let committed = std::fs::read(&existing)
            .with_context(|| format!("reading existing fixture {}", existing.display()))?;
        let committed_sha = sha256_hex(&committed);
        anyhow::ensure!(
            committed_sha == sha256,
            "fixture drift: {} has sha256 {committed_sha} but the source now serves {sha256} — \
             stopping for review (fixtures are ground truth)",
            existing.display()
        );
        println!(
            "confirmed {} == fetched bytes (sha256 {sha256})",
            existing.display()
        );
        return Ok(());
    }
    std::fs::create_dir_all(&case_dir)
        .with_context(|| format!("creating {}", case_dir.display()))?;
    std::fs::write(&target, &bytes).with_context(|| format!("writing {}", target.display()))?;
    println!(
        "captured {} ({} bytes, sha256 {sha256})",
        target.display(),
        bytes.len()
    );
    println!("next: author expected.silver.json / expected.gold.json independently of the parser");
    Ok(())
}

/// Polite download (identified UA, min interval, concurrency 1 — invariant 10).
fn download(url: &str) -> anyhow::Result<Vec<u8>> {
    let cfg = PolitenessCfg::new(Duration::from_secs(2), "ssm.leo@outlook.com");
    let client = PoliteClient::new(&cfg)?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("building tokio runtime")?;
    runtime.block_on(async {
        let response = client.get(url).await?;
        anyhow::ensure!(
            response.status().is_success(),
            "GET {url} -> {}",
            response.status()
        );
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("reading body of {url}"))?;
        Ok(bytes.to_vec())
    })
}

/// The existing `input.*` file of a case directory, if any.
fn existing_input(case_dir: &std::path::Path) -> anyhow::Result<Option<std::path::PathBuf>> {
    if !case_dir.is_dir() {
        return Ok(None);
    }
    for entry in std::fs::read_dir(case_dir)
        .with_context(|| format!("reading case {}", case_dir.display()))?
    {
        let path = entry?.path();
        if path.is_file() && path.file_stem().is_some_and(|stem| stem == "input") {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Lowercase file extension from the URL path; `bin` when unrecognizable.
fn extension_of(url: &str) -> String {
    url.split(['?', '#'])
        .next()
        .and_then(|path| path.rsplit('/').next())
        .and_then(|name| name.rsplit_once('.'))
        .map(|(_, ext)| ext.to_ascii_lowercase())
        .filter(|ext| {
            (1..=4).contains(&ext.len()) && ext.bytes().all(|b| b.is_ascii_alphanumeric())
        })
        .unwrap_or_else(|| "bin".to_owned())
}

/// sha256 of the raw bytes as 64 lowercase hex chars (the Bronze address form).
fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}
