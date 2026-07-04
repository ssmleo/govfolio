//! `fixture_fake` — a synthetic jurisdiction whose "source" is the local
//! fixture directory. It exists to prove the conformance harness (plan Task 7):
//! every trait method is exercised without touching the network.

use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;

use govfolio_core::domain::enums::{AssetClass, Currency, Owner, RecordType, Side};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use govfolio_core::ids::{FilingId, PoliticianId, RegimeId};
use pipeline::adapter::{
    FilingRef, JurisdictionAdapter, PolitenessCfg, RawDocRef, RegimeRef, RunCtx, StagingRow,
};

/// The synthetic adapter.
#[derive(Debug, Default, Clone, Copy)]
pub struct FixtureFakeAdapter;

/// Source shape of one fake filing — what `input.json` holds.
#[derive(Debug, Deserialize)]
struct FakeFiling {
    external_id: String,
    filing_id: String,
    politician_id: String,
    regime_id: String,
    rows: Vec<FakeRow>,
}

/// One source row, in the source's own vocabulary (`P`/`S`, `SP`/`JT`, bands).
#[derive(Debug, Deserialize)]
struct FakeRow {
    asset: String,
    #[serde(rename = "type")]
    kind: String,
    date: String,
    amount: String,
    owner: String,
}

/// Silver payload shape (source vocabulary + provenance), re-read by `normalize`.
#[derive(Debug, Deserialize)]
struct StagedFake {
    filing_id: String,
    politician_id: String,
    regime_id: String,
    ordinal: u32,
    asset: String,
    #[serde(rename = "type")]
    kind: String,
    date: String,
    amount: String,
    owner: String,
}

#[async_trait]
impl JurisdictionAdapter for FixtureFakeAdapter {
    fn regime(&self) -> RegimeRef {
        RegimeRef {
            code: "fixture_fake",
        }
    }

    fn politeness(&self) -> PolitenessCfg {
        // No network is touched; the config stays honest anyway (invariant 10).
        PolitenessCfg::new(Duration::from_secs(1), "dev@govfolio.io")
    }

    async fn discover(&self, _ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        // The local "source": every committed fixture case is one filing.
        let fixtures = pipeline::conformance::fixtures_dir("fixture_fake");
        let mut filings = Vec::new();
        for entry in std::fs::read_dir(&fixtures)
            .with_context(|| format!("listing {}", fixtures.display()))?
        {
            let case = entry?.path();
            let input = case.join("input.json");
            if input.is_file() {
                filings.push(FilingRef {
                    external_id: case
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    url: input.display().to_string(),
                });
            }
        }
        filings.sort_by(|a, b| a.external_id.cmp(&b.external_id));
        Ok(filings)
    }

    async fn fetch(&self, r: &FilingRef, ctx: &RunCtx) -> anyhow::Result<RawDocRef> {
        // The "download" is a local read; Bronze still receives the exact raw
        // bytes (invariant 2).
        let bytes = std::fs::read(&r.url).with_context(|| format!("reading {}", r.url))?;
        ctx.bronze.put(&bytes)
    }

    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        let bytes = ctx.bronze.get(d)?;
        let filing: FakeFiling =
            serde_json::from_slice(&bytes).context("input is not a fake filing")?;
        let mut rows = Vec::with_capacity(filing.rows.len());
        for (ordinal, row) in filing.rows.iter().enumerate() {
            rows.push(StagingRow {
                payload: json!({
                    "external_id": filing.external_id,
                    "filing_id": filing.filing_id,
                    "politician_id": filing.politician_id,
                    "regime_id": filing.regime_id,
                    "ordinal": ordinal,
                    "asset": row.asset,
                    "type": row.kind,
                    "date": row.date,
                    "amount": row.amount,
                    "owner": row.owner,
                }),
                confidence: 1.0,
            });
        }
        Ok(rows)
    }

    async fn normalize(
        &self,
        rows: &[StagingRow],
        _ctx: &RunCtx,
    ) -> anyhow::Result<Vec<GoldCandidate>> {
        rows.iter().map(normalize_row).collect()
    }
}

