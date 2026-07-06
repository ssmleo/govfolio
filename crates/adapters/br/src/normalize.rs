//! Silver → Gold (plan.md field-mapping table): every row is
//! `record_type = holding` with `side`/`transaction_date`/`notified_date`
//! NULL (`GoldCandidate::validate` requires only `as_of_date` for
//! `RecordType::Holding` — `core::domain::gold`); `owner` NULL (no
//! self/spouse/dependent concept in this regime, plan.md); `instrument_id`
//! NULL (invariant 3: no ISIN/ticker in source); `fingerprint` NULL
//! (computed at promotion, over row CONTENT per plan.md edge case 2 —
//! `DT_ULT_ATUAL_BEM_CANDIDATO` is a bulk backend re-timestamp artifact, not
//! a genuine per-candidate rectification signal, so it must never itself be
//! the fingerprint/supersession trigger).

use anyhow::Context as _;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use govfolio_core::domain::enums::{AssetClass, Currency, RecordType};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use govfolio_core::ids::{FilingId, PoliticianId, RegimeId};
use pipeline::adapter::{RunCtx, StagingRow};

use crate::details::BrHoldingDetailsV1;
use crate::parse::SilverRow;
use crate::tables::asset_class_for_code;

/// Fail-closed default bucket + confidence penalty (plan.md edge case 3) for
/// an asset-type code outside the resolved 5-code table. Never exercised by
/// the 3 committed fixtures (all 5 observed codes are pinned `Some(...)`
/// entries) — guards a code TSE ships later from silently landing as a
/// full-confidence `Other` guess.
const UNMAPPED_ASSET_CLASS_PENALTY: f32 = 0.10;

/// Fixed conformance ULIDs (fixtures `MANIFEST.json` `conformance_ids`):
/// fixtures carry no ULIDs and conformance runs with `pool: None`, so these
/// constants ARE the contract in conformance mode (`uk_commons_register`/
/// `us_house` precedent). Production resolves real ids from Postgres.
const CONFORMANCE_REGIME_ID: &str = "0BRAREG0000000000000000001";
const CONFORMANCE_FILING_PREFIX: &str = "0BRAFNG";
/// `SQ_CANDIDATO` -> politician ULID (MANIFEST `politician_ids`). Keyed on
/// the per-cycle join key itself, per the pinned fixture table — not a
/// durable cross-cycle identifier (AUTHORITY.md `identifiers_available`).
const CONFORMANCE_POLITICIANS: &[(&str, &str)] = &[
    ("10001595344", "0BRAMBR0000000000000000001"),
    ("20001716829", "0BRAMBR0000000000000000002"),
    ("10001606131", "0BRAMBR0000000000000000003"),
];

/// Identity binding mode (`us_house`/`uk_commons_register` pattern):
/// conformance runs (`pool: None`) emit the fixed MANIFEST ULID constants
/// the fixtures pin; pool-backed runs emit UNBOUND identity (nil ULIDs) for
/// the runner's publish stage to bind from Postgres.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdentityMode {
    Conformance,
    Unbound,
}

/// Nil ULID: the "identity not yet bound" marker for pool-backed runs.
const UNBOUND_ID: &str = "00000000000000000000000000";

