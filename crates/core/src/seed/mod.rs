//! Worldwide jurisdiction registry seed (design §5.7 launch jurisdictions +
//! §5.8 coverage factory).
//!
//! Seeds every sovereign country ([`iso::COUNTRIES`], the 193 UN members + 2
//! observer states) at `level = 'national'` plus the supranational European
//! Union, and gives every jurisdiction at least one `disclosure_regime` row:
//! the eight BUILT launch regimes ([`LIVE_REGIMES`]) get their real metadata,
//! and every other jurisdiction gets a `regime_type = 'none'` stub the coverage
//! factory later advances (design §5.8 state machine on
//! `jurisdiction.coverage_phase`). This IS the "transparency scorecard"
//! reference data the `/v1/jurisdictions` + `/v1/regimes` endpoints render
//! (design §6.1/§7.3).
//!
//! Idempotent by stable primary key: [`seed_registry`] is `ON CONFLICT DO
//! NOTHING`, so a re-run inserts nothing (invariant 4). The seed is Rust static
//! data (source of truth, unit-testable) executed as two multi-row inserts —
//! the goal-065 "seed fn run by a bin" option; the migrate bin
//! (`cargo run -p core --bin migrate`) applies the schema then this seed, so no
//! new DDL migration is added and the expand-only guardrail stays trivially
//! green.
//!
//! The eight `LIVE_REGIMES` ids are the SAME stable constants the adapters pin
//! in their fixtures (`crates/adapters/<x>/fixtures/MANIFEST.json`), so a
//! registry regime row and the Gold rows an adapter publishes point at one
//! `disclosure_regime.id`. The seven live jurisdictions (`us`, `gb`, `ca`, `au`,
//! `eu`, `fr`, `de`) match the sentinel's `live_targets()` (goal 017) — `us`
//! carries two regimes (House + Senate), so eight regimes span seven
//! jurisdictions (design §5.7).

use chrono::NaiveDate;
use serde_json::json;
use sqlx::{PgPool, Postgres, QueryBuilder};

mod iso;
pub use iso::COUNTRIES;

/// Compile-time-checked date literal (mirrors `us_house::seed`): a bad date is a
/// build error, never a runtime `unwrap` (invariant 8).
const fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
    match NaiveDate::from_ymd_opt(year, month, day) {
        Some(date) => date,
        None => panic!("invalid seed date literal"),
    }
}

/// One BUILT launch regime — the real `disclosure_regime` metadata (regime doc
/// §1) plus the jurisdiction it hangs off. `regime_id` is the stable adapter
/// constant pinned in the adapter's fixtures; `code` is the sentinel /
/// conformance code.
#[derive(Debug, Clone, Copy)]
pub struct LiveRegime {
    /// Stable `disclosure_regime.id` — the adapter's pinned fixture constant.
    pub regime_id: &'static str,
    /// Sentinel / conformance code (`live_targets()`), also the fixture dir.
    pub code: &'static str,
    /// `jurisdiction.id` (lowercase ISO alpha-2, or `eu` supranational).
    pub jurisdiction_id: &'static str,
    /// Jurisdiction display name.
    pub jurisdiction_name: &'static str,
    /// ISO 3166-1 alpha-2 (uppercase; `EU` exceptionally reserved).
    pub iso_code: &'static str,
    /// `national` | `supranational`.
    pub level: &'static str,
    /// Disclosing body, e.g. `US House`.
    pub body: &'static str,
    /// `transaction_report` | `periodic_declaration` | `change_notification`.
    pub regime_type: &'static str,
    /// `exact` | `banded` | `categorical` (never `none` — these are built).
    pub value_precision: &'static str,
    /// Free-form filing cadence description (regime doc §1).
    pub cadence: &'static str,
    /// Statutory maximum disclosure lag in days, when the regime fixes one.
    pub disclosure_lag_days: Option<i32>,
    /// Official source landing page.
    pub source_url: &'static str,
    /// Provenance: the regime doc backing this row (evidence discipline).
    pub regime_doc: &'static str,
    /// Date the regime's rules took effect (regime doc §1 / front-matter).
    pub effective_from: NaiveDate,
}

