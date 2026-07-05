//! Silver → Gold (regime doc §5.1): `record_type` + owner per grammar family,
//! `value` NULL always (§3.6), contract-typed `details` per `record_type`.
//! Unknown vocabulary is refused, never guessed (invariant 3). Supersession
//! stays NULL — Material Changes are independent notices with no source
//! linkage (§3.8).

use anyhow::Context as _;
use chrono::NaiveDate;

use govfolio_core::domain::enums::{AssetClass, Owner, RecordType};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::ids::{FilingId, PoliticianId, RegimeId};
use pipeline::adapter::{RunCtx, StagingRow};

use crate::details::{CanadaCiecChangeNotificationDetailsV1, CanadaCiecInterestDetailsV1};
use crate::parse::{SilverRow, parse_date_of_change};
use crate::tables::{self, Family};

/// Fixed conformance ULIDs (fixtures `MANIFEST.json` `conformance_ids`): details
/// pages carry no ULIDs and conformance runs with `pool: None`, so these
/// constants ARE the contract. Production resolves real ids from Postgres via
/// the clientId roster join (runner-binding follow-up, regime doc §2.4).
const CONFORMANCE_REGIME_ID: &str = "0CACREG0000000000000000001";
/// Filing ULIDs embed the declarationId's first hex group, uppercased (hex
/// digits are Crockford-valid), zero-padded to 19 — MANIFEST rule.
const CONFORMANCE_FILING_PREFIX: &str = "0CACFNG";
/// Source `clientId` GUID (lowercase) → politician ULID (MANIFEST
/// `politician_key_note`; §2.4 resolution input, keyed by clientId).
const CONFORMANCE_POLITICIANS: &[(&str, &str)] = &[
    (
        "5b99c2bd-7b2a-f011-8195-001dd8b72449",
        "0CACMBR0000000000000000001",
    ),
    (
        "1c26de25-482b-f011-8195-001dd8b72449",
        "0CACMBR0000000000000000002",
    ),
    (
        "f30de0ad-2778-e511-bec6-002655368060",
        "0CACMBR0000000000000000003",
    ),
    (
        "f0f4e0ff-7b2a-f011-8195-001dd8b72449",
        "0CACMBR0000000000000000004",
    ),
    (
        "9afbf0c2-172c-f011-8195-001dd8b72449",
        "0CACMBR0000000000000000005",
    ),
];

/// Nil ULID: the "identity not yet bound" marker for pool-backed runs.
const UNBOUND_ID: &str = "00000000000000000000000000";

/// Identity binding mode (`us_house`/`us_senate` pattern): conformance runs
/// (`pool: None`) emit the fixed MANIFEST ULIDs the fixtures pin; pool-backed
/// runs emit UNBOUND identity (nil ULIDs) for the runner's publish stage to
/// bind from Postgres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdentityMode {
    Conformance,
    Unbound,
}

/// Normalizes staged rows into Gold candidates.
pub(crate) fn normalize_rows(
    rows: &[StagingRow],
    ctx: &RunCtx,
) -> anyhow::Result<Vec<GoldCandidate>> {
    let mode = if ctx.pool.is_some() {
        IdentityMode::Unbound
    } else {
        IdentityMode::Conformance
    };
    rows.iter().map(|row| normalize_row(row, mode)).collect()
}

fn normalize_row(staged: &StagingRow, mode: IdentityMode) -> anyhow::Result<GoldCandidate> {
    let row: SilverRow = serde_json::from_value(staged.payload.clone())
        .context("silver payload is not a canada_ciec staging row")?;

    let family = tables::family_for_type(&row.declaration_type_raw).with_context(|| {
        format!(
            "declaration type {:?} is outside the v1 census — freeze (§3.1)",
            row.declaration_type_raw
        )
    })?;
    let law = tables::law_code(&row.law_raw).with_context(|| {
        format!(
            "Regime {:?} outside the Act/Code binary (§3.10)",
            row.law_raw
        )
    })?;
    let is_material_change = tables::is_material_change(&row.declaration_type_raw);
    let record_type = if is_material_change {
        RecordType::ChangeNotification
    } else {
        RecordType::Interest
    };

    // §5.1: asset_description source + §3.5 owner map, by grammar family.
    let (asset_description_raw, owner) = describe(&row, family)?;
    anyhow::ensure!(
        !asset_description_raw.is_empty(),
        "empty asset_description_raw for {} — reject (invariant 2)",
        row.declaration_id
    );

    let interest = build_interest(&row, law);
    let details = if is_material_change {
        // §3.7: normalized ISO date_of_change; fail-soft (raw survives in text).
        let date_of_change = row
            .description_raw
            .as_deref()
            .and_then(parse_date_of_change)
            .map(|d| d.to_string());
        serde_json::to_value(CanadaCiecChangeNotificationDetailsV1 {
            interest,
            date_of_change,
        })
    } else {
        serde_json::to_value(interest)
    }
    .context("serializing details")?;

    let notified_date = NaiveDate::parse_from_str(&row.disclosure_date_raw, "%Y-%m-%d")
        .with_context(|| format!("unparseable Disclosure date {:?}", row.disclosure_date_raw))?;

    let (filing_id, politician_id, regime_id) = resolve_ids(mode, &row)?;

    Ok(GoldCandidate {
        filing_id,
        politician_id,
        regime_id,
        instrument_id: None, // free-text corporations stay below threshold (§3.5, invariant 3)
        asset_description_raw,
        record_type,
        asset_class: AssetClass::Other, // §5.1: no structured asset typing anywhere
        side: None,
        transaction_date: None,
        as_of_date: None,
        notified_date: Some(notified_date),
        value: None, // §3.6: NULL always, by statute
        owner,
        extraction_confidence: Some(staged.confidence),
        extracted_by: row.extractor.clone(),
        fingerprint: None, // computed at promotion (Task 6)
        details,
    })
}

