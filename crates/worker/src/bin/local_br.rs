//! Offline local pipeline run — `br` proof (mirrors `worker::bin::local`'s
//! `us_house` proof, goal 001 Task 9 — see that file's own doc comment): drives
//! the in-process runner over the 3 existing `br` fixture cases
//! (`crates/adapters/br/fixtures/*/input.json`) with NO real TSE network fetch.
//!
//! `br`'s own `parse()` (`crates/adapters/br/src/adapter.rs`) is a pure
//! `serde_json` deserialize of an already-joined
//! `{"consulta_cand": {...}, "bem_candidato": [...]}` document — exactly the
//! shape `input.json` already is (fixtures `MANIFEST.json packaging_note`).
//! `Runner::run_local` Bronzes each input file's raw bytes and calls
//! `adapter.parse()` directly on them, bypassing `discover()`/`fetch()`
//! entirely (see `pipeline::run::Runner::process_local`) — so feeding the
//! fixture files straight in needs no adapter changes at all: `br`'s real
//! `fetch()` output and its fixture `input.json` are already the same shape.
//!
//! Regime + roster rows are seeded INLINE here (not under
//! `crates/adapters/br/src/`, unlike `us_house`'s `seed.rs` module) since this
//! is a one-off local proof binary, not a production seed path — `br` has no
//! production seed module yet. The regime id used here
//! (`0BRAREG0000000000000000001`) is the SAME constant
//! `crates/adapters/br/src/normalize.rs`'s conformance mode pins
//! (`CONFORMANCE_REGIME_ID`), so conformance-mode and this real pool-backed
//! proof agree on which row is "the" `br` regime. This is intentionally
//! DISTINCT from the coverage-factory `stub-regime-br` placeholder
//! (`crates/core/src/seed/mod.rs`, `regime_type: 'none'`, `body:
//! "(unresearched)"`) that already exists for every unbuilt jurisdiction —
//! graduating `br` from that stub to a real `LIVE_REGIMES` entry is a
//! separate, later coverage-factory task, out of scope here.
//!
//! Usage:
//!   cargo run -p worker --bin `local_br`
//!
//! `DATABASE_URL` must point at Postgres (e.g. the portable local PG 16 on
//! :5433; see `docs/runbooks/dev-host-windows.md`).

use std::path::PathBuf;

use anyhow::Context as _;
use chrono::NaiveDate;

use br::BrAdapter;
use br::binding::BrBinding;
use pipeline::adapter::{BronzeStore, Clock, JurisdictionAdapter as _, RunCtx};
use pipeline::conformance::{durable_bronze_parent, fixtures_dir};
use pipeline::run::{LocalFiling, RegimeBinding, Runner};
use pipeline::stages::roster::{RosterMember, seed_roster};
use pipeline::stages::seed::{JurisdictionSeed, RegimeSeed, seed_regime};

/// This proof's own `br` regime row id — see module doc comment for why it's
/// the same value as `normalize.rs`'s conformance constant, and why it's
/// distinct from the coverage-factory stub row.
const REGIME_ID: &str = "0BRAREG0000000000000000001";
const JURISDICTION_ID: &str = "br";
/// Disclosing body (docs/regimes/br/AUTHORITY.md `bodies`); all 3 fixtures are
/// `DEPUTADO FEDERAL` (Câmara), so only that body is seeded here.
const BODY: &str = "Câmara dos Deputados";

/// Lei 9.504/1997 enactment (AUTHORITY.md `regime_versions`, first entry) —
/// the regime's `effective_from`, proven valid at compile time (`us_house`
/// `seed.rs` precedent for this exact pattern).
const EFFECTIVE_FROM: NaiveDate = match NaiveDate::from_ymd_opt(1997, 9, 30) {
    Some(date) => date,
    None => panic!("1997-09-30 is a valid date"),
};

fn regime_binding() -> RegimeBinding {
    RegimeBinding {
        regime_id: REGIME_ID.to_owned(),
        jurisdiction_id: JURISDICTION_ID.to_owned(),
        body: BODY.to_owned(),
    }
}