/// The eight BUILT launch regimes (design §5.7; the E1 epoch). Ids are the
/// adapters' pinned fixture constants — see module docs. `us` appears twice
/// (House + Senate); every other jurisdiction once.
pub static LIVE_REGIMES: &[LiveRegime] = &[
    LiveRegime {
        regime_id: "0HSEREG0000000000000000001",
        code: "us_house",
        jurisdiction_id: "us",
        jurisdiction_name: "United States",
        iso_code: "US",
        level: "national",
        body: "US House",
        regime_type: "transaction_report",
        value_precision: "banded",
        cadence: "rolling; statutory <=30d from notification, <=45d from transaction",
        disclosure_lag_days: Some(45),
        source_url: "https://disclosures-clerk.house.gov/FinancialDisclosure",
        regime_doc: "docs/regimes/us-house.md",
        effective_from: ymd(2012, 4, 4),
    },
    LiveRegime {
        regime_id: "0SENREG0000000000000000001",
        code: "us_senate",
        jurisdiction_id: "us",
        jurisdiction_name: "United States",
        iso_code: "US",
        level: "national",
        body: "US Senate",
        regime_type: "transaction_report",
        value_precision: "banded",
        cadence: "rolling; statutory <=30d from notification, <=45d from transaction",
        disclosure_lag_days: Some(45),
        source_url: "https://efdsearch.senate.gov/search/home/",
        regime_doc: "docs/regimes/us_senate.md",
        effective_from: ymd(2012, 7, 3),
    },
    LiveRegime {
        regime_id: "0GBRREG0000000000000000001",
        code: "uk_commons_register",
        jurisdiction_id: "gb",
        jurisdiction_name: "United Kingdom",
        iso_code: "GB",
        level: "national",
        body: "UK House of Commons",
        regime_type: "change_notification",
        value_precision: "categorical",
        cadence: "rolling registration; formal register republished fortnightly in sitting periods",
        disclosure_lag_days: None,
        source_url: "https://interests-api.parliament.uk/",
        regime_doc: "docs/regimes/uk_commons_register.md",
        effective_from: ymd(2024, 3, 18),
    },
    LiveRegime {
        regime_id: "0CACREG0000000000000000001",
        code: "canada_ciec",
        jurisdiction_id: "ca",
        jurisdiction_name: "Canada",
        iso_code: "CA",
        level: "national",
        body: "Office of the Conflict of Interest and Ethics Commissioner (federal)",
        regime_type: "change_notification",
        value_precision: "none",
        cadence: "rolling; per-type statutory windows (30d gifts, 60d material changes, 120d initial declarations)",
        disclosure_lag_days: None,
        source_url: "https://ciec-ccie.parl.gc.ca/en/public-registry",
        regime_doc: "docs/regimes/canada_ciec.md",
        effective_from: ymd(2008, 5, 2),
    },
    LiveRegime {
        regime_id: "0AHRREG0000000000000000001",
        code: "australia_register",
        jurisdiction_id: "au",
        jurisdiction_name: "Australia",
        iso_code: "AU",
        level: "national",
        body: "Australian House of Representatives",
        regime_type: "change_notification",
        value_precision: "none",
        cadence: "rolling; 28-day windows (initial statement / alteration notice)",
        disclosure_lag_days: None,
        source_url: "https://www.aph.gov.au/Senators_and_Members/Members/Register",
        regime_doc: "docs/regimes/australia_register.md",
        effective_from: ymd(1984, 10, 9),
    },
    LiveRegime {
        regime_id: "0EPDREG0000000000000000001",
        code: "eu_parliament_dpi",
        jurisdiction_id: "eu",
        jurisdiction_name: "European Union",
        iso_code: "EU",
        level: "supranational",
        body: "European Parliament",
        regime_type: "periodic_declaration",
        value_precision: "exact",
        cadence: "start-of-mandate declaration; updated on each change",
        disclosure_lag_days: None,
        source_url: "https://www.europarl.europa.eu/meps/en/full-list/all",
        regime_doc: "docs/regimes/eu_fr_de_annual.md",
        effective_from: ymd(2023, 11, 1),
    },
    LiveRegime {
        regime_id: "0FRHREG0000000000000000001",
        code: "fr_hatvp_dia",
        jurisdiction_id: "fr",
        jurisdiction_name: "France",
        iso_code: "FR",
        level: "national",
        body: "HATVP",
        regime_type: "periodic_declaration",
        value_precision: "exact",
        cadence: "start-of-mandate declaration d'interets et d'activites + modificatives",
        disclosure_lag_days: None,
        source_url: "https://www.hatvp.fr/open-data/",
        regime_doc: "docs/regimes/eu_fr_de_annual.md",
        effective_from: ymd(2013, 10, 11),
    },
    LiveRegime {
        regime_id: "0DEBREG0000000000000000001",
        code: "de_bundestag",
        jurisdiction_id: "de",
        jurisdiction_name: "Germany",
        iso_code: "DE",
        level: "national",
        body: "Deutscher Bundestag",
        regime_type: "periodic_declaration",
        value_precision: "exact",
        cadence: "per-mandate declaration; exact-amount publication since the 2021 Transparenzgesetz",
        disclosure_lag_days: None,
        source_url: "https://www.bundestag.de/abgeordnete/biografien",
        regime_doc: "docs/regimes/eu_fr_de_annual.md",
        effective_from: ymd(2021, 10, 19),
    },
];

