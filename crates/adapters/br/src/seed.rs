//! Production seed wiring for the `br` regime: the `disclosure_regime` row
//! (matching `worker::bin::local_br`'s proof constants and
//! `crate::normalize`'s `CONFORMANCE_REGIME_ID`, so conformance mode, the
//! local fixture proof, and this real seed path all agree on one regime
//! row) and per-candidate roster seeding for the real
//! `pipeline::run::Runner` path.
//!
//! ## Why this regime's "roster" is not a member-list seed (read before use)
//!
//! `us_house::seed::seed_historical_rosters` seeds from a SEPARATE, durable
//! authority — the Clerk's own filing-index `Member` list, which exists
//! independently of any one filing and names a small, roughly-fixed body
//! (435 seats). `br` has no equivalent authority: `docs/regimes/br/AUTHORITY.md`
//! is explicit that `SQ_CANDIDATO` is minted fresh every election cycle and
//! there is no Brazil-side stable "roster" the way `us_house` has — a
//! federal general election draws thousands of candidates, most of whom run
//! exactly once. The only identity fields this regime's roster resolution
//! needs (`NM_CANDIDATO`/`SG_UF`, `crate::binding::BrBinding::filing_identity`'s
//! `filer_name`/`district`) live inside `consulta_cand` — the SAME per-cycle
//! bulk file [`crate::BrAdapter::discover_year`] already downloads to
//! discover filings in the first place. So "seeding the roster" for `br`
//! really means: mint one `politician` + `mandate` row per discovered
//! candidate for the year(s) being processed, using the SAME fields
//! `filing_identity()` will later derive from Silver. There is no separate
//! roster authority to pre-load ahead of time, and no full 1933-2024
//! "historical roster" to build in one pass — each election cycle's roster
//! is really just that cycle's candidate list, seeded when that cycle is
//! actually backfilled.
//!
//! `pipeline::stages::roster::resolve_politician` still requires an EXACT
//! pre-seeded `(alias, district, body)` match — no politician is ever
//! minted automatically inside the `Runner` (`crates/pipeline/src/run.rs`'s
//! `publish_document` fails a filing closed with an `unresolved_filer`
//! `review_task` when resolution comes back empty, invariant 3: never
//! guess). This module is the precondition step that makes real `br`
//! resolution possible at all — confirmed by reading `resolve_politician`
//! and `publish_document` directly: there is no "mint a new politician on
//! first sight" path anywhere in the Runner for ANY regime, `br` included.
//!
//! ## Multi-body support (widened, this pass)
//!
//! [`RegimeBinding`] itself still carries exactly one `body` string,
//! unchanged — `crates/pipeline/src/stages/roster.rs`'s
//! `resolve_hits`/`seed_roster` were not touched at all, and every OTHER
//! regime (`us_house`) still constructs and uses exactly one
//! `RegimeBinding` the same way it always has (zero blast radius). `br`
//! resolves its own two-body scope (`AUTHORITY.md` `bodies`: Câmara dos
//! Deputados + Senado Federal) by constructing TWO `RegimeBinding` values
//! that share one `jurisdiction_id` but differ in `regime_id`/`body`
//! ([`regime_binding`]/[`regime_binding_senado`], dispatched via
//! [`RosterBody`]) — one per elected body, each backed by its OWN
//! `disclosure_regime` row (see [`REGIME_ID_SENADO`]'s doc comment for why
//! a second row, not a shared one). Since `mandate.body` — not
//! `jurisdiction_id`/`regime_id` — is the column `resolve_hits`'s WHERE
//! clause actually matches on, giving `Senado Federal` its own body value
//! also STRUCTURALLY FIXES the residual cross-cargo resolution risk the
//! nationwide-2022 real write flagged (a `SENADOR`/suplente candidate could
//! previously only ever accidentally resolve against an existing
//! `DEPUTADO FEDERAL` mandate, because both were checked under the same
//! single body): a `Senado Federal`-bound lookup structurally cannot match
//! a `Câmara dos Deputados` mandate row, or vice versa, regardless of name
//! collisions. See [`RosterBody`] for the cargo->body mapping, the
//! suplente-handling decision, and the same-pass identity-collision logic's
//! per-body scoping — all reasoned through and write-back'd to
//! `docs/regimes/br/AUTHORITY.md`'s Quirks log rather than assumed.

use anyhow::Context as _;
use chrono::NaiveDate;
use sqlx::PgPool;

use pipeline::adapter::{FilingRef, JurisdictionAdapter as _, RunCtx};
use pipeline::run::RegimeBinding;
use pipeline::stages::roster::{RosterMember, seed_roster};
use pipeline::stages::seed::{JurisdictionSeed, RegimeSeed};

