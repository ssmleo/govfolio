//! UK House of Commons Register of Members' Financial Interests adapter
//! (regime code `uk_commons_register`, goal 061).
//!
//! Scope: categorical interests via the official Register of Interests API
//! (interests-api.parliament.uk) — govfolio's first `interest` record-type
//! adapter and first pure-JSON-API source (no scraping, no LLM on the green
//! path). The canonical methodology lives in
//! `docs/regimes/uk_commons_register.md` — §2 discovery/pagination/version
//! keys, §3 record anatomy + value/owner/date rules, §4 Silver shape, §5
//! details contract, §6 extraction strategy. Fixtures + conformance
//! conventions: `crates/adapters/uk_commons_register/fixtures/MANIFEST.json`.

pub mod adapter;
pub mod details;
pub(crate) mod fields;
pub(crate) mod normalize;
pub(crate) mod parse;

pub use adapter::UkCommonsRegisterAdapter;