/// Effective-from stamped on stub `none` regimes — the registry seed date
/// (goal 065). Reproducible (committed constant), not build-time.
const STUB_EFFECTIVE_FROM: NaiveDate = ymd(2026, 7, 6);
/// Body of a stub regime — reads as "no researched regime yet" on the scorecard.
const STUB_BODY: &str = "(unresearched)";
/// E1 (launch) epoch number and its priority; the built jurisdictions.
const E1_EPOCH: i16 = 1;
const E1_PRIORITY: f32 = 100.0;
/// E2 (Brazil) — the next epoch per `agents/EPOCHS.md`, pre-seeded so the
/// coverage factory picks it up first among the stub long tail.
const E2_EPOCH: i16 = 2;
const E2_PRIORITY: f32 = 90.0;

/// Whether a jurisdiction id carries a BUILT regime (is `coverage_phase = live`).
#[must_use]
pub fn is_live(jurisdiction_id: &str) -> bool {
    LIVE_REGIMES
        .iter()
        .any(|regime| regime.jurisdiction_id == jurisdiction_id)
}

/// `(epoch, coverage_phase, priority_score)` for a jurisdiction id.
fn coverage_for(jurisdiction_id: &str) -> (Option<i16>, &'static str, Option<f32>) {
    if is_live(jurisdiction_id) {
        (Some(E1_EPOCH), "live", Some(E1_PRIORITY))
    } else if jurisdiction_id == "br" {
        (Some(E2_EPOCH), "stub", Some(E2_PRIORITY))
    } else {
        (None, "stub", None)
    }
}

struct JurRow {
    id: String,
    name: String,
    iso: String,
    level: &'static str,
    epoch: Option<i16>,
    phase: &'static str,
    priority: Option<f32>,
}

struct RegRow {
    id: String,
    jurisdiction_id: String,
    body: String,
    regime_type: &'static str,
    value_precision: &'static str,
    cadence: Option<String>,
    disclosure_lag_days: Option<i32>,
    source_url: Option<String>,
    details: serde_json::Value,
    effective_from: NaiveDate,
}

