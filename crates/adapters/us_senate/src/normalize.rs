//! Silver → Gold (regime doc §5.1): closed-vocabulary maps, band →
//! [`ValueInterval`], amendment detection from the verbatim title,
//! contract-typed `details`. Unknown values are refused or bucketed exactly
//! as §3 prescribes — never guessed (invariant 3). Supersession stays NULL:
//! amendments are unlinked at the source (§3.6); the promotion machinery
//! opens `ptr_amendment_unlinked` review tasks at publish, mirroring the
//! `us_house` pattern.

use anyhow::Context as _;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use govfolio_core::domain::enums::{AssetClass, Currency, RecordType};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use govfolio_core::ids::{FilingId, PoliticianId, RegimeId};
use pipeline::adapter::{RunCtx, StagingRow};

use crate::details::UsSenatePtrTransactionDetailsV1;
use crate::parse::{SilverRow, title_parts};
use crate::tables;

/// Fixed conformance ULIDs (fixtures `MANIFEST.json`): view pages carry no
/// ids and conformance runs with `pool: None`, so these constants ARE the
/// contract in conformance mode. Production resolves real ids from Postgres
/// (runner-binding follow-up, `us_house` Task-9 pattern).
const CONFORMANCE_REGIME_ID: &str = "0SENREG0000000000000000001";
/// Filing ULIDs embed the report UUID's first hex group, uppercased (hex
/// digits are Crockford-valid), zero-padded to 19 — MANIFEST rule.
const CONFORMANCE_FILING_PREFIX: &str = "0SENFNG";
/// `h2.filedReport` parenthetical `(Last, First)` verbatim → politician ULID
/// (MANIFEST `politician_key_note`; §2.4 resolution input).
const CONFORMANCE_POLITICIANS: &[(&str, &str)] = &[
    ("Fetterman, John", "0SENMBR0000000000000000001"),
    ("Whitehouse, Sheldon", "0SENMBR0000000000000000002"),
    ("Boozman, John", "0SENMBR0000000000000000003"),
];

/// Identity binding mode (`us_house` pattern): conformance runs (`pool: None`)
/// emit the fixed MANIFEST ULID constants the fixtures pin; pool-backed runs
/// emit UNBOUND identity (nil ULIDs) for the runner's publish stage to bind
/// from Postgres. FK constraints reject a nil id if a bug ever let one through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdentityMode {
    Conformance,
    Unbound,
}

