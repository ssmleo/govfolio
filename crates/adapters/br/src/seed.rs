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
//! **Known limitation, NOT fixed here**: [`RegimeBinding`] carries a single
//! `body` string, but this regime's own scope covers TWO bodies (Câmara dos
//! Deputados AND Senado Federal — `AUTHORITY.md` `bodies`). Roster
//! resolution matches on `mandate.body = regime.body` (one fixed string
//! only), so a single `RegimeBinding` can resolve exactly one body's
//! candidates. [`seed_candidates_year`] seeds `DEPUTADO FEDERAL` only (this
//! module's [`BODY`]/[`SEEDED_CARGO`], matching `worker::bin::local_br`'s
//! existing choice) and counts (never seeds) any `SENADOR`/suplente
//! candidate it sees. A real `SENADOR` filing reaching the `Runner` still
//! fails closed correctly (`unresolved_filer` `review_task`, invariant 3),
//! it just never resolves under this pass. Supporting `SENADOR` for real
//! needs either a second `RegimeBinding`/regime row scoped to `Senado
//! Federal`, or a `RunnerBinding`/roster design change letting one binding
//! match more than one body — a genuine cross-regime design question,
//! flagged in `docs/regimes/br/AUTHORITY.md` and out of scope for this pass.

use anyhow::Context as _;
use chrono::NaiveDate;
use sqlx::PgPool;

use pipeline::adapter::{FilingRef, JurisdictionAdapter as _, RunCtx};
use pipeline::run::RegimeBinding;
use pipeline::stages::roster::{RosterMember, seed_roster};
use pipeline::stages::seed::{JurisdictionSeed, RegimeSeed};

use crate::BrAdapter;

/// Stable `disclosure_regime.id` — the SAME value `crate::normalize`'s
/// private `CONFORMANCE_REGIME_ID` and `worker::bin::local_br`'s own proof
/// constant already use (see those modules' own doc comments for why).
pub const REGIME_ID: &str = "0BRAREG0000000000000000001";
/// Stable `jurisdiction.id`.
pub const JURISDICTION_ID: &str = "br";
/// Body this seed path resolves against — see module doc comment's "Known
/// limitation" for why this is `DEPUTADO FEDERAL`'s body only.
pub const BODY: &str = "Câmara dos Deputados";
/// The one `DS_CARGO` value [`seed_candidates_year`] seeds (see module doc
/// comment).
const SEEDED_CARGO: &str = "DEPUTADO FEDERAL";

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
    /// `consulta_cand.DS_CARGO` — scoping filter (`SEEDED_CARGO` only is
    /// seeded by [`seed_candidates_year`]; see module doc comment).
    pub ds_cargo: String,
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
fn extract_identity(bytes: &[u8], external_id: &str) -> anyhow::Result<(String, String, String)> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .with_context(|| format!("parsing joined declaration for {external_id}"))?;
    let field = |name: &str| -> anyhow::Result<String> {
        value["consulta_cand"][name]
            .as_str()
            .map(str::to_owned)
            .with_context(|| format!("consulta_cand.{name} missing for {external_id}"))
    };
    Ok((field("NM_CANDIDATO")?, field("SG_UF")?, field("DS_CARGO")?))
}