/// Builds the jurisdiction rows: 195 sovereigns from [`COUNTRIES`] plus the
/// supranational EU (the one live jurisdiction not an ISO country).
fn jurisdiction_rows() -> Vec<JurRow> {
    let mut rows: Vec<JurRow> = COUNTRIES
        .iter()
        .map(|(code, name)| {
            let id = code.to_ascii_lowercase();
            let (epoch, phase, priority) = coverage_for(&id);
            JurRow {
                id,
                name: (*name).to_owned(),
                iso: (*code).to_owned(),
                level: "national",
                epoch,
                phase,
                priority,
            }
        })
        .collect();
    rows.push(JurRow {
        id: "eu".to_owned(),
        name: "European Union".to_owned(),
        iso: "EU".to_owned(),
        level: "supranational",
        epoch: Some(E1_EPOCH),
        phase: "live",
        priority: Some(E1_PRIORITY),
    });
    rows
}

/// Builds the regime rows: the eight real launch regimes plus one `none` stub
/// per non-live jurisdiction (so every jurisdiction has >= 1 regime row).
fn regime_rows() -> Vec<RegRow> {
    let mut rows: Vec<RegRow> = LIVE_REGIMES
        .iter()
        .map(|regime| RegRow {
            id: regime.regime_id.to_owned(),
            jurisdiction_id: regime.jurisdiction_id.to_owned(),
            body: regime.body.to_owned(),
            regime_type: regime.regime_type,
            value_precision: regime.value_precision,
            cadence: Some(regime.cadence.to_owned()),
            disclosure_lag_days: regime.disclosure_lag_days,
            source_url: Some(regime.source_url.to_owned()),
            details: json!({
                "coverage": "live",
                "regime_code": regime.code,
                "source": regime.regime_doc,
                "seed_goal": "065",
            }),
            effective_from: regime.effective_from,
        })
        .collect();
    for (code, _name) in COUNTRIES {
        let id = code.to_ascii_lowercase();
        if is_live(&id) {
            continue; // live jurisdictions already carry a real regime
        }
        rows.push(RegRow {
            id: format!("stub-regime-{id}"),
            jurisdiction_id: id,
            body: STUB_BODY.to_owned(),
            regime_type: "none",
            value_precision: "none",
            cadence: None,
            disclosure_lag_days: None,
            source_url: None,
            details: json!({
                "coverage": "stub",
                "seed_basis": "ISO 3166-1",
                "note": "regime unresearched — coverage factory",
                "seed_goal": "065",
            }),
            effective_from: STUB_EFFECTIVE_FROM,
        });
    }
    rows
}

