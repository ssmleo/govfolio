//! Canada CIEC public-registry adapter (regime code `canada_ciec`, goal 062).
//!
//! Scope: the ten financial-substance declaration types filed by the three
//! politician roles (MPs, Ministers, Parliamentary Secretaries) on the Office
//! of the Conflict of Interest and Ethics Commissioner public registry — one
//! govfolio regime covering both the Conflict of Interest Act and the Members'
//! Code (the instrument is per-record metadata, `details.law`). Two record
//! types: `interest` for asset/liability/activity/summary/gift/travel
//! declarations, `change_notification` for Material Changes. No record ever
//! carries a monetary value (`value` NULL always, by statute — regime doc
//! §3.6).
//!
//! The canonical methodology lives in `docs/regimes/canada_ciec.md` — §2
//! discovery + politeness, §3 record anatomy + grammar families A/B/C, §4
//! Silver shape, §5 the two `details` contracts, §6 extraction strategy +
//! confidence scoring. Fixtures + conformance conventions (the LOAD-BEARING
//! `<br>`→space rule, ULID constants, confidence literals):
//! `crates/adapters/canada_ciec/fixtures/MANIFEST.json`.

pub mod adapter;
pub mod details;
pub(crate) mod normalize;
pub(crate) mod parse;
pub(crate) mod tables;

pub use adapter::CanadaCiecAdapter;
