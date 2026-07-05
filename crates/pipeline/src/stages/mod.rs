//! Stage machinery for the in-process runner (design §5.2): `pipeline_run`
//! idempotency bookkeeping, Bronze→`raw_document` ingestion, regime/roster
//! seeding + politician resolution (§5.4), and the transactional publish
//! stage (Gold + outbox + review tasks, one transaction).

pub mod ingest;
pub mod pipeline_run;
pub mod publish;
pub mod roster;
pub mod seed;
