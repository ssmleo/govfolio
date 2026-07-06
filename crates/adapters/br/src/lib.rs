//! Brazil (`br`) TSE candidacy-time asset-declaration adapter (regime code
//! `br`, coverage-factory epoch E2 — first `record_type: holding` regime,
//! first non-English source). Scope: `declaração de bens` itemized asset
//! line items for `DEPUTADO FEDERAL`/`SENADOR` (+ suplentes) candidacies.
//! The canonical methodology lives in `docs/regimes/br/AUTHORITY.md` (survey)
//! and `plan.md`, at the crate root (field-mapping table, parse strategy,
//! `CD_TIPO_BEM_CANDIDATO -> AssetClass` table, politeness config, edge
//! cases). Fixtures + conformance conventions:
//! `crates/adapters/br/fixtures/MANIFEST.json`.

pub mod adapter;
pub mod binding;
pub mod details;
pub(crate) mod normalize;
pub(crate) mod parse;
pub(crate) mod tables;

pub use adapter::BrAdapter;