use crate::BrAdapter;
use crate::binding::external_identifier;

/// Stable `disclosure_regime.id` — the SAME value `crate::normalize`'s
/// private `CONFORMANCE_REGIME_ID` and `worker::bin::local_br`'s own proof
/// constant already use (see those modules' own doc comments for why).
pub const REGIME_ID: &str = "0BRAREG0000000000000000001";
/// Stable `jurisdiction.id`.
pub const JURISDICTION_ID: &str = "br";
/// Câmara dos Deputados body string — [`regime_binding`]/[`regime_seed`].
pub const BODY: &str = "Câmara dos Deputados";
/// Second `disclosure_regime.id`, added this pass for the Senado Federal
/// body — same jurisdiction/law (Lei 9.504/1997), same cadence/source, only
/// the elected body differs (`AUTHORITY.md` `bodies`). A DISTINCT row
/// rather than reusing [`REGIME_ID`] so `filing.regime_id`/
/// `disclosure_record.regime_id` accurately name which chamber a candidacy
/// is for — mirroring this schema's own established convention of one
/// `disclosure_regime` row per (jurisdiction, body) pair (`disclosure_regime.body`'s
/// own column comment in `crates/core/migrations/0001_core.sql` gives
/// `'US House'`/`'US Senate'` as the worked example: two bodies, two rows,
/// same country). **No migration needed**: `disclosure_regime` already
/// supports more than one body per jurisdiction (its own `unique
/// (jurisdiction_id, body, effective_from)` constraint requires exactly
/// this shape) — this is just a second idempotent seed row via the
/// existing `seed_regime()` path ([`regime_seed_senado`]), never a schema
/// change.
pub const REGIME_ID_SENADO: &str = "0BRAREG0000000000000000002";
/// Senado Federal body string — [`regime_binding_senado`]/[`regime_seed_senado`].
pub const BODY_SENADO: &str = "Senado Federal";

/// Lei 9.504/1997 enactment (`AUTHORITY.md` `regime_versions`, first entry)
/// — the regime's `effective_from`, proven valid at compile time.
const EFFECTIVE_FROM: NaiveDate = match NaiveDate::from_ymd_opt(1997, 9, 30) {
    Some(date) => date,
    None => panic!("1997-09-30 is a valid date"),
};

/// Runner binding constants for `br`.
#[must_use]
pub fn regime_binding() -> RegimeBinding {
    RegimeBinding {
        regime_id: REGIME_ID.to_owned(),
        jurisdiction_id: JURISDICTION_ID.to_owned(),
        body: BODY.to_owned(),
    }
}

/// The `br` regime row (same values `worker::bin::local_br` already seeded
/// into the shared dev DB — idempotent by primary key, `ON CONFLICT DO
/// NOTHING`, so this is a no-op replay against that existing row).
#[must_use]
pub fn regime_seed() -> RegimeSeed {
    RegimeSeed {
        jurisdiction: JurisdictionSeed {
            id: JURISDICTION_ID.to_owned(),
            name: "Brazil".to_owned(),
            iso_code: Some("BR".to_owned()),
            level: "national".to_owned(),
        },
        regime_id: REGIME_ID.to_owned(),
        body: BODY.to_owned(),
        regime_type: "periodic_declaration".to_owned(),
        value_precision: "exact".to_owned(),
        cadence: Some(
            "quadrennial candidacy-time snapshot (declaração de bens); filed once per \
             candidacy at each federal general election, not rolling/annual"
                .to_owned(),
        ),
        disclosure_lag_days: None,
        source_url: Some(
            "https://cdn.tse.jus.br/estatistica/sead/odsele/bem_candidato/bem_candidato_2022.zip"
                .to_owned(),
        ),
        effective_from: EFFECTIVE_FROM,
    }
}

/// Runner binding constants for `br`'s Senado Federal body (see
/// [`REGIME_ID_SENADO`]'s doc comment / module doc comment for why this is a
/// second binding rather than widening [`RegimeBinding`] itself).
#[must_use]
pub fn regime_binding_senado() -> RegimeBinding {
    RegimeBinding {
        regime_id: REGIME_ID_SENADO.to_owned(),
        jurisdiction_id: JURISDICTION_ID.to_owned(),
        body: BODY_SENADO.to_owned(),
    }
}

