//! Placeholder crate for the `br` (Brazil) jurisdiction adapter.
//!
//! Coverage-factory phase: SPECCED (fixtures + capture manifest under
//! `fixtures/` — see `MANIFEST.json`; Phase 3 spec-writer artifacts are
//! `plan.md`, at the crate root, and `details.rs`, below). `plan.md` carries
//! the field-mapping table, parse strategy, politeness config, edge-case
//! list, and open items (notably a blocking one: `Currency::BRL` does not
//! yet exist in `govfolio_core::domain::enums::Currency`). No adapter logic
//! (`discover`/`fetch`/`parse`/`normalize`, i.e. the `JurisdictionAdapter`
//! impl) lives here yet — that is Phase 4 (rust-builder) work, see
//! `agents/workflows/source-exploration.md`.

pub mod details;