/// Builds the shared interest details surface from a staging row (regime doc §5).
fn build_interest(row: &SilverRow, law: &str) -> CanadaCiecInterestDetailsV1 {
    let (travel_start, travel_end) = row
        .fields_raw
        .get("Dates")
        .map_or((None, None), |d| parse_travel_range(d));
    CanadaCiecInterestDetailsV1 {
        declaration_id: row.declaration_id.clone(),
        row_ordinal: row.row_ordinal,
        item_id: row.item_id_raw.clone(),
        section_label: row.section_label_raw.clone(),
        declaration_type_raw: row.declaration_type_raw.clone(),
        law: law.to_owned(),
        h1_title: row.h1_title_raw.clone(),
        client_id: row.client_id.clone(),
        client_title: row.client_title_raw.clone(),
        no_longer_applicable: row.no_longer_applicable,
        ociec_translation: row.ociec_translation,
        language: "en".to_owned(),
        gift_received_date: row.fields_raw.get("Gift received date").cloned(),
        gift_source: row.fields_raw.get("Source").cloned(),
        gift_circumstance: row.fields_raw.get("Circumstance").cloned(),
        travel_destination: row.fields_raw.get("Destination").cloned(),
        travel_sponsor: row.fields_raw.get("Sponsor").cloned(),
        travel_dates_raw: row.fields_raw.get("Dates").cloned(),
        travel_start,
        travel_end,
        fields: row.fields_raw.clone(),
    }
}

/// Resolves the three identity ULIDs per mode (regime doc §2.4; MANIFEST
/// `conformance_ids`): fixed constants in conformance, nil (unbound) with a pool.
fn resolve_ids(
    mode: IdentityMode,
    row: &SilverRow,
) -> anyhow::Result<(FilingId, PoliticianId, RegimeId)> {
    match mode {
        IdentityMode::Conformance => Ok((
            conformance_filing_id(&row.declaration_id)?,
            conformance_politician_id(&row.client_id)?,
            CONFORMANCE_REGIME_ID
                .parse::<RegimeId>()
                .map_err(|e| anyhow::anyhow!("conformance regime id: {e}"))?,
        )),
        IdentityMode::Unbound => Ok((
            unbound_id::<FilingId>("filing")?,
            unbound_id::<PoliticianId>("politician")?,
            unbound_id::<RegimeId>("regime")?,
        )),
    }
}

/// §5.1 `asset_description` + §3.5 owner map, by grammar family.
fn describe(row: &SilverRow, family: Family) -> anyhow::Result<(String, Option<Owner>)> {
    match family {
        Family::Flat => Ok((flat_description(row)?, Some(Owner::Self_))),
        Family::Itemized => {
            let label = row
                .section_label_raw
                .as_deref()
                .context("family-C row without a section label — reject")?;
            let (owner, _) = tables::family_c_owner(label)
                .with_context(|| format!("section label {label:?} outside the grammar (§3.5)"))?;
            Ok((flat_description(row)?, Some(owner)))
        }
        Family::TypedFields => {
            if tables::is_sponsored_travel(&row.declaration_type_raw) {
                // A′: Purpose is the description; owner self (§3.5).
                Ok((field(row, "Purpose")?, Some(Owner::Self_)))
            } else {
                // Gifts/Forfeited: Nature is the description; owner NULL (§3.5).
                Ok((field(row, "Nature")?, None))
            }
        }
    }
}

