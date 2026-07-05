//! US Senate eFD PTR adapter (regime code `us_senate`, goal 060).
//!
//! Scope: Periodic Transaction Reports only (eFD report type `11`), electronic
//! (`/search/view/ptr/`) on the green path; paper filings route to the LLM
//! seam stub. The canonical methodology lives in `docs/regimes/us_senate.md` —
//! §2 discovery + session dance + §2.5 client-fingerprint constraint, §3
//! mapping tables, §4 Silver shape, §5 details contract, §6 extraction
//! strategy. Fixtures + conformance conventions:
//! `crates/adapters/us_senate/fixtures/MANIFEST.json`.

pub mod adapter;
pub mod details;
pub mod extractor;
pub(crate) mod normalize;
pub(crate) mod parse;
pub(crate) mod tables;

pub use adapter::UsSenateAdapter;
