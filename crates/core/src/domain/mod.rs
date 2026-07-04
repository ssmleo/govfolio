//! Domain layer: canonical Gold-side types. Mirrors design §4.2 (`disclosure_record`) —
//! "one rule, two enforcers": every SQL CHECK has a Rust twin here.

pub mod enums;
pub mod gold;
pub mod value;

use rust_decimal::Decimal;

use crate::domain::enums::RecordType;

/// Violations of `disclosure_record` integrity rules (design §4.2) — the Rust
/// twin of the SQL CHECK constraints, so bad candidates die before Postgres.
#[derive(Debug, thiserror::Error)]
pub enum DomainError {
    /// Per-type CHECK: this `record_type` requires `field` to be present.
    #[error("record_type `{record_type:?}` requires `{field}`")]
    TypeRequires {
        /// The record type whose rule was violated.
        record_type: RecordType,
        /// The column the CHECK requires for that type.
        field: &'static str,
    },
    /// CHECK: `value_high >= value_low`.
    #[error("value interval inverted: high {high} < low {low}")]
    InvertedInterval {
        /// Lower bound as given.
        low: Decimal,
        /// Upper bound as given (below `low`).
        high: Decimal,
    },
}
