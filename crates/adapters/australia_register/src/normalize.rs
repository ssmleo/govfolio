//! Silver → Gold (regime doc §5.1): `record_type` per source section
//! (`interest` for Form A entries, `change_notification` for alterations),
//! owner per band/marker (§3.5), `value` NULL always (§3.6), day-first dates
//! (§3.7), contract-typed `details` per `record_type` (invariant 5). Unknown
//! vocabulary is refused, never guessed (invariant 3). Supersession stays NULL
//! — reissues arrive as new documents, not row edits (§3.8).

use std::collections::BTreeMap;

use anyhow::Context as _;
use chrono::NaiveDate;
use serde::Deserialize;

use govfolio_core::domain::enums::{AssetClass, Owner, RecordType};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::ids::{FilingId, PoliticianId, RegimeId};
use pipeline::adapter::{RunCtx, StagingRow};

use crate::details::{
    AdditionDeletion, AustraliaRegisterChangeNotificationDetailsV1,
    AustraliaRegisterInterestDetailsV1, OwnerBand, SourceFlavour,
};

/// Fixed conformance ULIDs (fixtures `MANIFEST.json` `conformance_ids`): the
/// scanned PDFs carry no ids and conformance runs with `pool: None`, so these
/// constants ARE the contract. Production resolves real ids from Postgres via
/// the `(electoral division, state)` roster join (runner-binding follow-up,
/// regime doc §2.4).
const CONFORMANCE_REGIME_ID: &str = "0AHRREG0000000000000000001";

/// Member document filename → conformance filing ULID (MANIFEST `filing_ids`,
/// version elided — the fixtures pin one version per member).
const CONFORMANCE_FILINGS: &[(&str, &str)] = &[
    ("Buchholz_48P", "0AHRFNG0000000000000000001"),
    ("Chalmers_48P", "0AHRFNG0000000000000000002"),
    ("Albanese_48P", "0AHRFNG0000000000000000003"),
    ("Katter_48P", "0AHRFNG0000000000000000004"),
];

/// Electoral division (UPPERCASE, the §2.4 stable House seat key) → conformance
/// politician ULID (MANIFEST `politician_ids`). Division is unique across the
/// four fixtures; state abbreviation varies as-filed (`QLD` vs `Queensland`).
const CONFORMANCE_POLITICIANS: &[(&str, &str)] = &[
    ("WRIGHT", "0AHRMBR0000000000000000001"),
    ("RANKIN", "0AHRMBR0000000000000000002"),
    ("GRAYNDLER", "0AHRMBR0000000000000000003"),
    ("KENNEDY", "0AHRMBR0000000000000000004"),
];

/// Nil ULID: the "identity not yet bound" marker for pool-backed runs.
const UNBOUND_ID: &str = "00000000000000000000000000";

/// Wrapper-confidence floor above which a document is the clean text-layer
/// flavour (regime doc §3.9 / MANIFEST `confidence_literals`: `text_layer` rows
/// carry `1.0`, `scanned_vision` rows `0.98`).
const TEXT_LAYER_CONFIDENCE_FLOOR: f32 = 0.995;

/// One Silver staging row (regime doc §4). Only the fields Gold consumes are
/// deserialized; the extra header fields (`family_name_raw`, `given_names_raw`)
/// ride along in the payload for audit but are not read here.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct SilverRow {
    pub(crate) document_filename: String,
    pub(crate) parliament_no: u32,
    pub(crate) row_ordinal: u32,
    pub(crate) page_ordinal: u32,
    pub(crate) section_kind: String,
    pub(crate) category_number: u32,
    pub(crate) category_name_raw: String,
    pub(crate) owner_band_raw: Option<String>,
    pub(crate) addition_deletion_raw: Option<String>,
    pub(crate) electoral_division_raw: String,
    pub(crate) state_raw: String,
    pub(crate) entry_text_raw: String,
    pub(crate) entry_fields_raw: BTreeMap<String, String>,
    pub(crate) date_raw: Option<String>,
    pub(crate) parliament_stamp_raw: Option<String>,
    pub(crate) extractor: String,
}