/// Normalizes staged rows into Gold candidates.
///
/// # Errors
/// A payload outside this adapter's Silver shape, an unparseable date/value,
/// or (conformance mode only) an unrecognized `SQ_CANDIDATO`/regime id.
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
        .context("silver payload is not a br staging row")?;

    let (asset_class, confidence_penalty) = match asset_class_for_code(&row.asset_type_code_raw) {
        Some(class) => (class, 0.0),
        None => (AssetClass::Other, UNMAPPED_ASSET_CLASS_PENALTY),
    };

    let as_of_date = parse_br_date(&row.dt_eleicao_raw)?;
    let election_year: u16 = row
        .election_year_raw
        .parse()
        .with_context(|| format!("ANO_ELEICAO {:?} is not a year", row.election_year_raw))?;
    let line_item_ordinal: u32 = row.line_item_ordinal_raw.parse().with_context(|| {
        format!(
            "NR_ORDEM_BEM_CANDIDATO {:?} is not an ordinal",
            row.line_item_ordinal_raw
        )
    })?;
    let value = parse_brl_value(&row.value_raw)?;

    let details = BrHoldingDetailsV1 {
        asset_type_code_raw: row.asset_type_code_raw.clone(),
        asset_type_label_raw: row.asset_type_label_raw.clone(),
        asset_class,
        asset_description_raw: row.asset_description_raw.clone(),
        value_raw: row.value_raw.clone(),
        election_year,
        line_item_ordinal,
        last_updated_date_raw: row.last_updated_date_raw.clone(),
        last_updated_time_raw: row.last_updated_time_raw.clone(),
    };

    let (filing_id, politician_id, regime_id) = match mode {
        IdentityMode::Conformance => (
            conformance_filing_id(election_year, &row.sq_candidato)?,
            conformance_politician_id(&row.sq_candidato)?,
            CONFORMANCE_REGIME_ID
                .parse::<RegimeId>()
                .map_err(|e| anyhow::anyhow!("conformance regime id: {e}"))?,
        ),
        IdentityMode::Unbound => (
            unbound_id::<FilingId>("filing")?,
            unbound_id::<PoliticianId>("politician")?,
            unbound_id::<RegimeId>("regime")?,
        ),
    };

    let extraction_confidence = (staged.confidence - confidence_penalty).max(0.0);

    Ok(GoldCandidate {
        filing_id,
        politician_id,
        regime_id,
        instrument_id: None, // invariant 3: no ISIN/ticker in source, never guessed
        asset_description_raw: row.asset_description_raw.clone(),
        record_type: RecordType::Holding,
        asset_class,
        side: None,
        transaction_date: None,
        as_of_date: Some(as_of_date),
        notified_date: None,
        value: Some(value),
        owner: None, // no self/spouse/dependent concept in this regime (plan.md)
        extraction_confidence: Some(extraction_confidence),
        extracted_by: row.extractor.clone(),
        // Computed at promotion over (filing_id, ordinal, content) — plan.md
        // edge case 2, not the DT_ULT_ATUAL_BEM_CANDIDATO bulk-retimestamp
        // artifact.
        fingerprint: None,
        details: serde_json::to_value(&details).context("serializing details")?,
    })
}

/// `DD/MM/YYYY` (plan.md "Date parsing" — first non-ISO date format in this
/// codebase). `pub(crate)`: also used by `crate::binding` to derive
/// `RunnerBinding::filing_identity()`'s `filed_date` from `dt_eleicao_raw`
/// (this task's own instruction — mirrors `us_house::normalize::parse_source_date`).
pub(crate) fn parse_br_date(raw: &str) -> anyhow::Result<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%d/%m/%Y")
        .with_context(|| format!("unparseable DD/MM/YYYY date {raw:?}"))
}

