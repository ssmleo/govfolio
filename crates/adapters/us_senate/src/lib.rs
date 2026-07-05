//! US Senate eFD PTR adapter (regime code `us_senate`, goal 060).
//!
//! SCAFFOLD ONLY: the fixtures + independently-authored expected outputs land
//! first (goal 060 leg B, test-designer); the adapter itself is the builder
//! leg. This empty crate exists because the workspace member glob
//! `crates/adapters/*` treats every subdirectory as a package — a fixtures
//! directory without a `Cargo.toml` breaks the whole workspace build
//! (`us_house` T8b lesson).
//!
//! The canonical methodology lives in `docs/regimes/us_senate.md` — §2
//! discovery + session dance + §2.5 client-fingerprint constraint, §3 mapping
//! tables, §4 Silver shape, §5 details contract, §6 extraction strategy.
//! Fixtures + conformance conventions:
//! `crates/adapters/us_senate/fixtures/MANIFEST.json`.
