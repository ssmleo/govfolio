//! Canada CIEC public-registry adapter (regime code `canada_ciec`).
//!
//! SCAFFOLD ONLY (goal 062 leg B, test-designer). This crate is an empty, valid
//! workspace member so the `crates/adapters/*` glob stays green while the
//! conformance fixtures + independent expected outputs land ahead of the adapter
//! logic (`us_house` CI-red lesson, commit c6dceb7). The builder leg (062) fills in
//! `adapter`/`parse`/`normalize`/`details`/`seed` per the methodology:
//!
//! - Spec: `docs/regimes/canada_ciec.md` — §2 discovery, §3 record anatomy,
//!   §4 Silver shape, §5 `details` contracts, §6 extraction strategy, §7 fixtures.
//! - Fixtures + conformance conventions:
//!   `crates/adapters/canada_ciec/fixtures/MANIFEST.json`.