/// Seeds the worldwide jurisdiction registry (design §5.7/§5.8). Idempotent:
/// `ON CONFLICT DO NOTHING`, so a re-run inserts nothing (invariant 4). Two
/// multi-row inserts (jurisdictions, then regimes — FK order).
///
/// # Errors
/// Database failure.
pub async fn seed_registry(pool: &PgPool) -> Result<(), sqlx::Error> {
    let jurs = jurisdiction_rows();
    let mut jur_qb = QueryBuilder::<Postgres>::new(
        "insert into jurisdiction \
           (id, name, iso_code, level, epoch, coverage_phase, priority_score) ",
    );
    jur_qb.push_values(&jurs, |mut b, row| {
        b.push_bind(row.id.as_str())
            .push_bind(row.name.as_str())
            .push_bind(row.iso.as_str())
            .push_bind(row.level)
            .push_bind(row.epoch)
            .push_bind(row.phase)
            .push_bind(row.priority);
    });
    jur_qb.push(" on conflict do nothing");
    jur_qb.build().execute(pool).await?;

    let regs = regime_rows();
    let mut reg_qb = QueryBuilder::<Postgres>::new(
        "insert into disclosure_regime \
           (id, jurisdiction_id, body, regime_type, value_precision, cadence, \
            disclosure_lag_days, source_url, details, effective_from) ",
    );
    reg_qb.push_values(&regs, |mut b, row| {
        b.push_bind(row.id.as_str())
            .push_bind(row.jurisdiction_id.as_str())
            .push_bind(row.body.as_str())
            .push_bind(row.regime_type)
            .push_bind(row.value_precision)
            .push_bind(row.cadence.as_deref())
            .push_bind(row.disclosure_lag_days)
            .push_bind(row.source_url.as_deref())
            .push_bind(row.details.clone())
            .push_bind(row.effective_from);
    });
    reg_qb.push(" on conflict do nothing");
    reg_qb.build().execute(pool).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countries_are_canonical_and_at_least_190() {
        assert!(
            COUNTRIES.len() >= 190,
            "seed must cover >=190 sovereigns, got {}",
            COUNTRIES.len()
        );
        let mut seen = std::collections::BTreeSet::new();
        for (code, name) in COUNTRIES {
            assert!(
                code.len() == 2 && code.chars().all(|c| c.is_ascii_uppercase()),
                "non-ISO alpha-2 code {code:?}"
            );
            assert!(!name.is_empty() && !name.contains(','), "bad name {name:?}");
            assert!(seen.insert(*code), "duplicate ISO code {code:?}");
        }
    }

    #[test]
    fn live_regimes_are_the_eight_built_launch_regimes() {
        assert_eq!(
            LIVE_REGIMES.len(),
            8,
            "design §5.7 has eight launch regimes"
        );
        // Ids match the adapters' pinned fixture constants, and are distinct.
        let ids: std::collections::BTreeSet<&str> =
            LIVE_REGIMES.iter().map(|r| r.regime_id).collect();
        assert_eq!(ids.len(), 8, "regime ids must be distinct");
        let codes: std::collections::BTreeSet<&str> = LIVE_REGIMES.iter().map(|r| r.code).collect();
        assert_eq!(
            codes,
            // exactly the sentinel `live_targets()` codes (goal 017)
            [
                "australia_register",
                "canada_ciec",
                "de_bundestag",
                "eu_parliament_dpi",
                "fr_hatvp_dia",
                "uk_commons_register",
                "us_house",
                "us_senate"
            ]
            .into_iter()
            .collect()
        );
        for regime in LIVE_REGIMES {
            assert_ne!(regime.regime_type, "none", "{} is built", regime.code);
            assert!(
                !regime.source_url.is_empty(),
                "{} needs a source",
                regime.code
            );
        }
    }

    #[test]
    fn seven_live_jurisdictions_us_appears_twice() {
        let live: std::collections::BTreeSet<&str> =
            LIVE_REGIMES.iter().map(|r| r.jurisdiction_id).collect();
        assert_eq!(
            live,
            ["au", "ca", "de", "eu", "fr", "gb", "us"]
                .into_iter()
                .collect()
        );
        let us = LIVE_REGIMES
            .iter()
            .filter(|r| r.jurisdiction_id == "us")
            .count();
        assert_eq!(us, 2, "US House + US Senate");
    }

    #[test]
    fn every_jurisdiction_gets_exactly_one_regime_path() {
        // Row builders agree with the invariant "every jurisdiction has >=1 regime".
        let jurs = jurisdiction_rows();
        let regs = regime_rows();
        let jur_ids: std::collections::BTreeSet<&str> =
            jurs.iter().map(|j| j.id.as_str()).collect();
        // 195 sovereign + EU
        assert_eq!(jurs.len(), COUNTRIES.len() + 1);
        for j in &jurs {
            let has = regs.iter().any(|r| r.jurisdiction_id == j.id);
            assert!(has, "jurisdiction {} has no regime row", j.id);
        }
        // stubs point only at real jurisdictions
        for r in &regs {
            assert!(jur_ids.contains(r.jurisdiction_id.as_str()));
        }
        // 8 real + one stub per non-live jurisdiction
        let stubs = regs.iter().filter(|r| r.regime_type == "none").count();
        assert_eq!(regs.len(), 8 + stubs);
        assert_eq!(jurs.iter().filter(|j| j.phase == "live").count(), 7);
    }
}
