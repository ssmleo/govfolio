//! §EU — European Parliament Declaration of Private Interests (DPI). An
//! LLM-vision regime (the `australia_register` precedent): `parse` routes the
//! multilingual PDF through the goal-021 extraction seam, which reads the OFFLINE
//! committed cache for conformance/e2e (`extraction.cache.json`, primed
//! mechanically from `expected.silver.json`). `normalize` maps the source-shaped
//! Silver into Gold: EUR income → exact `ValueInterval`; a parsed-but-unlisted
//! currency (PLN, the `Currency` enum holds only EUR/GBP/USD) → `value` NULL +
//! `value_source = unmapped_currency` (fail closed, §0.1); `section (D)` holdings
//! → `asset_class = equity`.

use anyhow::Context as _;
use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use govfolio_core::domain::enums::{AssetClass, Currency, Owner, RecordType};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use pipeline::adapter::{RawDocRef, RunCtx, StagingRow};
use pipeline::extraction::{CacheKey, FileCache, Models, pg_get};

use crate::ids::{self, IdentityMode};
use crate::util::{parse_amount, parse_ddmmyyyy};

/// Extractor tag recorded on every DPI Silver row (§EU.4).
pub(crate) const EXTRACTOR: &str = "eu_parliament_dpi/llm@1";
/// Regime code / `disclosure_regime` slug (§0).
pub(crate) const REGIME: &str = "eu_parliament_dpi";
/// Fixed conformance regime ULID (fixtures `MANIFEST.json`).
const CONFORMANCE_REGIME_ID: &str = "0EPDREG0000000000000000001";
/// Wrapper-confidence floor: a cached row below this fails closed.
const MIN_ACCEPT_CONFIDENCE: f32 = 0.9;

/// `dpi_uuid` → (conformance filing ULID, politician ULID). One document = one
/// filing = one MEP (fixtures `MANIFEST.json` `conformance_ids`).
const CONFORMANCE_MEMBERS: &[(&str, &str, &str)] = &[
    (
        "c8d42e82-1191-49d7-864e-52631d292544",
        "0EPDFNG0000000000000000001",
        "0EPDMBR0000000000000000001",
    ),
    (
        "37b6d6f9-8d23-4d6c-8dd0-14bb5472a06e",
        "0EPDFNG0000000000000000002",
        "0EPDMBR0000000000000000002",
    ),
    (
        "f4c30b56-86cb-4242-91e3-72b31bf8267c",
        "0EPDFNG0000000000000000003",
        "0EPDMBR0000000000000000003",
    ),
];

/// Periodicity of a declared income figure (rides `details`, never annualised
/// into `value` — §0.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Periodicity {
    /// Recurring monthly.
    Monthly,
    /// Recurring quarterly.
    Quarterly,
    /// Recurring annually.
    Annual,
    /// A single one-off benefit.
    OneOff,
    /// Any other cadence (hourly, per-session, …).
    Other,
}

/// §3.4 value-rule provenance for a DPI row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum EuValueSource {
    /// EUR income figure promoted to an exact `ValueInterval`.
    #[serde(rename = "income_amount")]
    IncomeAmount,
    /// No monetary income on the row — `value` NULL.
    #[serde(rename = "none")]
    NoneDeclared,
    /// Income declared in a currency the `Currency` enum does not hold (PLN, …)
    /// — fail closed to `value` NULL, raw amount + code kept (§0.1).
    #[serde(rename = "unmapped_currency")]
    UnmappedCurrency,
}

/// `details` payload of one DPI row (§EU.5). Optional fields serialize as
/// explicit nulls so the full contract surface is visible on every candidate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EuParliamentDpiInterestDetailsV1 {
    /// DPI document id (threaded from the URL).
    pub dpi_uuid: String,
    /// Filename `{ts}` millis — the version key (§EU.2).
    pub version_ts: String,
    /// European Parliament MEP id — the §EU.2 resolution key.
    pub mep_id: u64,
    /// Parliamentary term from the URL (e.g. `10`).
    #[schemars(range(min = 1))]
    pub parliamentary_term: u32,
    /// Article-keyed section letter `A`..`G` (§EU.3).
    pub section: String,
    /// 1-based row position across the document.
    #[schemars(range(min = 1))]
    pub row_ordinal: u32,
    /// The occupation/activity/holding text, verbatim in the filing language.
    pub entry_text: String,
    /// `Income amount` cell verbatim (`11000 EUR`, `4940 PLN`, null).
    pub income_amount_raw: Option<String>,
    /// Parsed ISO currency code (`EUR`/`PLN`); null when the row has no amount.
    pub income_currency: Option<String>,
    /// Parsed cadence; null when the row has no periodicity cell.
    pub periodicity: Option<Periodicity>,
    /// `Nature of the benefit` cell verbatim; null when absent.
    pub benefit_nature: Option<String>,
    /// Footer declaration date (`DD/MM/YYYY` → ISO).
    pub declaration_date: NaiveDate,
    /// Detected form language (ISO 639-1 or `mul`).
    pub language: String,
    /// §EU.6 value provenance.
    pub value_source: EuValueSource,
}

