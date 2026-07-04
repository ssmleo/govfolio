//! US House PTR adapter (regime code `us_house`, goal 001 Task 8).
//!
//! Scope: Periodic Transaction Reports only (`FilingType == "P"`). The
//! canonical methodology lives in `docs/regimes/us-house.md` — §2 discovery,
//! §3 mapping tables, §4 Silver shape, §5 details contract, §6 extraction
//! strategy. Fixtures + conformance conventions:
//! `crates/adapters/us_house/fixtures/MANIFEST.json`.

pub mod adapter;
pub mod details;
pub mod extractor;
pub(crate) mod index;
pub(crate) mod normalize;
pub(crate) mod parse;
pub(crate) mod tables;

pub use adapter::UsHouseAdapter;