/// The `br` Senado Federal `disclosure_regime` row — same law/cadence/source
/// as [`regime_seed`], different body/id.
#[must_use]
pub fn regime_seed_senado() -> RegimeSeed {
    RegimeSeed {
        jurisdiction: JurisdictionSeed {
            id: JURISDICTION_ID.to_owned(),
            name: "Brazil".to_owned(),
            iso_code: Some("BR".to_owned()),
            level: "national".to_owned(),
        },
        regime_id: REGIME_ID_SENADO.to_owned(),
        body: BODY_SENADO.to_owned(),
        regime_type: "periodic_declaration".to_owned(),
        value_precision: "exact".to_owned(),
        cadence: Some(
            "quadrennial candidacy-time snapshot (declaração de bens); filed once per \
             candidacy at each federal general election, not rolling/annual"
                .to_owned(),
        ),
        disclosure_lag_days: None,
        source_url: Some(
            "https://cdn.tse.jus.br/estatistica/sead/odsele/bem_candidato/bem_candidato_2022.zip"
                .to_owned(),
        ),
        effective_from: EFFECTIVE_FROM,
    }
}

/// Which body a `DS_CARGO` value belongs to for roster-resolution purposes
/// (design decision — `docs/regimes/br/AUTHORITY.md` Quirks log): `DEPUTADO
/// FEDERAL` -> Câmara dos Deputados; `SENADOR` AND both suplente ranks ->
/// Senado Federal.
///
/// **Suplente-handling decision**: TSE registers each Senate ticket as
/// THREE distinctly-named, separately-`SQ_CANDIDATO`/CPF real candidates —
/// one titular (`SENADOR`) plus two ranked alternates (`1º`/`2º SUPLENTE`)
/// — never one person under three aliases. A suplente is a real, disclosure-
/// relevant political figure in their own right: Brazilian practice
/// routinely has a suplente actually EXERCISE the mandate for extended
/// periods (the titular takes a ministry/governorship "on license", or
/// resigns/dies), so their own asset declaration is exactly the kind of
/// fact this project exists to track, not a lesser or derived record. They
/// are therefore seeded as their OWN politicians (own `politician`/
/// `politician_alias`/`mandate` rows), sharing the titular's BODY (a
/// suplente is, constitutionally, a member of Senado Federal the moment
/// they take the seat, and there is no separate roster-resolution reason to
/// split them out) but keeping their own `mandate.role` = the raw
/// `DS_CARGO` (e.g. `"1º SUPLENTE"`) for that distinction — `role` is
/// display/audit-only, never part of `resolve_hits`'s match key, so this
/// costs nothing in resolution precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RosterBody {
    /// Câmara dos Deputados — `DEPUTADO FEDERAL` only (this seed path's
    /// original, single-body scope).
    Camara,
    /// Senado Federal — `SENADOR` plus `1º SUPLENTE`/`2º SUPLENTE`.
    Senado,
}

/// Every [`RosterBody`] this regime seeds — the single source of truth
/// [`seed_candidates_year`] and `worker::bin::backfill-real-br` loop over,
/// so a third body (should TSE ever add one) needs updating in exactly one
/// place.
pub const ROSTER_BODIES: [RosterBody; 2] = [RosterBody::Camara, RosterBody::Senado];

impl RosterBody {
    /// The `DS_CARGO` values that seed under this body.
    #[must_use]
    pub fn cargos(self) -> &'static [&'static str] {
        match self {
            Self::Camara => &["DEPUTADO FEDERAL"],
            Self::Senado => &["SENADOR", "1º SUPLENTE", "2º SUPLENTE"],
        }
    }

    /// This body's [`RegimeBinding`] (roster-resolution scope).
    #[must_use]
    pub fn regime_binding(self) -> RegimeBinding {
        match self {
            Self::Camara => regime_binding(),
            Self::Senado => regime_binding_senado(),
        }
    }

    /// This body's `disclosure_regime` seed row.
    #[must_use]
    pub fn regime_seed(self) -> RegimeSeed {
        match self {
            Self::Camara => regime_seed(),
            Self::Senado => regime_seed_senado(),
        }
    }
}

/// Looks up which [`RosterBody`] a `DS_CARGO` value belongs to. `None` for a
/// cargo outside `crate::parse::IN_SCOPE_CARGOS` entirely — should not occur
/// since discovery already filters to that scope; handled defensively
/// (counted as [`YearSeedResult::skipped_other_cargo`]) rather than assumed
/// impossible.
#[must_use]
pub fn roster_body_for_cargo(ds_cargo: &str) -> Option<RosterBody> {
    ROSTER_BODIES
        .into_iter()
        .find(|body| body.cargos().contains(&ds_cargo))
}