/// JSON Schema for the `(eu_parliament_dpi, interest)` details contract.
#[must_use]
pub fn interest_details_schema() -> schemars::Schema {
    schemars::schema_for!(EuParliamentDpiInterestDetailsV1)
}

/// One DPI Silver staging row (§EU.4); only the fields Gold consumes are read
/// (the header names ride along in the payload for audit).
#[derive(Debug, Clone, Deserialize)]
struct SilverRow {
    dpi_uuid: String,
    version_ts: String,
    mep_id: u64,
    parliamentary_term: u32,
    section: String,
    row_ordinal: u32,
    entry_text_raw: String,
    income_amount_raw: Option<String>,
    benefit_nature_raw: Option<String>,
    periodicity_raw: Option<String>,
    #[allow(dead_code)]
    none_flag: bool,
    declaration_date_raw: String,
    lang: String,
    extractor: String,
}

/// The goal-021 LLM extractor: offline committed cache + Postgres tier; the live
/// vision path is a recorded follow-up (browser-engine fetch seam + schema-
/// constrained transcription), so a cache miss freezes the document (invariant 6).
#[derive(Debug, Clone)]
pub(crate) struct LlmExtractor {
    file_cache: FileCache,
}

impl Default for LlmExtractor {
    fn default() -> Self {
        Self {
            file_cache: FileCache::open(pipeline::conformance::fixtures_dir("eu_fr_de_annual")),
        }
    }
}

impl LlmExtractor {
    pub(crate) async fn extract(
        &self,
        doc: &RawDocRef,
        ctx: &RunCtx,
    ) -> anyhow::Result<Vec<StagingRow>> {
        let models = Models::from_env();
        let key = CacheKey::new(&doc.sha256, EXTRACTOR, &models.primary);
        if let Some(rows) = self.file_cache.get(&key)? {
            return validated(rows, &doc.sha256);
        }
        if let Some(pool) = &ctx.pool
            && let Some(rows) = pg_get(pool, &key).await?
        {
            return validated(rows, &doc.sha256);
        }
        anyhow::bail!(
            "needs_llm_extraction: no cached vision extraction for DPI document {} \
             (key: {EXTRACTOR} / {}); the live path requires the europarl fetch + \
             schema-constrained vision transcription (recorded follow-ups) — freeze \
             + review_task (invariant 6)",
            doc.sha256,
            models.primary
        )
    }
}

fn validated(rows: Vec<StagingRow>, sha256: &str) -> anyhow::Result<Vec<StagingRow>> {
    anyhow::ensure!(
        !rows.is_empty(),
        "cached extraction for {sha256} is empty — fail closed (invariant 6)"
    );
    for (index, staged) in rows.iter().enumerate() {
        let row: SilverRow = serde_json::from_value(staged.payload.clone()).with_context(|| {
            format!("cached extraction row {index} for {sha256} is not a DPI SilverRow")
        })?;
        anyhow::ensure!(
            row.extractor == EXTRACTOR,
            "cached row {index} for {sha256} carries tag {:?}, want {EXTRACTOR:?}",
            row.extractor
        );
        anyhow::ensure!(
            (MIN_ACCEPT_CONFIDENCE..=1.0).contains(&staged.confidence),
            "cached row {index} for {sha256} confidence {} below the {MIN_ACCEPT_CONFIDENCE} floor",
            staged.confidence
        );
    }
    Ok(rows)
}

/// Silver → Gold (§EU.5).
pub(crate) fn normalize(rows: &[StagingRow], ctx: &RunCtx) -> anyhow::Result<Vec<GoldCandidate>> {
    let mode = IdentityMode::of(ctx);
    rows.iter().map(|row| normalize_row(row, mode)).collect()
}