/// Identity binding mode (`canada_ciec`/`us_house` pattern): conformance runs
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
        .context("silver payload is not an australia_register staging row")?;

    anyhow::ensure!(
        !row.entry_text_raw.is_empty(),
        "empty entry_text_raw at row {} of {} — reject (invariant 2)",
        row.row_ordinal,
        row.document_filename
    );
    anyhow::ensure!(
        (1..=14).contains(&row.category_number),
        "category number {} outside the 1..14 census (§3.10 check 2) — freeze",
        row.category_number
    );

    let is_alteration = match row.section_kind.as_str() {
        "statement" => false,
        "alteration" => true,
        other => anyhow::bail!("unknown section_kind {other:?} (§3.4) — freeze"),
    };
    let record_type = if is_alteration {
        RecordType::ChangeNotification
    } else {
        RecordType::Interest
    };

    let source_flavour = if staged.confidence >= TEXT_LAYER_CONFIDENCE_FLOOR {
        SourceFlavour::TextLayer
    } else {
        SourceFlavour::ScannedVision
    };

    let owner_band = resolve_owner_band(&row, is_alteration)?;
    let notified_date = row.date_raw.as_deref().and_then(parse_au_date);

    // Interest surface: statement rows carry the completion date here;
    // alteration rows carry it in submitted_date instead (§3.7).
    let interest = AustraliaRegisterInterestDetailsV1 {
        document_filename: row.document_filename.clone(),
        parliament_no: row.parliament_no,
        row_ordinal: row.row_ordinal,
        page_ordinal: row.page_ordinal,
        category_number: row.category_number,
        category_name: row.category_name_raw.clone(),
        owner_band,
        electoral_division: row.electoral_division_raw.clone(),
        state: row.state_raw.clone(),
        entry_text: row.entry_text_raw.clone(),
        entry_fields: row.entry_fields_raw.clone(),
        statement_date: if is_alteration { None } else { notified_date },
        language: "en".to_owned(),
        source_flavour,
    };

    let details = if is_alteration {
        let addition_deletion = resolve_addition_deletion(row.addition_deletion_raw.as_deref())?;
        serde_json::to_value(AustraliaRegisterChangeNotificationDetailsV1 {
            interest,
            addition_deletion,
            submitted_date: notified_date,
            parliament_stamp: row.parliament_stamp_raw.clone(),
        })
    } else {
        serde_json::to_value(interest)
    }
    .context("serializing details")?;

    let (filing_id, politician_id, regime_id) = resolve_ids(mode, &row)?;

    Ok(GoldCandidate {
        filing_id,
        politician_id,
        regime_id,
        instrument_id: None, // company/trust free text stays below threshold (§3.5, invariant 3)
        asset_description_raw: row.entry_text_raw.clone(),
        record_type,
        asset_class: asset_class_for(row.category_number),
        side: None,
        transaction_date: None,
        as_of_date: None,
        notified_date: if is_alteration { notified_date } else { None },
        value: None, // §3.6: NULL always — the register is descriptive
        owner: owner_band.map(owner_of),
        extraction_confidence: Some(staged.confidence),
        extracted_by: row.extractor.clone(),
        fingerprint: None, // computed at promotion (Task 6)
        details,
    })
}

/// §3.2 category → `asset_class`: only cat 1 is `equity`, only cat 3 is
/// `real_estate`, everything else `other` (no creative bucketing).
fn asset_class_for(category_number: u32) -> AssetClass {
    match category_number {
        1 => AssetClass::Equity,
        3 => AssetClass::RealEstate,
        _ => AssetClass::Other,
    }
}

/// §3.5 owner map. Statement rows read the printed band; alteration rows have
/// no band and default to the Member (`self`), flipping to `spouse` only on a
/// documented `Spouse -`/`Spouse gift -` marker. Free text is never parsed for
/// owner beyond those markers (invariant 3 spirit).
fn resolve_owner_band(row: &SilverRow, is_alteration: bool) -> anyhow::Result<Option<OwnerBand>> {
    if is_alteration {
        let text = &row.entry_text_raw;
        let spouse = text.starts_with("Spouse -") || text.starts_with("Spouse gift -");
        return Ok(Some(if spouse {
            OwnerBand::Spouse
        } else {
            OwnerBand::Self_
        }));
    }
    match row.owner_band_raw.as_deref() {
        Some("Self") => Ok(Some(OwnerBand::Self_)),
        Some("Spouse/Partner") => Ok(Some(OwnerBand::Spouse)),
        Some("Dependent Children") => Ok(Some(OwnerBand::Dependent)),
        None => Ok(None), // ambiguous under vision — never guessed (§3.5)
        Some(other) => anyhow::bail!("unknown owner band {other:?} (§3.10 check 3) — review"),
    }
}

