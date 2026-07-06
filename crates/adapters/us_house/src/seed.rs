//! Production seed wiring for the `us_house` regime (plan Task 9): the
//! `disclosure_regime` row (regime doc §1 metadata) and roster seeding from
//! the Clerk's filing-index `Member` data — the official member list design
//! §5.4 prescribes. Offline runs seed from the archived index evidence slice
//! (`docs/regimes/us-house/evidence/`); live runs use the same parser on the
//! index zip XML.

use anyhow::Context as _;
use async_trait::async_trait;
use chrono::NaiveDate;
use sqlx::PgPool;

use pipeline::adapter::RunCtx;
use pipeline::run::RegimeBinding;
use pipeline::stages::roster::{RosterMember, seed_roster};
use pipeline::stages::seed::{JurisdictionSeed, RegimeSeed};

use crate::UsHouseAdapter;
use crate::index;

/// Stable `disclosure_regime.id` — the same constant the conformance fixtures
/// pin (`fixtures/MANIFEST.json`), so fixture-mode and pool-backed runs agree
/// on the regime row.
pub const REGIME_ID: &str = "0HSEREG0000000000000000001";
/// Stable `jurisdiction.id` (ISO 3166-1 alpha-2 lowercase by convention).
pub const JURISDICTION_ID: &str = "us";
/// Body string mandates + the regime row are scoped to.
pub const BODY: &str = "US House";

/// STOCK Act effective date (regime `effective_from`), proven valid at
/// compile time.
const EFFECTIVE_FROM: NaiveDate = match NaiveDate::from_ymd_opt(2012, 4, 4) {
    Some(date) => date,
    None => panic!("2012-04-04 is a valid date"),
};

/// Runner binding constants for `us_house`.
#[must_use]
pub fn regime_binding() -> RegimeBinding {
    RegimeBinding {
        regime_id: REGIME_ID.to_owned(),
        jurisdiction_id: JURISDICTION_ID.to_owned(),
        body: BODY.to_owned(),
    }
}

/// The `us_house` regime row per regime doc §1.
#[must_use]
pub fn regime_seed() -> RegimeSeed {
    RegimeSeed {
        jurisdiction: JurisdictionSeed {
            id: JURISDICTION_ID.to_owned(),
            name: "United States".to_owned(),
            iso_code: Some("US".to_owned()),
            level: "national".to_owned(),
        },
        regime_id: REGIME_ID.to_owned(),
        body: BODY.to_owned(),
        regime_type: "transaction_report".to_owned(),
        value_precision: "banded".to_owned(),
        cadence: Some(
            "rolling; statutory <=30d from notification, <=45d from transaction".to_owned(),
        ),
        disclosure_lag_days: Some(45),
        source_url: Some("https://disclosures-clerk.house.gov/FinancialDisclosure".to_owned()),
        effective_from: EFFECTIVE_FROM,
    }
}

/// Roster members from index XML (`Member` elements — live `{YYYY}FD.xml` or
/// the archived evidence slice). One entry per distinct `(as-filed name,
/// district)`; rows lacking name/district/year are skipped (the index blanks
/// them on some `W` rows, regime doc §2.2). The as-filed alias is
/// `Prefix First Last Suffix` — exactly what the PTR `Name:` header prints;
/// members the index lists without a prefix simply resolve (or fail closed)
/// on their prefix-less form.
///
/// # Errors
/// Unparseable XML, an unparseable `Year`, or an empty roster (fail closed).
pub fn roster_from_index_xml(xml: &str) -> anyhow::Result<Vec<RosterMember>> {
    let mut seen = std::collections::BTreeSet::new();
    let mut roster = Vec::new();
    for member in index::parse_index_xml(xml)? {
        if member.last.is_empty() || member.state_dst.is_empty() || member.year.is_empty() {
            continue;
        }
        let filed_alias = join_name(&[&member.prefix, &member.first, &member.last, &member.suffix]);
        if !seen.insert((filed_alias.clone(), member.state_dst.clone())) {
            continue; // the index repeats members across filings
        }
        let active_year: i32 = member
            .year
            .parse()
            .with_context(|| format!("index Year {:?} is not a number", member.year))?;
        roster.push(RosterMember {
            canonical_name: join_name(&[&member.first, &member.last, &member.suffix]),
            filed_alias,
            district: member.state_dst,
            role: "Representative".to_owned(),
            active_year,
        });
    }
    anyhow::ensure!(
        !roster.is_empty(),
        "no roster members in index XML — fail closed (invariant 6)"
    );
    Ok(roster)
}

/// Joins non-empty name parts with single spaces.
fn join_name(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|part| !part.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Historical roster seeding (goal 081 Task 1): loop `roster_from_index_xml` +
// `seed_roster` (both above, unchanged) over every archive year.
// ---------------------------------------------------------------------------

/// Where one archive year's index XML comes from for historical roster
/// seeding. [`LiveIndexSource`] shares `UsHouseAdapter`'s own conditional-GET
/// fetch — the SAME fetch [`crate::UsHouseAdapter::discover_year`] uses for
/// filing discovery, so a year's archive is fetched exactly once, never
/// twice (invariant 10). Tests inject a fixture-backed source instead
/// (mirrors `worker::backfill::ArchiveSource`'s per-year isolation,
/// `crates/worker/src/backfill.rs`).
#[async_trait]
pub trait IndexXmlSource: Send + Sync {
    /// The archive year's raw index XML, or `None` on a 304 (index
    /// unchanged since the last poll for this year).
    ///
    /// # Errors
    /// Transport failure or an unparseable historical index — the caller
    /// ([`seed_historical_rosters`]) fails that year closed and continues
    /// the range.
    async fn fetch_year(&self, year: i32) -> anyhow::Result<Option<String>>;
}

/// The live source: `UsHouseAdapter`'s own conditional-GET fetch.
pub struct LiveIndexSource<'a> {
    /// The adapter whose cached conditional-GET validators this shares.
    pub adapter: &'a UsHouseAdapter,
    /// The run context (HTTP client, politeness) to fetch through.
    pub ctx: &'a RunCtx,
}