/// One discovered candidate's roster-relevant identity, alongside the
/// [`FilingRef`] callers need to drive discovery/fetch/publish. Identity
/// fields are peeked from the SAME cached joined declaration
/// `BrAdapter::discover_year` already populated (`crate::adapter` module
/// doc comment) via the adapter's own public `fetch()` — never a second
/// network round trip, never a dependency on the private
/// `crate::parse::ConsultaCand` type.
#[derive(Debug, Clone)]
pub struct DiscoveredCandidate {
    /// Where `fetch`/`Runner::run_over` retrieve this candidate's document.
    pub filing_ref: FilingRef,
    /// `consulta_cand.NM_CANDIDATO` — this candidate's roster `filed_alias`.
    pub nm_candidato: String,
    /// `consulta_cand.SG_UF` — this candidate's roster `district`.
    pub sg_uf: String,
    /// `consulta_cand.DS_CARGO` — dispatched to a [`RosterBody`] via
    /// [`roster_body_for_cargo`] by [`seed_candidates_year`].
    pub ds_cargo: String,
    /// This candidate's durable per-filer identifier (design
    /// `docs/decisions/politician-identity-resolution-design.md` §3.2) —
    /// CPF when present and unmasked, else the voter-registration number,
    /// else `None`. Read from the SAME cached joined declaration as the
    /// other identity fields, independent of the `ctx.pool.is_some()` PII
    /// gate that governs `SilverRow` (this reads raw Bronze bytes directly,
    /// never Silver).
    pub external_identifier: Option<String>,
}

/// One joined declaration's `consulta_cand` identity fields, extracted from
/// its raw JSON bytes.
struct CandidateIdentity {
    nm_candidato: String,
    sg_uf: String,
    ds_cargo: String,
    external_identifier: Option<String>,
}

/// Parses one joined declaration's `consulta_cand` identity fields out of
/// its raw JSON bytes. Pure and offline-testable (unlike
/// [`discover_candidates_year`], which needs a live `BrAdapter` cache) —
/// isolated here so the one part of this module most likely to have a
/// mapping bug has direct unit coverage.
///
/// # Errors
/// The bytes are not the `{"consulta_cand": {...}}` join shape, or a
/// required identity field is missing — fail closed (invariant 6), never
/// guessed.
fn extract_identity(bytes: &[u8], external_id: &str) -> anyhow::Result<CandidateIdentity> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .with_context(|| format!("parsing joined declaration for {external_id}"))?;
    let field = |name: &str| -> anyhow::Result<String> {
        value["consulta_cand"][name]
            .as_str()
            .map(str::to_owned)
            .with_context(|| format!("consulta_cand.{name} missing for {external_id}"))
    };
    let nm_candidato = field("NM_CANDIDATO")?;
    let sg_uf = field("SG_UF")?;
    let ds_cargo = field("DS_CARGO")?;
    let cpf = field("NR_CPF_CANDIDATO")?;
    let titulo = field("NR_TITULO_ELEITORAL_CANDIDATO")?;
    Ok(CandidateIdentity {
        nm_candidato,
        sg_uf,
        ds_cargo,
        external_identifier: external_identifier(Some(&cpf), Some(&titulo)),
    })
}

/// Peeks one discovered filing's cached identity via the adapter's own
/// public `fetch()` (a cache hit against the in-process `joined_cache`
/// `discover_year` already populated — see [`DiscoveredCandidate`]).
async fn peek_identity(
    adapter: &BrAdapter,
    ctx: &RunCtx,
    filing_ref: &FilingRef,
) -> anyhow::Result<CandidateIdentity> {
    let doc = adapter
        .fetch(filing_ref, ctx)
        .await
        .with_context(|| format!("re-reading cached identity for {}", filing_ref.external_id))?;
    let bytes = ctx.bronze.get(&doc)?;
    extract_identity(&bytes, &filing_ref.external_id)
}

/// Discovers one year's in-scope candidates (`br`'s `IN_SCOPE_CARGOS` —
/// every cargo, not just `DEPUTADO FEDERAL`) WITH their roster-relevant
/// identity attached. Shared by [`seed_candidates_year`] and
/// `worker::bin::backfill-real-br`'s own `--uf` bound — one discovery+peek
/// pass, reused everywhere a caller needs to filter/scope candidates before
/// seeding or publishing.
///
/// # Errors
/// Discovery failure for the year (network/parse — fails the whole year
/// closed at the caller), or a joined declaration missing an expected
/// `consulta_cand` identity field (contract drift — fail closed).
pub async fn discover_candidates_year(
    adapter: &BrAdapter,
    ctx: &RunCtx,
    year: i32,
) -> anyhow::Result<Vec<DiscoveredCandidate>> {
    let refs = adapter
        .discover_year(year, ctx)
        .await
        .with_context(|| format!("discovering {year}"))?;
    let mut out = Vec::with_capacity(refs.len());
    for filing_ref in refs {
        let identity = peek_identity(adapter, ctx, &filing_ref).await?;
        out.push(DiscoveredCandidate {
            filing_ref,
            nm_candidato: identity.nm_candidato,
            sg_uf: identity.sg_uf,
            ds_cargo: identity.ds_cargo,
            external_identifier: identity.external_identifier,
        });
    }
    Ok(out)
}