/// The `description_raw` of a flat/itemized row (must be present).
fn flat_description(row: &SilverRow) -> anyhow::Result<String> {
    row.description_raw.clone().with_context(|| {
        format!(
            "row for {} has no description text — reject",
            row.declaration_id
        )
    })
}

/// A required typed field verbatim (Nature/Purpose); absent/empty is a reject.
fn field(row: &SilverRow, key: &str) -> anyhow::Result<String> {
    let value = row
        .fields_raw
        .get(key)
        .with_context(|| format!("typed declaration missing {key:?} field — reject"))?;
    anyhow::ensure!(
        !value.is_empty(),
        "empty {key:?} field — reject (invariant 2)"
    );
    Ok(value.clone())
}

/// Best-effort travel-range parse (fail-soft, regime doc §3.7): the `Dates`
/// value is `YYYY-MM-DD – YYYY-MM-DD (N days)` with an en/em dash separator;
/// `(None, None)` on any deviation (the verbatim string survives in
/// `travel_dates_raw`). The ASCII hyphen is NOT a separator — it is internal
/// to the ISO dates.
fn parse_travel_range(dates: &str) -> (Option<String>, Option<String>) {
    let range = dates.split('(').next().unwrap_or(dates);
    for sep in ['\u{2013}', '\u{2014}'] {
        if let Some((start, end)) = range.split_once(sep)
            && let (Ok(s), Ok(e)) = (
                NaiveDate::parse_from_str(start.trim(), "%Y-%m-%d"),
                NaiveDate::parse_from_str(end.trim(), "%Y-%m-%d"),
            )
        {
            return (Some(s.to_string()), Some(e.to_string()));
        }
    }
    (None, None)
}

/// Nil-ULID placeholder for a not-yet-bound identity field.
fn unbound_id<T: std::str::FromStr>(what: &str) -> anyhow::Result<T>
where
    T::Err: std::fmt::Display,
{
    UNBOUND_ID
        .parse()
        .map_err(|e: T::Err| anyhow::anyhow!("unbound {what} id: {e}"))
}

/// Conformance filing ULID: prefix + the declarationId's first hex group,
/// uppercased and zero-padded to 19 (MANIFEST rule).
fn conformance_filing_id(declaration_id: &str) -> anyhow::Result<FilingId> {
    let group = declaration_id
        .split('-')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    anyhow::ensure!(
        group.len() == 8 && group.bytes().all(|b| b.is_ascii_hexdigit()),
        "declaration id {declaration_id:?} cannot embed into a conformance filing ULID"
    );
    format!("{CONFORMANCE_FILING_PREFIX}{group:0>19}")
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance filing id for {declaration_id:?}: {e}"))
}

/// Fixed conformance politician ULID keyed by the source `clientId`; an
/// unknown filer is refused (extend the MANIFEST table — never guess).
fn conformance_politician_id(client_id: &str) -> anyhow::Result<PoliticianId> {
    CONFORMANCE_POLITICIANS
        .iter()
        .find(|(known, _)| *known == client_id)
        .with_context(|| {
            format!("no conformance politician id for clientId {client_id:?} — never guess")
        })?
        .1
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance politician id for {client_id:?}: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn manifest_ulid_constants_are_valid_crockford() {
        assert_eq!(
            conformance_filing_id("30c94327-3108-f111-81a2-001dd8b72449")
                .unwrap()
                .to_string(),
            "0CACFNG0000000000030C94327"
        );
        assert_eq!(
            conformance_filing_id("a4542986-719d-f011-819d-001dd8b72449")
                .unwrap()
                .to_string(),
            "0CACFNG00000000000A4542986"
        );
        assert_eq!(
            conformance_politician_id("f0f4e0ff-7b2a-f011-8195-001dd8b72449")
                .unwrap()
                .to_string(),
            "0CACMBR0000000000000000004"
        );
        assert_eq!(
            CONFORMANCE_REGIME_ID
                .parse::<RegimeId>()
                .unwrap()
                .to_string(),
            CONFORMANCE_REGIME_ID
        );
    }

    #[test]
    fn unknown_filers_are_refused_not_guessed() {
        assert!(conformance_politician_id("00000000-0000-0000-0000-000000000000").is_err());
    }

    #[test]
    fn travel_range_parses_en_dash_and_fails_soft() {
        assert_eq!(
            parse_travel_range("2026-01-05 \u{2013} 2026-01-09 (5 days)"),
            (Some("2026-01-05".to_owned()), Some("2026-01-09".to_owned()))
        );
        // ASCII hyphen alone is not a range separator — fail soft.
        assert_eq!(parse_travel_range("2026-01-05 to 2026-01-09"), (None, None));
    }
}
