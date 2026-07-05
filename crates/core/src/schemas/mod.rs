//! JSON Schema contract home (invariant 5: contract-typed, snapshot-committed).
//! Generated snapshots live at `crates/core/schemas/*.json` (crate root, not `src/`)
//! and are guarded by `tests/schema_snapshot.rs` — contract drift must show in git.

use crate::domain::gold::GoldCandidate;
use crate::query::RecordFilter;

/// JSON Schema for the Gold-layer candidate contract.
#[must_use]
pub fn gold_candidate() -> schemars::Schema {
    schemars::schema_for!(GoldCandidate)
}

/// JSON Schema for the shared record filter grammar — the `alert_rule.filter`
/// contract (design §6.3: one grammar for `/records`, the UI and alerts).
#[must_use]
pub fn record_filter() -> schemars::Schema {
    schemars::schema_for!(RecordFilter)
}