/// Silver → Gold for one staged row: source codes map to the closed
/// vocabularies; unknown codes are refused, never guessed (invariant 3).
fn normalize_row(row: &StagingRow) -> anyhow::Result<GoldCandidate> {
    let staged: StagedFake = serde_json::from_value(row.payload.clone())
        .context("silver payload is not a staged fake row")?;
    let side = match staged.kind.as_str() {
        "P" => Side::Buy,
        "S" => Side::Sell,
        other => anyhow::bail!("unknown transaction code {other:?} — never guess (invariant 3)"),
    };
    let owner = match staged.owner.as_str() {
        "SELF" => Owner::Self_,
        "SP" => Owner::Spouse,
        "JT" => Owner::Joint,
        other => anyhow::bail!("unknown owner code {other:?} — never guess (invariant 3)"),
    };
    let transaction_date = NaiveDate::parse_from_str(&staged.date, "%m/%d/%Y")
        .with_context(|| format!("bad date {:?}", staged.date))?;
    let filing_id: FilingId = staged
        .filing_id
        .parse()
        .map_err(|e| anyhow::anyhow!("bad filing_id {:?}: {e}", staged.filing_id))?;
    let politician_id: PoliticianId = staged
        .politician_id
        .parse()
        .map_err(|e| anyhow::anyhow!("bad politician_id {:?}: {e}", staged.politician_id))?;
    let regime_id: RegimeId = staged
        .regime_id
        .parse()
        .map_err(|e| anyhow::anyhow!("bad regime_id {:?}: {e}", staged.regime_id))?;
    Ok(GoldCandidate {
        filing_id,
        politician_id,
        regime_id,
        instrument_id: None,
        asset_description_raw: staged.asset,
        record_type: RecordType::Transaction,
        asset_class: AssetClass::Equity,
        side: Some(side),
        transaction_date: Some(transaction_date),
        as_of_date: None,
        notified_date: None,
        value: Some(parse_band(&staged.amount)?),
        owner: Some(owner),
        extraction_confidence: Some(row.confidence),
        extracted_by: "fixture_fake:parser@1".to_owned(),
        fingerprint: None,
        details: json!({
            "amount_band_raw": staged.amount,
            "source_ordinal": staged.ordinal,
        }),
    })
}

/// `"$1,001 - $15,000"` → `1001.00 ..= 15000.00 USD` (invariant 7: decimals).
fn parse_band(band: &str) -> anyhow::Result<ValueInterval> {
    let (low, high) = band
        .split_once(" - ")
        .with_context(|| format!("amount {band:?} is not a band"))?;
    let low = parse_money(low)?;
    let high = parse_money(high)?;
    ValueInterval::new(low, Some(high), Currency::USD)
        .map_err(|e| anyhow::anyhow!("bad band {band:?}: {e}"))
}

/// `"$15,000"` → `15000.00` (band bounds are whole dollars; two decimal places
/// on the wire per invariant 7).
fn parse_money(text: &str) -> anyhow::Result<Decimal> {
    let cleaned: String = text
        .trim()
        .chars()
        .filter(|c| *c != '$' && *c != ',')
        .collect();
    let mut amount: Decimal = cleaned
        .parse()
        .map_err(|e| anyhow::anyhow!("bad amount {text:?}: {e}"))?;
    amount.rescale(2);
    Ok(amount)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::json;

    use super::*;

    #[test]
    fn band_parses_to_decimal_interval() {
        let v = parse_band("$1,001 - $15,000").unwrap();
        assert_eq!(v.low(), dec!(1001.00));
        assert_eq!(v.high().unwrap(), dec!(15000.00));
        assert_eq!(v.currency(), Currency::USD);
    }

    #[test]
    fn non_band_amounts_are_refused() {
        assert!(parse_band("a lot").is_err());
        assert!(parse_band("$5,000").is_err());
    }

    fn staged(kind: &str, owner: &str) -> StagingRow {
        StagingRow {
            payload: json!({
                "external_id": "FF-2026-0001",
                "filing_id": "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                "politician_id": "01BX5ZZKBKACTAV9WEVGEMMVRZ",
                "regime_id": "01BX5ZZKBKACTAV9WEVGEMMVS0",
                "ordinal": 0,
                "asset": "Microsoft Corporation - Common Stock (MSFT)",
                "type": kind,
                "date": "03/02/2026",
                "amount": "$1,001 - $15,000",
                "owner": owner,
            }),
            confidence: 1.0,
        }
    }

    #[test]
    fn known_codes_normalize_to_closed_vocabulary() {
        let gold = normalize_row(&staged("P", "SP")).unwrap();
        assert_eq!(gold.record_type, RecordType::Transaction);
        assert_eq!(gold.side, Some(Side::Buy));
        assert_eq!(gold.owner, Some(Owner::Spouse));
        assert_eq!(
            gold.transaction_date,
            Some(NaiveDate::from_ymd_opt(2026, 3, 2).unwrap())
        );
        gold.validate().unwrap();
    }

    #[test]
    fn unknown_codes_are_never_guessed() {
        assert!(
            normalize_row(&staged("X", "SP")).is_err(),
            "unknown side code"
        );
        assert!(
            normalize_row(&staged("P", "??")).is_err(),
            "unknown owner code"
        );
    }
}
