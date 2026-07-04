//! JSON Schema contract home (invariant 5: contract-typed, snapshot-committed).
//! Generated snapshots live at `crates/core/schemas/*.json` (crate root, not `src/`)
//! and are guarded by `tests/schema_snapshot.rs` — contract drift must show in git.

use crate::domain::gold::GoldCandidate;

/// JSON Schema for the Gold-layer candidate contract.
#[must_use]
pub fn gold_candidate() -> schemars::Schema {
    schemars::schema_for!(GoldCandidate)
}
