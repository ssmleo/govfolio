//! Silver → Gold (regime doc §5.1): closed-vocabulary maps, band →
//! [`ValueInterval`], owner inheritance provenance, contract-typed `details`.
//! Unknown codes are refused or bucketed exactly as §3 prescribes — never
//! guessed (invariant 3).

use anyhow::Context as _;
use chrono::NaiveDate;
use rust_decimal::Decimal;

use govfolio_core::domain::enums::{AssetClass, Currency, Owner, RecordType, Side};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use govfolio_core::ids::{FilingId, PoliticianId, RegimeId};
use pipeline::adapter::{RunCtx, StagingRow};

use crate::details::{OwnerSource, UsHousePtrTransactionDetailsV1};
use crate::parse::SilverRow;
use crate::tables;

/// Fixed conformance ULIDs (fixtures `MANIFEST.json`): the PDF carries no ids
/// and conformance runs with `pool: None`, so these constants ARE the contract
/// in conformance mode. Production resolves real ids from Postgres (Task 9).
const CONFORMANCE_REGIME_ID: &str = "0HSEREG0000000000000000001";
/// Filing ULIDs embed the `DocID` digits for auditability: prefix + zero-padded `DocID`.
const CONFORMANCE_FILING_PREFIX: &str = "0HSEFNG";
/// `filer_name_raw|state_district_raw` → politician ULID.
const CONFORMANCE_POLITICIANS: &[(&str, &str)] = &[
    (
        "Hon. Nicholas Begich III|AK00",
        "0HSEMBR0000000000000000001",
    ),
    ("Hon. Lloyd K. Smucker|PA11", "0HSEMBR0000000000000000002"),
    ("Hon. David Rouzer|NC07", "0HSEMBR0000000000000000003"),
    ("Hon. Nancy Pelosi|CA11", "0HSEMBR0000000000000000004"),
];

/// Identity binding mode (plan Task 9, closing the T8c seam).
///
/// Conformance runs (`pool: None`) emit the fixed MANIFEST ULID constants the
/// fixtures pin. Pool-backed runs emit UNBOUND identity (nil ULIDs): the
/// runner's publish stage binds real ids resolved from Postgres (roster
/// lookup + `(regime_id, external_id)` filing dedup). A placeholder can never
/// reach Gold — publish overwrites unconditionally, and the FK constraints
/// reject a nil id if a bug ever let one through.
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
        .context("silver payload is not a us_house staging row")?;

    // §3.4 transaction side map; parse already hard-rejected unknown tokens.
    let (side, partial_sale) = match row.transaction_type_raw.as_str() {
        "P" => (Side::Buy, false),
        "S" => (Side::Sell, false),
        "S (partial)" => (Side::Sell, true),
        "E" => (Side::Exchange, false),
        other => anyhow::bail!("unknown transaction type token {other:?} — never guess"),
    };

    // §3.3 owner precedence: row code, then vehicle `(Owner: XX)`, then the
    // flagged default-self assumption — provenance recorded in details.
    let (owner, owner_source) = if let Some(code) = row.owner_code_raw.as_deref() {
        (
            tables::owner_for_code(code).unwrap_or(Owner::Unknown),
            OwnerSource::Row,
        )
    } else if let Some(code) = row.vehicle_owner_code_raw.as_deref() {
        (
            tables::owner_for_code(code).unwrap_or(Owner::Unknown),
            OwnerSource::Vehicle,
        )
    } else {
        (Owner::Self_, OwnerSource::DefaultSelf)
    };

    // §3.6 asset-class buckets; missing/unknown code buckets to `other`
    // (already confidence-penalized at parse).
    let asset_class = row
        .asset_type_code_raw
        .as_deref()
        .and_then(tables::asset_class_for_code)
        .unwrap_or(AssetClass::Other);

    let value = band_interval(&row.amount_raw)?;
    let transaction_date = parse_mdy(&row.transaction_date_raw)?;
    let notified_date = parse_mdy(&row.notification_date_raw)?;
    let signed_date = parse_mdy(&row.signed_date_raw)?;

    let details = UsHousePtrTransactionDetailsV1 {
        doc_id: row.doc_id.clone(),
        row_ordinal: row.row_ordinal,
        row_id: row.row_id_raw.clone(),
        asset_type_code: row.asset_type_code_raw.clone(),
        amount_band_raw: row.amount_raw.clone(),
        transaction_type_raw: row.transaction_type_raw.clone(),
        partial_sale,
        cap_gains_over_200: row.cap_gains_over_200,
        filing_status_raw: row.filing_status_raw.clone(),
        owner_source: Some(owner_source),
        subholding_of: row.subholding_of_raw.clone(),
        vehicle_owner_code: row.vehicle_owner_code_raw.clone(),
        vehicle_location: row.vehicle_location_raw.clone(),
        description: row.description_raw.clone(),
        comments: row.comments_raw.clone(),
        signed_date,
    };

    let (filing_id, politician_id, regime_id) = match mode {
        IdentityMode::Conformance => (
            conformance_filing_id(&row.doc_id)?,
            conformance_politician_id(&row.filer_name_raw, &row.state_district_raw)?,
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
        asset_description_raw: row.asset_raw.clone(),
        record_type: RecordType::Transaction,
        asset_class,
        side: Some(side),
        transaction_date: Some(transaction_date),
        as_of_date: None,
        notified_date: Some(notified_date),
        value: Some(value),
        owner: Some(owner),
        extraction_confidence: Some(staged.confidence),
        extracted_by: row.extractor.clone(),
        // Computed at promotion over (filing_id, ordinal, content) — plan
        // Task 6; the candidate ships without it (GoldCandidate contract).
        fingerprint: None,
        details: serde_json::to_value(&details).context("serializing details")?,
    })
}

