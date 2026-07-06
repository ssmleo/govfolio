//! Conformance identity binding (design §5.1; fixtures `MANIFEST.json`
//! `conformance_ids`). Conformance runs carry `pool: None` so filing/politician/
//! regime ids cannot resolve from a roster — the sub-adapters emit the fixed
//! Crockford ULID constants the fixtures pin. Pool-backed (production) runs emit
//! nil (unbound) ULIDs for the publish stage to bind from Postgres (the
//! `australia_register`/`canada_ciec` precedent; runner binding is a follow-up).

use pipeline::adapter::RunCtx;

use govfolio_core::ids::{FilingId, PoliticianId, RegimeId};

/// Nil ULID: the "identity not yet bound" marker for pool-backed runs.
pub(crate) const UNBOUND_ID: &str = "00000000000000000000000000";

/// How identity ids are resolved for this run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdentityMode {
    /// Offline conformance: emit the fixed MANIFEST ULIDs.
    Conformance,
    /// Pool-backed run: emit nil ULIDs for the publish stage to bind.
    Unbound,
}

impl IdentityMode {
    pub(crate) fn of(ctx: &RunCtx) -> Self {
        if ctx.pool.is_some() {
            Self::Unbound
        } else {
            Self::Conformance
        }
    }
}

/// Parses a ULID-newtype constant, wrapping the decode error.
fn ulid<T: std::str::FromStr>(s: &str) -> anyhow::Result<T>
where
    T::Err: std::fmt::Display,
{
    s.parse()
        .map_err(|e: T::Err| anyhow::anyhow!("ulid {s:?}: {e}"))
}

/// Resolves the three identity ULIDs: the pinned constants in conformance, nil
/// (unbound) with a pool.
pub(crate) fn resolve(
    mode: IdentityMode,
    filing: &str,
    politician: &str,
    regime: &str,
) -> anyhow::Result<(FilingId, PoliticianId, RegimeId)> {
    match mode {
        IdentityMode::Conformance => Ok((ulid(filing)?, ulid(politician)?, ulid(regime)?)),
        IdentityMode::Unbound => Ok((ulid(UNBOUND_ID)?, ulid(UNBOUND_ID)?, ulid(UNBOUND_ID)?)),
    }
}