fn normalize_row(staged: &StagingRow, mode: IdentityMode) -> anyhow::Result<GoldCandidate> {
    let row: SilverRow = serde_json::from_value(staged.payload.clone())
        .context("silver payload is not a DPI staging row")?;

    anyhow::ensure!(
        !row.entry_text_raw.trim().is_empty(),
        "empty entry_text at DPI row {} — reject (invariant 2)",
        row.row_ordinal
    );

    let declaration_date = parse_ddmmyyyy(&row.declaration_date_raw).with_context(|| {
        format!(
            "unparseable DPI declaration date {:?}",
            row.declaration_date_raw
        )
    })?;

    let income_currency = row.income_amount_raw.as_deref().and_then(parse_currency);
    let periodicity = map_periodicity(row.periodicity_raw.as_deref());

    let (value, value_source) = match &row.income_amount_raw {
        None => (None, EuValueSource::NoneDeclared),
        Some(raw) => match income_currency.as_deref() {
            Some("EUR") => {
                let amount = parse_amount(raw, false)
                    .with_context(|| format!("unparseable EUR income {raw:?}"))?;
                let interval = ValueInterval::new(amount, Some(amount), Currency::EUR)
                    .map_err(|e| anyhow::anyhow!("bad EUR income {raw:?}: {e}"))?;
                (Some(interval), EuValueSource::IncomeAmount)
            }
            // A parsed-but-unlisted currency (PLN …) fails closed to NULL (§0.1).
            _ => (None, EuValueSource::UnmappedCurrency),
        },
    };

    let asset_class = if row.section == "D" {
        AssetClass::Equity
    } else {
        AssetClass::Other
    };

    let details = EuParliamentDpiInterestDetailsV1 {
        dpi_uuid: row.dpi_uuid.clone(),
        version_ts: row.version_ts.clone(),
        mep_id: row.mep_id,
        parliamentary_term: row.parliamentary_term,
        section: row.section.clone(),
        row_ordinal: row.row_ordinal,
        entry_text: row.entry_text_raw.clone(),
        income_amount_raw: row.income_amount_raw.clone(),
        income_currency,
        periodicity,
        benefit_nature: row.benefit_nature_raw.clone(),
        declaration_date,
        language: row.lang.clone(),
        value_source,
    };

    let (filing_id, politician_id, regime_id) = resolve_ids(mode, &row.dpi_uuid)?;

    Ok(GoldCandidate {
        filing_id,
        politician_id,
        regime_id,
        instrument_id: None,
        asset_description_raw: row.entry_text_raw.clone(),
        record_type: RecordType::Interest,
        asset_class,
        side: None,
        transaction_date: None,
        as_of_date: None,
        notified_date: Some(declaration_date),
        value,
        owner: Some(Owner::Self_),
        extraction_confidence: Some(staged.confidence),
        extracted_by: row.extractor.clone(),
        fingerprint: None,
        details: serde_json::to_value(details).context("serializing DPI details")?,
    })
}

/// The trailing 3-letter uppercase currency token of an `Income amount` cell.
fn parse_currency(raw: &str) -> Option<String> {
    raw.split_whitespace()
        .next_back()
        .filter(|t| t.len() == 3 && t.chars().all(|c| c.is_ascii_uppercase()))
        .map(str::to_owned)
}

/// Maps a multilingual periodicity cell to the closed cadence vocabulary; an
/// unrecognized non-empty cell is `Other` (never guessed into a number).
fn map_periodicity(raw: Option<&str>) -> Option<Periodicity> {
    let norm = raw?.trim().to_lowercase();
    Some(match norm.as_str() {
        "μηνιαίως" | "miesięcznie" | "monthly" | "mensuel" | "mensuellement" | "monatlich" => {
            Periodicity::Monthly
        }
        "kwartalnie" | "quarterly" | "trimestriel" | "vierteljährlich" => Periodicity::Quarterly,
        "rocznie" | "ετησίως" | "annually" | "annual" | "annuel" | "annuellement" | "jährlich" => {
            Periodicity::Annual
        }
        "jednorazowo" | "one-off" | "one_off" | "einmalig" | "ponctuel" => Periodicity::OneOff,
        "" => return None,
        _ => Periodicity::Other,
    })
}

fn resolve_ids(
    mode: IdentityMode,
    dpi_uuid: &str,
) -> anyhow::Result<(
    govfolio_core::ids::FilingId,
    govfolio_core::ids::PoliticianId,
    govfolio_core::ids::RegimeId,
)> {
    let (_, filing, politician) = CONFORMANCE_MEMBERS
        .iter()
        .find(|(uuid, _, _)| *uuid == dpi_uuid)
        .with_context(|| format!("no conformance ids for DPI {dpi_uuid:?} — never guess"))?;
    ids::resolve(mode, filing, politician, CONFORMANCE_REGIME_ID)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn periodicity_maps_across_languages() {
        assert_eq!(
            map_periodicity(Some("Μηνιαίως")),
            Some(Periodicity::Monthly)
        );
        assert_eq!(map_periodicity(Some("Monthly")), Some(Periodicity::Monthly));
        assert_eq!(
            map_periodicity(Some("miesięcznie")),
            Some(Periodicity::Monthly)
        );
        assert_eq!(
            map_periodicity(Some("kwartalnie")),
            Some(Periodicity::Quarterly)
        );
        assert_eq!(
            map_periodicity(Some("Ωριαία αποζημίωση")),
            Some(Periodicity::Other)
        );
        assert_eq!(map_periodicity(None), None);
    }

    #[test]
    fn currency_token_is_the_trailing_iso_code() {
        assert_eq!(parse_currency("11000 EUR").as_deref(), Some("EUR"));
        assert_eq!(parse_currency("4940 PLN").as_deref(), Some("PLN"));
        assert_eq!(parse_currency("informacja publiczna"), None);
    }
}