/// One candidate that failed to seed WITHIN an otherwise-good year (mirrors
/// `us_house::seed::MemberSeedError`'s per-member isolation) — e.g.
/// `seed_roster` found the roster ambiguous for this one candidate.
/// Recorded instead of sinking the year: every other real candidate in the
/// same year still seeds.
#[derive(Debug, Clone)]
pub struct CandidateSeedError {
    /// The candidate's `FilingRef.external_id` (`{year}:{SQ_CANDIDATO}`).
    pub external_id: String,
    /// The candidate's `NM_CANDIDATO`.
    pub filed_alias: String,
    /// The candidate's `SG_UF`.
    pub district: String,
    /// `seed_roster`'s own error for this one candidate.
    pub error: String,
}

/// One year's `br` candidate-roster seeding outcome.
#[derive(Debug, Clone, Default)]
pub struct YearSeedResult {
    /// The year seeded.
    pub year: i32,
    /// Full in-scope (`IN_SCOPE_CARGOS`) discovery count for the year —
    /// every cargo, not just `DEPUTADO FEDERAL` (honest "full scope"
    /// reporting, `worker::backfill::YearDiff` precedent).
    pub discovered: usize,
    /// Candidates actually considered this call, after any caller-applied
    /// `uf_filter` bound (a proof-scale run narrows this; a full run leaves
    /// it equal to `discovered`).
    pub considered: usize,
    /// Candidates newly inserted as politicians, summed across BOTH
    /// [`RosterBody`] groups this pass now seeds.
    pub inserted: u32,
    /// Candidates skipped because [`roster_body_for_cargo`] returned `None`
    /// for their `DS_CARGO` — defensive only; every cargo
    /// `crate::parse::IN_SCOPE_CARGOS` admits now maps to exactly one
    /// [`RosterBody`], so this should always be `0` in practice (see that
    /// function's doc comment).
    pub skipped_other_cargo: usize,
    /// Individual candidates this call could not seed (e.g. an ambiguous
    /// roster match) — does not sink the rest of the year.
    pub errors: Vec<CandidateSeedError>,
}

/// Counts, per `(NM_CANDIDATO, SG_UF)` identity, how many DISTINCT
/// candidates whose `DS_CARGO` is in `cargos` THIS PASS share it (after the
/// same `uf_filter` scoping [`seed_candidates_year`] itself applies). Pure
/// and network/DB-free — the safety net `seed_roster`'s own ambiguity check
/// cannot provide on its own: that check only rejects when 2+ rows are
/// ALREADY COMMITTED in the database before a call starts, so it never sees
/// two DIFFERENT candidates (different `SQ_CANDIDATO`) discovered in the
/// SAME call before either is committed — the second one silently resolves
/// onto the first's just-inserted (or just-matched) row instead, with no
/// error and no trace. Confirmed REAL at nationwide 2022 scale, not
/// hypothetical: 89 such `(alias, district)` pairs (178 distinct real
/// candidates) — common-name collisions within one state's proportional-list
/// ballot. A count `> 1` means every candidate sharing that identity must be
/// refused (invariant 3: never guess entities), not just whichever one a
/// caller happens to process second.
///
/// **Scoped per [`RosterBody`], not globally** (design decision,
/// `docs/regimes/br/AUTHORITY.md` Quirks log): `seed_candidates_year` calls
/// this once per body, passing that body's OWN `cargos()`. A `DEPUTADO
/// FEDERAL` candidate and a `SENADOR`/suplente candidate sharing the exact
/// same `(NM_CANDIDATO, SG_UF)` is deliberately NOT flagged here — `mandate.
/// body` is part of `resolve_hits`'s own WHERE clause (`crates/pipeline/src/
/// stages/roster.rs`), so the two bodies' roster lookups can never merge
/// onto the same mandate row regardless of a name collision; treating a
/// cross-body match as an ambiguity would refuse otherwise-legitimate seeds
/// for no real safety benefit. This is not merely theoretical: the
/// nationwide-2022 real write found and independently CPF-verified 3 real
/// individuals who filed under two different cargos in the same cycle
/// (e.g. `DEPUTADO FEDERAL` + `SENADOR`) — seeding each candidacy under its
/// own body is the correct outcome for that case too, not a risk to guard
/// against. A same-body collision (e.g. two `SENADOR`/suplente candidates,
/// or two `1º SUPLENTE`s, sharing identity) is exactly as real a risk as
/// the original `DEPUTADO FEDERAL`-only case and IS guarded, per body.
fn identity_collision_counts(
    candidates: &[DiscoveredCandidate],
    cargos: &[&str],
    uf_filter: &[String],
) -> std::collections::HashMap<(String, String), u32> {
    let mut counts = std::collections::HashMap::new();
    for candidate in candidates {
        if !cargos.contains(&candidate.ds_cargo.as_str()) {
            continue;
        }
        if !uf_filter.is_empty() && !uf_filter.iter().any(|uf| uf == &candidate.sg_uf) {
            continue;
        }
        *counts
            .entry((candidate.nm_candidato.clone(), candidate.sg_uf.clone()))
            .or_insert(0u32) += 1;
    }
    counts
}

