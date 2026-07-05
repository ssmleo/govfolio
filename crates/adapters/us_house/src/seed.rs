//! Production seed wiring for the `us_house` regime (plan Task 9): the
//! `disclosure_regime` row (regime doc §1 metadata) and roster seeding from
//! the Clerk's filing-index `Member` data — the official member list design
//! §5.4 prescribes. Offline runs seed from the archived index evidence slice
//! (`docs/regimes/us-house/evidence/`); live runs use the same parser on the
//! index zip XML.

use anyhow::Context as _;
use chrono::NaiveDate;

use pipeline::run::RegimeBinding;
use pipeline::stages::roster::RosterMember;
use pipeline::stages::seed::{JurisdictionSeed, RegimeSeed};

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