/// §3.5: band string → decimal interval, bounds as scale-2 decimals, USD.
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
pub(crate) fn parse_mdy(raw: &str) -> anyhow::Result<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%m/%d/%Y").with_context(|| format!("unparseable date {raw:?}"))
}

/// Nil-ULID placeholder for a not-yet-bound identity field (see
/// [`IdentityMode::Unbound`]).
fn unbound_id<T: std::str::FromStr>(what: &str) -> anyhow::Result<T>
where
    T::Err: std::fmt::Display,
{
    UNBOUND_ID
        .parse()
        .map_err(|e: T::Err| anyhow::anyhow!("unbound {what} id: {e}"))
}

/// Conformance filing ULID: prefix + `DocID` zero-padded to 19 (MANIFEST rule).
fn conformance_filing_id(doc_id: &str) -> anyhow::Result<FilingId> {
    anyhow::ensure!(
        !doc_id.is_empty() && doc_id.len() <= 19 && doc_id.bytes().all(|b| b.is_ascii_digit()),
        "doc_id {doc_id:?} cannot embed into a conformance filing ULID"
    );
    format!("{CONFORMANCE_FILING_PREFIX}{doc_id:0>19}")
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance filing id for doc {doc_id:?}: {e}"))
}

/// Fixed conformance politician ULID for `name|district`; unknown filers are
/// refused (extend the MANIFEST table with the fixture — never guess).
fn conformance_politician_id(name: &str, district: &str) -> anyhow::Result<PoliticianId> {
    let key = format!("{name}|{district}");
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
            conformance_filing_id("20020055").unwrap().to_string(),
            "0HSEFNG0000000000020020055"
        );
        assert_eq!(
            conformance_politician_id("Hon. Nancy Pelosi", "CA11")
                .unwrap()
                .to_string(),
            "0HSEMBR0000000000000000004"
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
        assert!(conformance_politician_id("Hon. Nobody", "ZZ99").is_err());
    }

    #[test]
    fn pool_backed_mode_emits_unbound_identity_for_any_filer() {
        // Not in the conformance table on purpose: pool-backed runs must not
        // consult it — the runner binds identities from Postgres instead.
        let staged = StagingRow {
            payload: serde_json::json!({
                "doc_id": "20099999",
                "row_ordinal": 1,
                "filer_name_raw": "Hon. Someone Unknown",
                "filer_status_raw": "Member",
                "state_district_raw": "ZZ99",
                "row_id_raw": null,
                "owner_code_raw": null,
                "asset_raw": "Example Corp (EX) [ST]",
                "asset_type_code_raw": "ST",
                "transaction_type_raw": "P",
                "transaction_date_raw": "05/13/2026",
                "notification_date_raw": "05/13/2026",
                "amount_raw": "$1,001 - $15,000",
                "cap_gains_over_200": null,
                "filing_status_raw": "New",
                "subholding_of_raw": null,
                "description_raw": null,
                "comments_raw": null,
                "vehicle_owner_code_raw": null,
                "vehicle_location_raw": null,
                "signed_date_raw": "06/12/2026",
                "extractor": "us_house_ptr/text@1"
            }),
            confidence: 0.98,
        };
        let candidate = normalize_row(&staged, IdentityMode::Unbound).unwrap();
        assert_eq!(candidate.filing_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.politician_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.regime_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.fingerprint, None, "computed at publish");
        // Conformance mode still refuses unknown filers (fixtures contract).
        assert!(normalize_row(&staged, IdentityMode::Conformance).is_err());
    }

    #[test]
    fn band_interval_maps_open_and_closed_bands() {
        let closed = band_interval("$250,001 - $500,000").unwrap();
        assert_eq!(closed.low(), dec!(250001.00));
        assert_eq!(closed.high().unwrap(), dec!(500000.00));
        assert_eq!(closed.currency(), Currency::USD);
        let open = band_interval("Over $50,000,000").unwrap();
        assert_eq!(open.low(), dec!(50000000.00));
        assert_eq!(open.high(), None);
        assert!(band_interval("$1 - $2").is_err(), "outside grammar");
    }

    #[test]
    fn money_serializes_as_decimal_strings() {
        let value = serde_json::to_value(band_interval("$1,001 - $15,000").unwrap()).unwrap();
        assert_eq!(value["low"], serde_json::json!("1001.00"));
        assert_eq!(value["high"], serde_json::json!("15000.00"));
    }
}
