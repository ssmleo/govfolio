//! Australia — House of Representatives Register of Members' Interests adapter
//! (regime code `australia_register`, goal 063). govfolio's FIRST
//! LLM-vision-first regime: the Bronze document is a scanned, often-handwritten,
//! multi-column paper form (regime doc §3.9), so every document routes through
//! the goal-021 extraction seam (offline cache for conformance/e2e).
//!
//! Two record types coexist in ONE compound per-member document: the initial
//! Statement of Registrable Interests (`record_type = interest`, §3.5) and the
//! appended Notification(s) of Alteration (`record_type = change_notification`,
//! §3.5). No record carries a monetary value (`value` NULL always, §3.6) — the
//! register is descriptive. The Senate register is a separate regime (§2.6).
//!
//! The authoritative methodology is `docs/regimes/australia_register.md`: §2
//! discovery + the Azure-WAF browser-engine fetch seam, §3 document anatomy,
//! §4 Silver `StagingRow`, §5 the two `details` contracts, §6 the extraction
//! strategy + confidence. Fixtures + conformance conventions (ULID constants,
//! confidence literals, `entry_fields`/`category_name` normalization, DD/MM
//! day-first dates, owner map, cache priming) live in
//! `crates/adapters/australia_register/fixtures/MANIFEST.json`.

pub mod adapter;
pub mod details;
pub(crate) mod extractor;
pub(crate) mod normalize;

pub use adapter::AustraliaRegisterAdapter;