fn regime_seed() -> RegimeSeed {
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

/// One roster entry per fixture candidate. `filed_alias`/`canonical_name` are
/// BOTH the verbatim `NM_CANDIDATO` — this regime's
/// `RunnerBinding::filing_identity()` (`crates/adapters/br/src/binding.rs`)
/// emits `filer_name` straight from the source's own name field with no
/// honorific to strip, unlike `us_house`.
fn br_roster() -> Vec<RosterMember> {
    vec![
        RosterMember {
            canonical_name: "ROGÉRIO DA SILVA E SILVA".to_owned(),
            filed_alias: "ROGÉRIO DA SILVA E SILVA".to_owned(),
            district: "AC".to_owned(),
            role: "Deputado Federal".to_owned(),
            active_year: 2022,
            external_identifier: None,
        },
        RosterMember {
            canonical_name: "ANA MARIA PEREIRA HORA".to_owned(),
            filed_alias: "ANA MARIA PEREIRA HORA".to_owned(),
            district: "AL".to_owned(),
            role: "Deputado Federal".to_owned(),
            active_year: 2022,
            external_identifier: None,
        },
        RosterMember {
            canonical_name: "WILLINGTON DE MORAIS FERREIRA".to_owned(),
            filed_alias: "WILLINGTON DE MORAIS FERREIRA".to_owned(),
            district: "AC".to_owned(),
            role: "Deputado Federal".to_owned(),
            active_year: 2022,
            external_identifier: None,
        },
    ]
}

/// `<fixtures>/<case>/input.json` for every case directory, sorted
/// (`worker::bin::local`'s `collect_inputs`, `.json` instead of `.pdf`).
fn collect_inputs(fixtures: &PathBuf) -> anyhow::Result<Vec<LocalFiling>> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(fixtures)
        .with_context(|| format!("reading fixtures dir {}", fixtures.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    let inputs: Vec<LocalFiling> = dirs
        .into_iter()
        .map(|dir| LocalFiling {
            path: dir.join("input.json"),
        })
        .filter(|filing| filing.path.is_file())
        .collect();
    anyhow::ensure!(
        !inputs.is_empty(),
        "no <case>/input.json files under {} — nothing to run",
        fixtures.display()
    );
    Ok(inputs)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let database_url =
        std::env::var("DATABASE_URL").context("DATABASE_URL must point at Postgres")?;
    let pool = sqlx::PgPool::connect(&database_url)
        .await
        .context("connecting to Postgres")?;
    govfolio_core::db::migrate(&pool)
        .await
        .context("applying migrations")?;

    // Seed the regime row + politician roster (design §5.4) — see module doc
    // comment for why this is inline rather than a `br::seed` module.
    seed_regime(&pool, &regime_seed()).await?;
    let roster = br_roster();
    let seeded = seed_roster(&pool, &regime_binding(), &roster).await?;
    println!(
        "roster: {} members ({seeded} newly seeded) for regime {REGIME_ID} ({BODY})",
        roster.len(),
    );

    let fixtures = fixtures_dir("br");
    let inputs = collect_inputs(&fixtures)?;
    let bronze = durable_bronze_parent().join("bronze-local-br");
    println!(
        "running {} local br filings from {} (bronze: {})",
        inputs.len(),
        fixtures.display(),
        bronze.display()
    );

    let adapter = BrAdapter::default();
    let binding = BrBinding;
    let ctx = RunCtx::new(
        BronzeStore::open(&bronze)?,
        Some(pool.clone()),
        Clock::System,
        &adapter.politeness(),
    )?;
    let runner = Runner::new(&adapter, &binding, regime_binding(), ctx)?;
    let report = runner.run_local(&inputs).await?;

    println!(
        "filings: {} | published: {} | replayed: {} | gold inserted: {} | \
         outbox written: {} | review tasks: {}",
        report.filings,
        report.published,
        report.replayed,
        report.gold_inserted,
        report.outbox_written,
        report.review_tasks
    );
    if !report.failed.is_empty() {
        for failure in &report.failed {
            eprintln!("FAILED {failure}");
        }
        anyhow::bail!("{} filing(s) failed closed", report.failed.len());
    }
    Ok(())
}