/// Seeds one year's in-scope candidates as politicians, across BOTH
/// [`RosterBody`] groups this regime now covers (module doc comment).
/// `uf_filter`, when non-empty, additionally bounds seeding to those states
/// only — a PROOF-scale bound, not meant for a genuine full-year seed (an
/// empty filter seeds every in-scope candidate `discover_year` returns).
/// One discovery pass serves both bodies (never a second network fetch for
/// the same year, invariant 10).
///
/// # Errors
/// Discovery failure for the year (fails the whole year closed at the
/// caller, mirrors `us_house::seed::seed_historical_rosters`'s per-year
/// isolation).
pub async fn seed_candidates_year(
    adapter: &BrAdapter,
    ctx: &RunCtx,
    pool: &PgPool,
    year: i32,
    uf_filter: &[String],
) -> anyhow::Result<YearSeedResult> {
    let candidates = discover_candidates_year(adapter, ctx, year).await?;
    let mut result = YearSeedResult {
        year,
        discovered: candidates.len(),
        ..Default::default()
    };
    // One collision map PER BODY — see identity_collision_counts's own doc
    // comment for why a cross-body name+state match is deliberately not
    // flagged here.
    let camara_collisions =
        identity_collision_counts(&candidates, RosterBody::Camara.cargos(), uf_filter);
    let senado_collisions =
        identity_collision_counts(&candidates, RosterBody::Senado.cargos(), uf_filter);
    let camara_regime = RosterBody::Camara.regime_binding();
    let senado_regime = RosterBody::Senado.regime_binding();

    for candidate in candidates {
        if !uf_filter.is_empty() && !uf_filter.iter().any(|uf| uf == &candidate.sg_uf) {
            continue;
        }
        result.considered += 1;
        let Some(body) = roster_body_for_cargo(&candidate.ds_cargo) else {
            result.skipped_other_cargo += 1;
            continue;
        };
        let key = (candidate.nm_candidato.clone(), candidate.sg_uf.clone());
        let collisions = match body {
            RosterBody::Camara => camara_collisions.get(&key).copied().unwrap_or(0),
            RosterBody::Senado => senado_collisions.get(&key).copied().unwrap_or(0),
        };
        if collisions > 1 {
            result.errors.push(CandidateSeedError {
                external_id: candidate.filing_ref.external_id.clone(),
                filed_alias: candidate.nm_candidato,
                district: candidate.sg_uf,
                error: format!(
                    "same-pass (alias, district) collision within {body:?}: {collisions} \
                     distinct candidates this pass share this identity — refusing to guess \
                     which is which (invariant 3)"
                ),
            });
            continue;
        }
        let member = RosterMember {
            canonical_name: candidate.nm_candidato.clone(),
            filed_alias: candidate.nm_candidato.clone(),
            district: candidate.sg_uf.clone(),
            // Raw DS_CARGO, not a fixed constant (module doc comment /
            // RosterBody's suplente-handling decision): distinguishes
            // SENADOR from 1º/2º SUPLENTE in mandate.role without affecting
            // resolution (role is never part of resolve_hits's match key).
            role: candidate.ds_cargo.clone(),
            active_year: year,
            external_identifier: candidate.external_identifier.clone(),
        };
        let regime = match body {
            RosterBody::Camara => &camara_regime,
            RosterBody::Senado => &senado_regime,
        };
        match seed_roster(pool, regime, std::slice::from_ref(&member)).await {
            Ok(inserted) => result.inserted += inserted,
            Err(error) => result.errors.push(CandidateSeedError {
                external_id: candidate.filing_ref.external_id.clone(),
                filed_alias: candidate.nm_candidato,
                district: candidate.sg_uf,
                error: format!("{error:#}"),
            }),
        }
    }
    Ok(result)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn regime_constants_match_the_conformance_pin() {
        assert_eq!(REGIME_ID, "0BRAREG0000000000000000001");
        let seed = regime_seed();
        assert_eq!(seed.regime_id, REGIME_ID);
        assert_eq!(seed.body, BODY);
        assert_eq!(seed.effective_from.to_string(), "1997-09-30");
        let binding = regime_binding();
        assert_eq!(binding.regime_id, seed.regime_id);
        assert_eq!(binding.jurisdiction_id, seed.jurisdiction.id);
        assert_eq!(binding.body, seed.body);
    }

    /// Same shape as the Câmara constants test above, for the Senado Federal
    /// body added this pass — and a distinctness check: the two bodies must
    /// never accidentally share an id/body string (that would silently
    /// collapse the whole point of the widen).
    #[test]
    fn senado_regime_constants_are_distinct_from_camara() {
        assert_eq!(REGIME_ID_SENADO, "0BRAREG0000000000000000002");
        let seed = regime_seed_senado();
        assert_eq!(seed.regime_id, REGIME_ID_SENADO);
        assert_eq!(seed.body, BODY_SENADO);
        assert_eq!(seed.effective_from.to_string(), "1997-09-30");
        let binding = regime_binding_senado();
        assert_eq!(binding.regime_id, seed.regime_id);
        assert_eq!(binding.jurisdiction_id, JURISDICTION_ID);
        assert_eq!(binding.body, seed.body);

        assert_ne!(REGIME_ID, REGIME_ID_SENADO);
        assert_ne!(BODY, BODY_SENADO);
    }

    #[test]
    fn roster_body_for_cargo_maps_every_in_scope_cargo() {
        assert_eq!(
            roster_body_for_cargo("DEPUTADO FEDERAL"),
            Some(RosterBody::Camara)
        );
        assert_eq!(roster_body_for_cargo("SENADOR"), Some(RosterBody::Senado));
        assert_eq!(
            roster_body_for_cargo("1º SUPLENTE"),
            Some(RosterBody::Senado)
        );
        assert_eq!(
            roster_body_for_cargo("2º SUPLENTE"),
            Some(RosterBody::Senado)
        );
        assert_eq!(
            roster_body_for_cargo("GOVERNADOR"),
            None,
            "a cargo outside this regime's scope entirely is never guessed"
        );
    }

    #[test]
    fn extract_identity_reads_consulta_cand_fields() {
        let json = serde_json::json!({
            "consulta_cand": {
                "SQ_CANDIDATO": "10001595344",
                "NM_CANDIDATO": "MARIA TESTE CANDIDATA",
                "SG_UF": "AC",
                "DS_CARGO": "DEPUTADO FEDERAL",
                "NR_TITULO_ELEITORAL_CANDIDATO": "[SYNTHETIC-TITULO]",
                "NR_CPF_CANDIDATO": "[SYNTHETIC-CPF]"
            },
            "bem_candidato": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let identity = extract_identity(&bytes, "2022:10001595344").unwrap();
        assert_eq!(identity.nm_candidato, "MARIA TESTE CANDIDATA");
        assert_eq!(identity.sg_uf, "AC");
        assert_eq!(identity.ds_cargo, "DEPUTADO FEDERAL");
        assert_eq!(
            identity.external_identifier,
            Some("[SYNTHETIC-CPF]".to_owned())
        );
    }

    #[test]
    fn extract_identity_falls_back_to_titulo_when_cpf_masked() {
        let json = serde_json::json!({
            "consulta_cand": {
                "SQ_CANDIDATO": "10001595344",
                "NM_CANDIDATO": "MARIA TESTE CANDIDATA",
                "SG_UF": "AC",
                "DS_CARGO": "DEPUTADO FEDERAL",
                "NR_TITULO_ELEITORAL_CANDIDATO": "[SYNTHETIC-TITULO]",
                "NR_CPF_CANDIDATO": "-4"
            },
            "bem_candidato": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let identity = extract_identity(&bytes, "2022:10001595344").unwrap();
        assert_eq!(
            identity.external_identifier,
            Some("[SYNTHETIC-TITULO]".to_owned())
        );
    }

    #[test]
    fn extract_identity_fails_closed_on_missing_field() {
        let json = serde_json::json!({
            "consulta_cand": {"SQ_CANDIDATO": "1"},
            "bem_candidato": []
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        assert!(extract_identity(&bytes, "2022:1").is_err());
    }

    #[test]
    fn extract_identity_fails_closed_on_non_json() {
        assert!(extract_identity(b"not json", "2022:1").is_err());
    }

    /// Builds a bare [`DiscoveredCandidate`] for identity-collision tests —
    /// only the fields `identity_collision_counts` reads matter.
    fn candidate(external_id: &str, name: &str, uf: &str, cargo: &str) -> DiscoveredCandidate {
        DiscoveredCandidate {
            filing_ref: FilingRef {
                external_id: external_id.to_owned(),
                url: format!("file://{external_id}"),
            },
            nm_candidato: name.to_owned(),
            sg_uf: uf.to_owned(),
            ds_cargo: cargo.to_owned(),
            external_identifier: None,
        }
    }

    /// Reproduces this session's real nationwide-2022 finding at unit scale:
    /// two DIFFERENT candidates (different `external_id`/`SQ_CANDIDATO`)
    /// filing under the exact same `(NM_CANDIDATO, SG_UF)` must BOTH count
    /// as a collision (order-independent — neither is silently preferred),
    /// while a same-name-different-state pair, a genuinely-unique name, and
    /// an out-of-body-scope cargo are all left alone.
    #[test]
    fn identity_collision_counts_flags_same_pass_duplicates_both_ways() {
        let candidates = vec![
            candidate("2022:1", "MARIA TESTE", "AC", "DEPUTADO FEDERAL"),
            candidate("2022:2", "MARIA TESTE", "AC", "DEPUTADO FEDERAL"),
            candidate("2022:3", "JOAO TESTE", "AC", "DEPUTADO FEDERAL"),
            candidate("2022:4", "MARIA TESTE", "AL", "DEPUTADO FEDERAL"),
            candidate("2022:5", "PEDRO TESTE", "AC", "SENADOR"),
        ];
        let counts = identity_collision_counts(&candidates, RosterBody::Camara.cargos(), &[]);
        assert_eq!(
            counts[&("MARIA TESTE".to_owned(), "AC".to_owned())],
            2,
            "two distinct candidates sharing one identity must both be counted"
        );
        assert_eq!(counts[&("JOAO TESTE".to_owned(), "AC".to_owned())], 1);
        assert_eq!(
            counts[&("MARIA TESTE".to_owned(), "AL".to_owned())],
            1,
            "same name, different district — not a collision"
        );
        assert!(
            !counts.contains_key(&("PEDRO TESTE".to_owned(), "AC".to_owned())),
            "out-of-body-scope cargo (SENADOR, not in the Câmara cargo list) is never counted"
        );
    }

    #[test]
    fn identity_collision_counts_respects_uf_filter() {
        let candidates = vec![
            candidate("2022:1", "MARIA TESTE", "AC", "DEPUTADO FEDERAL"),
            candidate("2022:2", "MARIA TESTE", "AC", "DEPUTADO FEDERAL"),
        ];
        // Filtered out of scope entirely — a real collision, but not one
        // this pass considers, so it must not be reported as one.
        let counts =
            identity_collision_counts(&candidates, RosterBody::Camara.cargos(), &["AL".to_owned()]);
        assert!(counts.is_empty());
    }

    /// The collision-logic design decision itself, at unit scale: a
    /// `DEPUTADO FEDERAL` and a `SENADOR` candidate sharing the exact same
    /// `(NM_CANDIDATO, SG_UF)` must NOT collide with each other (different
    /// bodies, different `mandate.body`, structurally can't merge) — but two
    /// candidates sharing an identity WITHIN the Senado body (here, a
    /// `SENADOR` and a `1º SUPLENTE`) must still collide, exactly like the
    /// original Câmara-only case.
    #[test]
    fn identity_collision_counts_is_scoped_per_body_not_globally() {
        let candidates = vec![
            candidate("2022:1", "ANA TESTE", "RJ", "DEPUTADO FEDERAL"),
            candidate("2022:2", "ANA TESTE", "RJ", "SENADOR"),
            candidate("2022:3", "ANA TESTE", "RJ", "1º SUPLENTE"),
        ];
        let camara = identity_collision_counts(&candidates, RosterBody::Camara.cargos(), &[]);
        let senado = identity_collision_counts(&candidates, RosterBody::Senado.cargos(), &[]);
        assert_eq!(
            camara[&("ANA TESTE".to_owned(), "RJ".to_owned())],
            1,
            "exactly one Câmara candidate shares this identity — not a Câmara-scope collision"
        );
        assert_eq!(
            senado[&("ANA TESTE".to_owned(), "RJ".to_owned())],
            2,
            "SENADOR + 1º SUPLENTE sharing this identity IS a same-body (Senado) collision"
        );
    }
}
