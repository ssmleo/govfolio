//! EU Parliament DPI + France HATVP DIA + Germany Bundestag — annual private-interest
//! declarations. ONE adapter crate (`eu_fr_de_annual`), THREE source sub-adapters
//! (`eu`/`fr`/`de`) and THREE `disclosure_regime` rows (`eu_parliament_dpi`,
//! `fr_hatvp_dia`, `de_bundestag`); conformance dispatches by the single adapter name
//! over source-namespaced fixtures `fixtures/{eu,fr,de}_<case>/`. See the authoritative
//! methodology `docs/regimes/eu_fr_de_annual.md` and the fixture conventions +
//! per-source builder notes in `crates/adapters/eu_fr_de_annual/fixtures/MANIFEST.json`.
//!
//! SCAFFOLD ONLY (goal 064 leg B, test-designer): this crate exists so the
//! `crates/adapters/*` workspace glob stays valid and `cargo test --workspace` is green
//! before the build leg (the `us_house` CI-red lesson, commit `c6dceb7`). It carries no
//! adapter logic. The build leg (`064c`, rust-builder) replaces this file with the real
//! `Adapter` impl (`Source` enum + dispatch), the `src/{eu,fr,de}/*` sub-modules, the
//! three snapshot-committed `crates/pipeline/schemas/details/*.interest.json` schema
//! files + their three `conformance.rs` `details_schema()` arms, and a `conformance_entry`
//! bin. Until then the harness fails closed (no details schema registered) — do NOT hack
//! the harness to go green.