#[async_trait]
impl IndexXmlSource for LiveIndexSource<'_> {
    async fn fetch_year(&self, year: i32) -> anyhow::Result<Option<String>> {
        self.adapter.fetch_index_xml(year, self.ctx).await
    }
}

/// One archive year's historical-roster-seeding outcome (goal 081 Task 1).
#[derive(Debug, Clone)]
pub struct YearSeedResult {
    /// The archive year.
    pub year: i32,
    /// Roster members newly inserted this year (0 on a 304, or an
    /// already-seeded replay).
    pub inserted: u32,
    /// Set when the YEAR itself failed — index unreachable/unparseable, or
    /// `seed_roster` bailed on an ambiguous roster. Fail closed per year,
    /// the range continues (mirrors `worker::backfill::dry_run`'s per-year
    /// isolation): an ambiguous roster on one year must not sink the rest.
    pub error: Option<String>,
}

/// Seeds the historical `us_house` roster across every archive year in
/// `from..=to` (design §5.4; Clerk index only — goal 081 research
/// findings). Loops the EXISTING, unchanged [`roster_from_index_xml`] +
/// `seed_roster` over each year's index (`index_zip_url(year)`, fetched via
/// `source`). Each year fails closed INDEPENDENTLY: an unreachable/
/// unparseable index or an ambiguous roster on one year is recorded in that
/// year's [`YearSeedResult`] and the sweep continues — never sinking the
/// rest of the range.
pub async fn seed_historical_rosters(
    source: &dyn IndexXmlSource,
    pool: &PgPool,
    regime: &RegimeBinding,
    from: i32,
    to: i32,
) -> Vec<YearSeedResult> {
    let mut results = Vec::new();
    for year in from..=to {
        results.push(match seed_one_year(source, pool, regime, year).await {
            Ok(inserted) => YearSeedResult {
                year,
                inserted,
                error: None,
            },
            Err(error) => YearSeedResult {
                year,
                inserted: 0,
                error: Some(format!("{error:#}")),
            },
        });
    }
    results
}

/// Fetches, rosters, and seeds ONE year — the unit [`seed_historical_rosters`]
/// isolates failures around.
async fn seed_one_year(
    source: &dyn IndexXmlSource,
    pool: &PgPool,
    regime: &RegimeBinding,
    year: i32,
) -> anyhow::Result<u32> {
    let Some(xml) = source.fetch_year(year).await? else {
        return Ok(0); // index unchanged since the last poll — nothing new
    };
    let roster = roster_from_index_xml(&xml)?;
    seed_roster(pool, regime, &roster).await
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const SLICE: &str = "<FinancialDisclosureSlice>\
        <Member><Prefix>Hon.</Prefix><Last>Begich</Last><First>Nicholas</First>\
          <Suffix>III</Suffix><FilingType>P</FilingType><StateDst>AK00</StateDst>\
          <Year>2026</Year><FilingDate>6/12/2026</FilingDate><DocID>20020055</DocID></Member>\
        <Member><Prefix>Hon.</Prefix><Last>Smucker</Last><First>Lloyd K.</First>\
          <Suffix /><FilingType>P</FilingType><StateDst>PA11</StateDst>\
          <Year>2026</Year><FilingDate>4/30/2026</FilingDate><DocID>20019182</DocID></Member>\
        <Member><Prefix>Hon.</Prefix><Last>Smucker</Last><First>Lloyd K.</First>\
          <Suffix /><FilingType>P</FilingType><StateDst>PA11</StateDst>\
          <Year>2026</Year><FilingDate>5/02/2026</FilingDate><DocID>20019999</DocID></Member>\
        <Member><Last>Blank</Last><FilingType>W</FilingType><StateDst></StateDst>\
          <Year>2026</Year><DocID>8068</DocID></Member>\
        </FinancialDisclosureSlice>";

    #[test]
    fn assembles_as_filed_names_and_dedups_members() {
        let roster = roster_from_index_xml(SLICE).unwrap();
        assert_eq!(roster.len(), 2, "repeat filings dedup; blank rows skipped");
        assert_eq!(roster[0].filed_alias, "Hon. Nicholas Begich III");
        assert_eq!(roster[0].canonical_name, "Nicholas Begich III");
        assert_eq!(roster[0].district, "AK00");
        assert_eq!(roster[0].active_year, 2026);
        assert_eq!(roster[1].filed_alias, "Hon. Lloyd K. Smucker");
        assert_eq!(roster[1].canonical_name, "Lloyd K. Smucker");
    }

    #[test]
    fn empty_index_fails_closed() {
        assert!(roster_from_index_xml("<FinancialDisclosure></FinancialDisclosure>").is_err());
    }

    #[test]
    fn regime_constants_match_the_manifest_pin() {
        let seed = regime_seed();
        assert_eq!(seed.regime_id, "0HSEREG0000000000000000001");
        assert_eq!(seed.body, "US House");
        assert_eq!(seed.effective_from.to_string(), "2012-04-04");
        let binding = regime_binding();
        assert_eq!(binding.regime_id, seed.regime_id);
        assert_eq!(binding.jurisdiction_id, seed.jurisdiction.id);
    }
}