/// Nil ULID: the "identity not yet bound" marker for pool-backed runs.
const UNBOUND_ID: &str = "00000000000000000000000000";

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
        .context("silver payload is not a us_senate staging row")?;

    // §3.3 transaction-side map; parse already hard-rejected unknown tokens.
    let (side, partial_sale) =
        tables::side_for_type(&row.transaction_type_raw).with_context(|| {
            format!(
                "unknown transaction Type {:?} — never guess",
                row.transaction_type_raw
            )
        })?;

    // §3.2 owner map: full words, always populated, no default problem.
    let owner = tables::owner_for_word(&row.owner_raw)
        .with_context(|| format!("unknown Owner word {:?} — never guess", row.owner_raw))?;

    // §3.5: unknown vocabulary members bucket to `other` (parse already paid
    // the confidence penalty; publish routes them to review).
    let asset_class =
        tables::asset_class_for_type(&row.asset_type_raw).unwrap_or(AssetClass::Other);

    let value = band_interval(&row.amount_raw)?;
    let transaction_date = parse_mdy(&row.transaction_date_raw)?;
    // §3.6: amendment number from the verbatim title; supersession stays NULL.
    let (_, amendment_number) = title_parts(&row.report_title_raw)?;

    let details = UsSenatePtrTransactionDetailsV1 {
        report_uuid: row.report_uuid.clone(),
        row_ordinal: row.row_ordinal,
        row_number: row.row_number_raw.clone(),
        ticker: row.ticker_raw.clone(),
        asset_type_raw: row.asset_type_raw.clone(),
        asset_detail: row.asset_detail_raw.clone(),
        amount_band_raw: row.amount_raw.clone(),
        transaction_type_raw: row.transaction_type_raw.clone(),
        partial_sale,
        comment: row.comment_raw.clone(),
        amendment_number,
        filed_at_raw: row.filed_at_raw.clone(),
    };

    let (filing_id, politician_id, regime_id) = match mode {
        IdentityMode::Conformance => (
            conformance_filing_id(&row.report_uuid)?,
            conformance_politician_id(&row.filer_name_raw)?,
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

    Ok(GoldCandidate {
        filing_id,
        politician_id,
        regime_id,
        instrument_id: None, // resolution waterfall is design §5.4; never guess
        asset_description_raw: row.asset_name_raw.clone(),
        record_type: RecordType::Transaction,
        asset_class,
        side: Some(side),
        transaction_date: Some(transaction_date),
        as_of_date: None,
        // §5.1: the Senate electronic PTR has NO notification-date column.
        notified_date: None,
        value: Some(value),
        owner: Some(owner),
        extraction_confidence: Some(staged.confidence),
        extracted_by: row.extractor.clone(),
        // Computed at promotion over (filing_id, ordinal, content) — the
        // candidate ships without it (GoldCandidate contract).
        fingerprint: None,
        details: serde_json::to_value(&details).context("serializing details")?,
    })
}

/// §3.4: band string → decimal interval, bounds as scale-2 decimals, USD.
fn band_interval(amount_raw: &str) -> anyhow::Result<ValueInterval> {
    let (low, high) = tables::band_bounds(amount_raw)
        .with_context(|| format!("band {amount_raw:?} outside the grammar — hard reject"))?;
    let low: Decimal = low
        .parse()
        .map_err(|e| anyhow::anyhow!("band low {low:?}: {e}"))?;
    let high: Option<Decimal> = high
        .map(|h| {
            h.parse()
                .map_err(|e| anyhow::anyhow!("band high {h:?}: {e}"))
        })
        .transpose()?;
    ValueInterval::new(low, high, Currency::USD)
        .map_err(|e| anyhow::anyhow!("band {amount_raw:?}: {e}"))
}

/// `MM/DD/YYYY` as printed → [`NaiveDate`].
fn parse_mdy(raw: &str) -> anyhow::Result<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%m/%d/%Y").with_context(|| format!("unparseable date {raw:?}"))
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

/// Conformance filing ULID: prefix + the report UUID's first hex group,
/// uppercased and zero-padded to 19 (MANIFEST rule).
fn conformance_filing_id(report_uuid: &str) -> anyhow::Result<FilingId> {
    let group = report_uuid
        .split('-')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    anyhow::ensure!(
        group.len() == 8 && group.bytes().all(|b| b.is_ascii_hexdigit()),
        "report uuid {report_uuid:?} cannot embed into a conformance filing ULID"
    );
    format!("{CONFORMANCE_FILING_PREFIX}{group:0>19}")
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance filing id for {report_uuid:?}: {e}"))
}

/// Fixed conformance politician ULID keyed by the filer parenthetical;
/// unknown filers are refused (extend the MANIFEST table — never guess).
fn conformance_politician_id(filer_name_raw: &str) -> anyhow::Result<PoliticianId> {
    let key = filer_name_raw
        .strip_suffix(')')
        .and_then(|s| s.rsplit_once('('))
        .map(|(_, key)| key)
        .with_context(|| format!("filer {filer_name_raw:?} has no (Last, First) parenthetical"))?;
    CONFORMANCE_POLITICIANS
        .iter()
        .find(|(known, _)| *known == key)
        .with_context(|| format!("no conformance politician id for {key:?} — never guess"))?
        .1
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance politician id for {key:?}: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rust_decimal_macros::dec;

    use super::*;

    #[test]
    fn manifest_ulid_constants_are_valid_crockford() {
        assert_eq!(
            conformance_filing_id("4b69867f-0376-4526-93f2-cd556b1155c9")
                .unwrap()
                .to_string(),
            "0SENFNG000000000004B69867F"
        );
        assert_eq!(
            conformance_politician_id("The Honorable John Fetterman (Fetterman, John)")
                .unwrap()
                .to_string(),
            "0SENMBR0000000000000000001"
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
        assert!(conformance_politician_id("The Honorable No Body (Body, No)").is_err());
        assert!(conformance_politician_id("No parenthetical at all").is_err());
    }

    fn staged(payload: serde_json::Value) -> StagingRow {
        StagingRow {
            payload,
            confidence: 1.0,
        }
    }

    fn payload() -> serde_json::Value {
        serde_json::json!({
            "report_uuid": "727b4eb6-d8c7-4792-aa5b-c651c2d72f9c",
            "row_ordinal": 1,
            "row_number_raw": "18",
            "report_title_raw": "Periodic Transaction Report for 06/16/2026 (Amendment 1)",
            "filer_name_raw": "The Honorable John Boozman (Boozman, John)",
            "filed_at_raw": "06/16/2026 @ 10:27 AM",
            "owner_raw": "Joint",
            "ticker_raw": "VEA",
            "asset_name_raw": "Vanguard Developed Markets Index Fund - ETF Shares",
            "asset_detail_raw": null,
            "asset_type_raw": "Stock",
            "transaction_type_raw": "Sale (Partial)",
            "transaction_date_raw": "05/27/2026",
            "amount_raw": "$1,001 - $15,000",
            "comment_raw": null,
            "extractor": "us_senate_ptr/html@1"
        })
    }

    #[test]
    fn amendment_rows_carry_the_number_and_no_supersession_guess() {
        let candidate = normalize_row(&staged(payload()), IdentityMode::Conformance).unwrap();
        assert_eq!(candidate.details["amendment_number"], serde_json::json!(1));
        assert_eq!(candidate.details["partial_sale"], serde_json::json!(true));
        assert_eq!(
            candidate.notified_date, None,
            "no notification column (§5.1)"
        );
        assert_eq!(
            serde_json::to_value(candidate.owner).unwrap(),
            serde_json::json!("joint")
        );
        assert_eq!(
            candidate.filing_id.to_string(),
            "0SENFNG00000000000727B4EB6"
        );
    }

    #[test]
    fn pool_backed_mode_emits_unbound_identity() {
        let candidate = normalize_row(&staged(payload()), IdentityMode::Unbound).unwrap();
        assert_eq!(candidate.filing_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.politician_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.regime_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.fingerprint, None, "computed at publish");
    }

    #[test]
    fn band_interval_maps_open_and_closed_bands() {
        let closed = band_interval("$1,001 - $15,000").unwrap();
        assert_eq!(closed.low(), dec!(1001.00));
        assert_eq!(closed.high().unwrap(), dec!(15000.00));
        assert_eq!(closed.currency(), Currency::USD);
        let open = band_interval("Over $50,000,000").unwrap();
        assert_eq!(open.low(), dec!(50000000.00));
        assert_eq!(open.high(), None);
        assert!(band_interval("Over $1,000,000").is_err(), "outside grammar");
    }

    #[test]
    fn money_serializes_as_decimal_strings() {
        let value = serde_json::to_value(band_interval("$1,001 - $15,000").unwrap()).unwrap();
        assert_eq!(value["low"], serde_json::json!("1001.00"));
        assert_eq!(value["high"], serde_json::json!("15000.00"));
    }
}