/// §3.3 alteration axis, normalized. A missing/unknown value on an alteration
/// row is a hard reject (§3.10 check 3), never guessed.
fn resolve_addition_deletion(raw: Option<&str>) -> anyhow::Result<AdditionDeletion> {
    match raw {
        Some("ADDITION") => Ok(AdditionDeletion::Addition),
        Some("DELETION") => Ok(AdditionDeletion::Deletion),
        other => {
            anyhow::bail!("alteration row addition_deletion {other:?} outside the axis — review")
        }
    }
}

/// `OwnerBand` → the core `owner` vocabulary.
fn owner_of(band: OwnerBand) -> Owner {
    match band {
        OwnerBand::Self_ => Owner::Self_,
        OwnerBand::Spouse => Owner::Spouse,
        OwnerBand::Dependent => Owner::Dependent,
    }
}

/// Day-first Australian date (`DD/MM/YYYY` or handwritten `D/M/YY`) → ISO;
/// fail-soft `None` on any deviation (the raw string survives in `date_raw` /
/// `entry_text`, regime doc §3.7). Contrast US month-first dates.
fn parse_au_date(raw: &str) -> Option<NaiveDate> {
    let mut parts = raw.trim().split('/');
    let day = parts.next()?.trim();
    let month = parts.next()?.trim();
    let year = parts.next()?.trim();
    if parts.next().is_some() {
        return None; // more than three components — not a plain date
    }
    let day: u32 = day.parse().ok()?;
    let month: u32 = month.parse().ok()?;
    let year_num: i32 = year.parse().ok()?;
    // Two-digit years are 21st century (the register's 48th-Parliament window).
    let year = if year.len() <= 2 {
        2000 + year_num
    } else {
        year_num
    };
    NaiveDate::from_ymd_opt(year, month, day)
}