/// Comma-decimal BRL string -> exact `ValueInterval` (`low == high`,
/// `AUTHORITY.md value_precision: "exact"`, plan.md "Value parsing"): strip
/// `.` thousands separators defensively (none observed in the 9 sampled
/// line items, but plausible at scale — plan.md edge case 9), replace `,`
/// with `.`, parse `rust_decimal` (invariant 7 — never a float).
fn parse_brl_value(raw: &str) -> anyhow::Result<ValueInterval> {
    let normalized = raw.replace('.', "").replace(',', ".");
    let amount: Decimal = normalized
        .parse()
        .with_context(|| format!("VR_BEM_CANDIDATO {raw:?} is not a decimal amount"))?;
    ValueInterval::new(amount, Some(amount), Currency::BRL)
        .map_err(|e| anyhow::anyhow!("VR_BEM_CANDIDATO {raw:?} builds an invalid interval: {e}"))
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

/// Conformance filing ULID: prefix + `{election_year}{sq_candidato}`
/// zero-padded to 19 (MANIFEST rule — `SQ_CANDIDATO` alone is only unique
/// within one cycle's file set, plan.md field-mapping table).
fn conformance_filing_id(election_year: u16, sq_candidato: &str) -> anyhow::Result<FilingId> {
    let key = format!("{election_year}{sq_candidato}");
    format!("{CONFORMANCE_FILING_PREFIX}{key:0>19}")
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance filing id for {sq_candidato}: {e}"))
}

/// Fixed conformance politician ULID keyed by `SQ_CANDIDATO`; unknown
/// candidates are refused (extend the MANIFEST table — never guess).
fn conformance_politician_id(sq_candidato: &str) -> anyhow::Result<PoliticianId> {
    CONFORMANCE_POLITICIANS
        .iter()
        .find(|(known, _)| *known == sq_candidato)
        .with_context(|| {
            format!("no conformance politician id for SQ_CANDIDATO {sq_candidato} — never guess")
        })?
        .1
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance politician id for {sq_candidato}: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn staged(payload: serde_json::Value) -> StagingRow {
        StagingRow {
            payload,
            confidence: 1.0,
        }
    }

    fn house_payload() -> serde_json::Value {
        json!({
            "sq_candidato": "10001595344",
            "nm_candidato": "ROGÉRIO DA SILVA E SILVA",
            "sg_uf": "AC",
            "dt_eleicao_raw": "02/10/2022",
            "election_year_raw": "2022",
            "line_item_ordinal_raw": "1",
            "asset_type_code_raw": "12",
            "asset_type_label_raw": "Casa",
            "asset_description_raw": "Casa na zona rural de xapuri",
            "value_raw": "10000,00",
            "last_updated_date_raw": "02/10/2022",
            "last_updated_time_raw": "23:21:28",
            "extractor": "br_bem_candidato/csv@1"
        })
    }

    #[test]
    fn typical_holding_maps_per_plan_md() {
        let candidate = normalize_row(&staged(house_payload()), IdentityMode::Conformance).unwrap();
        candidate.validate().unwrap();
        assert_eq!(candidate.record_type, RecordType::Holding);
        assert_eq!(candidate.side, None);
        assert_eq!(candidate.transaction_date, None);
        assert_eq!(candidate.notified_date, None);
        assert_eq!(candidate.owner, None);
        assert_eq!(candidate.instrument_id, None);
        assert_eq!(candidate.fingerprint, None);
        assert_eq!(
            candidate.as_of_date,
            Some(NaiveDate::from_ymd_opt(2022, 10, 2).unwrap())
        );
        assert_eq!(candidate.asset_class, AssetClass::RealEstate);
        let value = candidate.value.unwrap();
        assert_eq!(value.low(), rust_decimal_macros::dec!(10000.00));
        assert_eq!(value.high(), Some(rust_decimal_macros::dec!(10000.00)));
        assert_eq!(value.currency(), Currency::BRL);
        assert_eq!(
            candidate.filing_id.to_string(),
            "0BRAFNG0000202210001595344"
        );
        assert_eq!(
            candidate.politician_id.to_string(),
            "0BRAMBR0000000000000000001"
        );
    }

    #[test]
    fn unmapped_asset_code_fails_closed_to_other_with_a_penalty() {
        let mut payload = house_payload();
        payload["asset_type_code_raw"] = json!("61");
        let candidate = normalize_row(&staged(payload), IdentityMode::Conformance).unwrap();
        assert_eq!(candidate.asset_class, AssetClass::Other);
        assert_eq!(
            candidate.details["asset_class"],
            json!("other"),
            "details echoes the same fail-closed bucket"
        );
        assert!(
            (candidate.extraction_confidence.unwrap() - 0.90).abs() < f32::EPSILON,
            "confidence penalty must apply: {:?}",
            candidate.extraction_confidence
        );
    }

    #[test]
    fn unknown_candidate_is_refused_not_guessed() {
        let mut payload = house_payload();
        payload["sq_candidato"] = json!("99999999999");
        assert!(normalize_row(&staged(payload), IdentityMode::Conformance).is_err());
    }

    #[test]
    fn pool_backed_mode_emits_unbound_identity() {
        let candidate = normalize_row(&staged(house_payload()), IdentityMode::Unbound).unwrap();
        assert_eq!(candidate.filing_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.politician_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.regime_id.to_string(), UNBOUND_ID);
    }

    #[test]
    fn thousands_separated_value_parses_defensively() {
        assert_eq!(
            parse_brl_value("1.500.000,00").unwrap().low(),
            rust_decimal_macros::dec!(1500000.00)
        );
    }
}
