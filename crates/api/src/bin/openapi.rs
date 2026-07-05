//! Emits `packages/contracts/openapi.json` (design §6.1): generated and
//! committed, never hand-edited. CI regenerates and fails on drift:
//! `cargo run -p api --bin openapi && git diff --exit-code packages/contracts/`.

use std::fs;
use std::path::Path;

use anyhow::Context as _;

fn main() -> anyhow::Result<()> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("packages")
        .join("contracts");
    fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join("openapi.json");
    fs::write(&path, api::openapi_json()?)
        .with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}
