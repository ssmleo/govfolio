//! Ingestion pipeline: the `JurisdictionAdapter` contract (design §5.1), the
//! shared conformance harness every adapter must pass (plan Task 7), and the
//! in-process runner + stage machinery (plan Task 9, design §5.2).

pub mod adapter;
pub mod conformance;
pub mod run;
pub mod stages;
