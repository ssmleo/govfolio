//! govfolio worker: consumers and offline bins. The alert dispatcher
//! (goal 030, design §6.3) lives in [`alerts`]; usage -> Stripe metering
//! (goal 050) in [`billing`] over the [`stripe`] seam; the bulk export in
//! [`snapshot`]; continuous drift defense (goal 017, design §5.6/§5.8) in
//! [`sentinel`]; the monthly sampling audit + precision report (goal 070,
//! design §7.4) in [`sampler`]; the local pipeline runner is the `local` bin;
//! the US archive backfill dry-run + diff report (goal 080, design §5.6) in
//! [`backfill`] (the `backfill` bin); the local-only-backfill -> prod copy
//! (per founder-directed policy, 2026-07-09 session direction — pending
//! write-back into a future root `CLAUDE.md` invariant) in
//! [`migrate_local_to_prod`] (the `migrate-local-to-prod` bin); the atomic
//! jurisdiction lease for parallel loop lanes (goal 097, parallel-factory
//! pre-check 1) in [`lease`] (the `jurisdiction-lease` bin); the read-only
//! factory/loop dashboard in [`board`] (the `loop-board` bin, drives
//! `agents/monitor.sh`).

pub mod alerts;
pub mod backfill;
pub mod billing;
pub mod board;
pub mod lease;
pub mod migrate_local_to_prod;
pub mod sampler;
pub mod sentinel;
pub mod snapshot;
pub mod stripe;