/// Peeks one discovered filing's cached identity via the adapter's own
/// public `fetch()` (a cache hit against the in-process `joined_cache`
/// `discover_year` already populated — see [`DiscoveredCandidate`]).
async fn peek_identity(
    adapter: &BrAdapter,
    ctx: &RunCtx,
    filing_ref: &FilingRef,
) -> anyhow::Result<(String, String, String)> {
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
        let (nm_candidato, sg_uf, ds_cargo) = peek_identity(adapter, ctx, &filing_ref).await?;
        out.push(DiscoveredCandidate {
            filing_ref,
            nm_candidato,
            sg_uf,
            ds_cargo,
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
    /// `DEPUTADO FEDERAL` candidates newly inserted as politicians.
    pub inserted: u32,
    /// Candidates skipped because their `DS_CARGO` is outside this seed
    /// path's single-body scope (see module doc comment) — not an error,
    /// just outside what this pass resolves.
    pub skipped_other_cargo: usize,
    /// Individual candidates this call could not seed (e.g. an ambiguous
    /// roster match) — does not sink the rest of the year.
    pub errors: Vec<CandidateSeedError>,
}

/// Counts, per `(NM_CANDIDATO, SG_UF)` identity, how many DISTINCT
/// `SEEDED_CARGO` candidates THIS PASS share it (after the same `uf_filter`
/// scoping [`seed_candidates_year`] itself applies). Pure and
/// network/DB-free — the safety net `seed_roster`'s own ambiguity check
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
fn identity_collision_counts(
    candidates: &[DiscoveredCandidate],
    uf_filter: &[String],
) -> std::collections::HashMap<(String, String), u32> {
    let mut counts = std::collections::HashMap::new();
    for candidate in candidates {
        if candidate.ds_cargo != SEEDED_CARGO {
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

/// Seeds one year's `DEPUTADO FEDERAL` candidates as politicians (module doc
/// comment's judgment call). `uf_filter`, when non-empty, additionally
/// bounds seeding to those states only — a PROOF-scale bound, not meant for
/// a genuine full-year seed (an empty filter seeds every in-scope candidate
/// `discover_year` returns).
///
/// # Errors
/// Discovery failure for the year (fails the whole year closed at the
/// caller, mirrors `us_house::seed::seed_historical_rosters`'s per-year
/// isolation).
pub async fn seed_candidates_year(
    adapter: &BrAdapter,
    ctx: &RunCtx,
    pool: &PgPool,
    regime: &RegimeBinding,
    year: i32,
    uf_filter: &[String],
) -> anyhow::Result<YearSeedResult> {
    let candidates = discover_candidates_year(adapter, ctx, year).await?;
    let mut result = YearSeedResult {
        year,
        discovered: candidates.len(),
        ..Default::default()
    };
    let collision_counts = identity_collision_counts(&candidates, uf_filter);
    for candidate in candidates {
        if !uf_filter.is_empty() && !uf_filter.iter().any(|uf| uf == &candidate.sg_uf) {
            continue;
        }
        result.considered += 1;
        if candidate.ds_cargo != SEEDED_CARGO {
            result.skipped_other_cargo += 1;
            continue;
        }
        let key = (candidate.nm_candidato.clone(), candidate.sg_uf.clone());
        let collisions = collision_counts.get(&key).copied().unwrap_or(0);
        if collisions > 1 {
            result.errors.push(CandidateSeedError {
                external_id: candidate.filing_ref.external_id.clone(),
                filed_alias: candidate.nm_candidato,
                district: candidate.sg_uf,
                error: format!(
                    "same-pass (alias, district) collision: {collisions} distinct candidates \
                     this pass share this identity — refusing to guess which is which \
                     (invariant 3)"
                ),
            });
            continue;
        }
        let member = RosterMember {
            canonical_name: candidate.nm_candidato.clone(),
            filed_alias: candidate.nm_candidato.clone(),
            district: candidate.sg_uf.clone(),
            role: SEEDED_CARGO.to_owned(),
            active_year: year,
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
        let (name, uf, cargo) = extract_identity(&bytes, "2022:10001595344").unwrap();
        assert_eq!(name, "MARIA TESTE CANDIDATA");
        assert_eq!(uf, "AC");
        assert_eq!(cargo, "DEPUTADO FEDERAL");
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
        }
    }

    /// Reproduces this session's real nationwide-2022 finding at unit scale:
    /// two DIFFERENT candidates (different `external_id`/`SQ_CANDIDATO`)
    /// filing under the exact same `(NM_CANDIDATO, SG_UF)` must BOTH count
    /// as a collision (order-independent — neither is silently preferred),
    /// while a same-name-different-state pair, a genuinely-unique name, and
    /// an out-of-scope cargo are all left alone.
    #[test]
    fn identity_collision_counts_flags_same_pass_duplicates_both_ways() {
        let candidates = vec![
            candidate("2022:1", "MARIA TESTE", "AC", "DEPUTADO FEDERAL"),
            candidate("2022:2", "MARIA TESTE", "AC", "DEPUTADO FEDERAL"),
            candidate("2022:3", "JOAO TESTE", "AC", "DEPUTADO FEDERAL"),
            candidate("2022:4", "MARIA TESTE", "AL", "DEPUTADO FEDERAL"),
            candidate("2022:5", "PEDRO TESTE", "AC", "SENADOR"),
        ];
        let counts = identity_collision_counts(&candidates, &[]);
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
            "out-of-scope cargo (SENADOR) is never counted"
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
        let counts = identity_collision_counts(&candidates, &["AL".to_owned()]);
        assert!(counts.is_empty());
    }
}