/// Resolves the three identity ULIDs per mode (regime doc §2.4; MANIFEST
/// `conformance_ids`): fixed constants in conformance, nil (unbound) with a pool.
fn resolve_ids(
    mode: IdentityMode,
    row: &SilverRow,
) -> anyhow::Result<(FilingId, PoliticianId, RegimeId)> {
    match mode {
        IdentityMode::Conformance => Ok((
            conformance_filing_id(&row.document_filename)?,
            conformance_politician_id(&row.electoral_division_raw)?,
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

/// Nil-ULID placeholder for a not-yet-bound identity field.
fn unbound_id<T: std::str::FromStr>(what: &str) -> anyhow::Result<T>
where
    T::Err: std::fmt::Display,
{
    UNBOUND_ID
        .parse()
        .map_err(|e: T::Err| anyhow::anyhow!("unbound {what} id: {e}"))
}

/// Fixed conformance filing ULID keyed by the member document filename; an
/// unknown document is refused (extend the MANIFEST table — never guess).
fn conformance_filing_id(document_filename: &str) -> anyhow::Result<FilingId> {
    CONFORMANCE_FILINGS
        .iter()
        .find(|(name, _)| *name == document_filename)
        .with_context(|| {
            format!("no conformance filing id for {document_filename:?} — never guess")
        })?
        .1
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance filing id for {document_filename:?}: {e}"))
}

/// Fixed conformance politician ULID keyed by the electoral division (§2.4);
/// an unknown seat is refused (extend the MANIFEST table — never guess).
fn conformance_politician_id(electoral_division: &str) -> anyhow::Result<PoliticianId> {
    let key = electoral_division.to_ascii_uppercase();
    CONFORMANCE_POLITICIANS
        .iter()
        .find(|(division, _)| *division == key)
        .with_context(|| {
            format!(
                "no conformance politician id for division {electoral_division:?} — never guess"
            )
        })?
        .1
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance politician id for {electoral_division:?}: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn manifest_ulid_constants_are_valid_crockford() {
        assert_eq!(
            conformance_filing_id("Katter_48P").unwrap().to_string(),
            "0AHRFNG0000000000000000004"
        );
        assert_eq!(
            conformance_politician_id("GRAYNDLER").unwrap().to_string(),
            "0AHRMBR0000000000000000003"
        );
        // Case-insensitive on the seat key (Albanese's header is uppercase).
        assert_eq!(
            conformance_politician_id("Wright").unwrap().to_string(),
            "0AHRMBR0000000000000000001"
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
    fn unknown_document_or_seat_is_refused_not_guessed() {
        assert!(conformance_filing_id("Nobody_48P").is_err());
        assert!(conformance_politician_id("Nowhere").is_err());
    }

    #[test]
    fn au_dates_are_day_first_and_fail_soft() {
        assert_eq!(
            parse_au_date("25/02/2026"),
            NaiveDate::from_ymd_opt(2026, 2, 25)
        );
        assert_eq!(
            parse_au_date("18/08/2025"),
            NaiveDate::from_ymd_opt(2025, 8, 18)
        );
        // Handwritten two-digit year → 21st century.
        assert_eq!(
            parse_au_date("12/8/25"),
            NaiveDate::from_ymd_opt(2025, 8, 12)
        );
        // Garbled handwriting fails soft.
        assert_eq!(parse_au_date("3/41 Y6"), None);
        assert_eq!(parse_au_date("2026-02-25"), None);
    }

    #[test]
    fn asset_class_only_promotes_shares_and_real_estate() {
        assert_eq!(asset_class_for(1), AssetClass::Equity);
        assert_eq!(asset_class_for(3), AssetClass::RealEstate);
        for cat in [2, 6, 8, 9, 10, 11, 12, 13, 14] {
            assert_eq!(asset_class_for(cat), AssetClass::Other);
        }
    }

    #[test]
    fn alteration_owner_flips_to_spouse_only_on_a_marker() {
        let mut row = statement_row();
        row.section_kind = "alteration".to_owned();
        row.owner_band_raw = None;
        row.entry_text_raw = "General tickets to the show".to_owned();
        assert_eq!(
            resolve_owner_band(&row, true).unwrap(),
            Some(OwnerBand::Self_)
        );
        row.entry_text_raw = "Spouse - Pendant gift".to_owned();
        assert_eq!(
            resolve_owner_band(&row, true).unwrap(),
            Some(OwnerBand::Spouse)
        );
        row.entry_text_raw = "Spouse gift - Box".to_owned();
        assert_eq!(
            resolve_owner_band(&row, true).unwrap(),
            Some(OwnerBand::Spouse)
        );
    }

    #[test]
    fn statement_owner_reads_the_band_and_rejects_unknown() {
        let mut row = statement_row();
        assert_eq!(
            resolve_owner_band(&row, false).unwrap(),
            Some(OwnerBand::Self_)
        );
        row.owner_band_raw = Some("Spouse/Partner".to_owned());
        assert_eq!(
            resolve_owner_band(&row, false).unwrap(),
            Some(OwnerBand::Spouse)
        );
        row.owner_band_raw = Some("Nonsense".to_owned());
        assert!(resolve_owner_band(&row, false).is_err());
    }

    fn statement_row() -> SilverRow {
        SilverRow {
            document_filename: "Katter_48P".to_owned(),
            parliament_no: 48,
            row_ordinal: 1,
            page_ordinal: 2,
            section_kind: "statement".to_owned(),
            category_number: 1,
            category_name_raw: "Shareholdings in public and private companies".to_owned(),
            owner_band_raw: Some("Self".to_owned()),
            addition_deletion_raw: None,
            electoral_division_raw: "Kennedy".to_owned(),
            state_raw: "QLD".to_owned(),
            entry_text_raw: "A number of AMP shares".to_owned(),
            entry_fields_raw: BTreeMap::new(),
            date_raw: None,
            parliament_stamp_raw: None,
            extractor: "australia_register/llm@1".to_owned(),
        }
    }
}
