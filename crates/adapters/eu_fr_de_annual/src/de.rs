//! §DE — Deutscher Bundestag veröffentlichungspflichtige Angaben (Nebentätigkeiten).
//! A deterministic parse of the server-rendered disclosure fragment behind the
//! MANDATORY browser-engine fetch seam (Enodia gate, §DE.2) — the committed
//! `input.html` was captured through that seam (fixtures `MANIFEST.json`). Parsing
//! uses the crate's light `quick-xml` DOM (NOT `scraper` — CI link-footprint,
//! §DE builder notes). `value` = the FIRST regular published amount (exact
//! euro/cent); `zuzüglich`/`(ab YYYY)` supplements ride `amount_raw` only (§DE
//! flags). 21. WP publishes exact amounts — the §DE.6 10-Stufen banding is
//! historical/backfill, OUT of this green path.

use anyhow::Context as _;
use rust_decimal::Decimal;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use govfolio_core::domain::enums::{AssetClass, Currency, Owner, RecordType};
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::domain::value::ValueInterval;
use pipeline::adapter::{RunCtx, StagingRow};

use crate::dom::{self, Node};
use crate::ids::{self, IdentityMode};
use crate::util::parse_amount;

/// Extractor tag recorded on every Bundestag Silver row (§DE.4).
pub(crate) const EXTRACTOR: &str = "de_bundestag/html@1";
/// Regime code / `disclosure_regime` slug (§0).
pub(crate) const REGIME: &str = "de_bundestag";
/// Fixed conformance regime ULID (fixtures `MANIFEST.json`).
const CONFORMANCE_REGIME_ID: &str = "0DEBREG0000000000000000001";

/// Document sha256 → (mdb id, member name, Wahlperiode). Production threads member
/// identity from the discovery URL / roster (§DE.2); conformance pins it here.
const CONFORMANCE_MEMBERS: &[(&str, u64, &str, u32)] = &[
    (
        "e08e494ecccf8783dc8eee9f26d6a4758b661b56b58aa57deee3466298129509",
        1_049_290,
        "Alexander Throm",
        21,
    ),
    (
        "e203363827b8f2a5fcd5fe51d553b7490c12475958f371505c2d597970f30b01",
        1_049_272,
        "Stephan Stracke",
        21,
    ),
    (
        "689a7468c857f5449ba3a93ec1925d6422cc4be5b2e1c3ff60c5b00bd6772b0a",
        1_048_368,
        "Thomas Bareiß",
        21,
    ),
];

/// mdb id → (conformance filing ULID, politician ULID).
const CONFORMANCE_FILINGS: &[(u64, &str, &str)] = &[
    (
        1_049_290,
        "0DEBFNG0000000000000000001",
        "0DEBMBR0000000000000000001",
    ),
    (
        1_049_272,
        "0DEBFNG0000000000000000002",
        "0DEBMBR0000000000000000002",
    ),
    (
        1_048_368,
        "0DEBFNG0000000000000000003",
        "0DEBMBR0000000000000000003",
    ),
];

/// Published income cadence (§DE.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Period {
    /// `monatlich` regular monthly income.
    Monthly,
    /// `jährlich` regular annual income.
    Annual,
    /// `einmalig` / year-prefixed one-off income.
    OneOff,
}

/// §DE.5 value-rule provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum DeValueSource {
    /// Exact euro/cent figure (current, 21. WP).
    #[serde(rename = "betragsgenau")]
    Betragsgenau,
    /// Backfilled 10-Stufen band (18./19. WP; OUT of this green path).
    #[serde(rename = "stufe_historical")]
    StufeHistorical,
    /// No published amount — `value` NULL.
    #[serde(rename = "none")]
    NoneDeclared,
}

/// `details` payload of one Bundestag disclosure entry (§DE.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeBundestagInterestDetailsV1 {
    /// Biografien numeric id — the §DE.2 person key.
    pub mdb_id: u64,
    /// Wahlperiode (e.g. `21`).
    #[schemars(range(min = 1))]
    pub wahlperiode: u32,
    /// §DE.3 category number 1..8.
    #[schemars(range(min = 1, max = 8))]
    pub category_number: u32,
    /// Category heading verbatim.
    pub category_name: String,
    /// 1-based order across the member's disclosures.
    #[schemars(range(min = 1))]
    pub row_ordinal: u32,
    /// Entity + role line(s), verbatim German.
    pub entry_text: String,
    /// Published income string verbatim (`monatlich, 1.250,43 EUR, …`); null when
    /// the entry carries no income.
    pub amount_raw: Option<String>,
    /// Parsed cadence of the regular amount; null when no amount.
    pub period: Option<Period>,
    /// Year prefix of a one-off amount; null otherwise.
    pub amount_year: Option<i64>,
    /// Anonymised contract-partner list (`Mandant N, sector, years`), joined
    /// `; `; null when the entry has no partner sub-list.
    pub partner: Option<String>,
    /// The `Gewinn vor Steuern` profit variant.
    pub profit_before_tax: bool,
    /// Non-quantifiable position (`Rechtsposition`) — value NULL, raw kept.
    pub non_quantifiable: bool,
    /// Honorary marker.
    pub ehrenamtlich: bool,
    /// Ingestion language: always `de`.
    pub language: String,
    /// §DE.5 value provenance.
    pub value_source: DeValueSource,
}

/// JSON Schema for the `(de_bundestag, interest)` details contract.
#[must_use]
pub fn interest_details_schema() -> schemars::Schema {
    schemars::schema_for!(DeBundestagInterestDetailsV1)
}

/// One Bundestag Silver staging row (§DE.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SilverRow {
    mdb_id: u64,
    member_name_raw: String,
    wahlperiode: u32,
    category_number: u32,
    category_name_raw: String,
    row_ordinal: u32,
    entry_text_raw: String,
    amount_raw: Option<String>,
    partner_raw: Option<String>,
    ehrenamtlich: bool,
    extractor: String,
}

/// Bronze HTML → Silver (§DE.4). `sha256` binds the conformance member identity.
pub(crate) fn parse(bytes: &[u8], sha256: &str) -> anyhow::Result<Vec<StagingRow>> {
    let (_, mdb_id, member_name, wahlperiode) = CONFORMANCE_MEMBERS
        .iter()
        .find(|(sha, ..)| *sha == sha256)
        .copied()
        .with_context(|| {
            format!(
                "no runner binding for Bundestag document {sha256} — production threads member \
                 identity from the browser-engine discovery seam (follow-up); freeze (invariant 6)"
            )
        })?;

    let html = String::from_utf8_lossy(bytes);
    let region = disclosure_region(&html)
        .context("Bundestag page has no m-biography__infos disclosure block — freeze")?;
    let root = dom::parse_str(&strip_ignorable(region)).context("parsing Bundestag disclosure")?;
    let container = root
        .children
        .iter()
        .find(|c| c.name == "div")
        .unwrap_or(&root);

    let mut rows = Vec::new();
    let mut ordinal: u32 = 0;
    let mut category: Option<(u32, String)> = None;
    for node in &container.children {
        match node.name.as_str() {
            "h3" => {
                let heading = dom::normalize_ws(&node.text);
                let number = category_number(&heading).with_context(|| {
                    format!("unknown Bundestag category heading {heading:?} — freeze (invariant 6)")
                })?;
                category = Some((number, heading));
            }
            "ul" if node.class().split_whitespace().any(|c| c == "voa_list") => {
                let (number, name) = category
                    .as_ref()
                    .context("disclosure entry before any category heading — freeze")?;
                ordinal += 1;
                rows.push(build_silver_row(
                    mdb_id,
                    member_name,
                    wahlperiode,
                    *number,
                    name,
                    ordinal,
                    node,
                )?);
            }
            _ => {}
        }
    }

    anyhow::ensure!(
        !rows.is_empty(),
        "Bundestag parse produced zero rows for {sha256} — freeze (invariant 6)"
    );
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
fn build_silver_row(
    mdb_id: u64,
    member_name: &str,
    wahlperiode: u32,
    category_number: u32,
    category_name: &str,
    ordinal: u32,
    entry: &Node,
) -> anyhow::Result<StagingRow> {
    let lis: Vec<&Node> = entry.children_named("li").collect();
    let (entity_node, role_nodes) = lis
        .split_first()
        .context("disclosure entry has no <li> — freeze")?;

    let entity = strip_trailing_comma(&dom::normalize_ws(&entity_node.text));
    anyhow::ensure!(
        !entity.is_empty(),
        "empty disclosure entity — reject (invariant 2)"
    );

    let mut roles = Vec::new();
    let mut amount_pieces: Vec<String> = Vec::new();
    let mut partners = Vec::new();

    // Contract partners may hang off any <li> (they hang off the role li in
    // practice); collect from all lis.
    for li in &lis {
        for nested in li.children_named("ul") {
            for partner in nested.children_named("li") {
                let text = dom::normalize_ws(&partner.text);
                if !text.is_empty() {
                    partners.push(text);
                }
            }
        }
    }

    for li in role_nodes {
        let text = dom::normalize_ws(&li.text);
        if text.is_empty() {
            continue;
        }
        let (role, income) = split_income(&text);
        if let Some(income) = income {
            amount_pieces.push(income);
        }
        if !role.is_empty() {
            roles.push(role);
        }
    }
    let amount_raw = (!amount_pieces.is_empty()).then(|| amount_pieces.join("; "));

    let entry_text = if roles.is_empty() {
        entity
    } else {
        format!("{entity} \u{2014} {}", roles.join(" \u{b7} "))
    };
    let ehrenamtlich =
        roles.iter().any(|r| r.contains("ehrenamtlich")) || entry_text.contains("ehrenamtlich");
    let partner_raw = (!partners.is_empty()).then(|| partners.join("; "));

    let row = SilverRow {
        mdb_id,
        member_name_raw: member_name.to_owned(),
        wahlperiode,
        category_number,
        category_name_raw: category_name.to_owned(),
        row_ordinal: ordinal,
        entry_text_raw: entry_text,
        amount_raw,
        partner_raw,
        ehrenamtlich,
        extractor: EXTRACTOR.to_owned(),
    };

    Ok(StagingRow {
        payload: serde_json::to_value(&row).context("serializing Bundestag silver row")?,
        confidence: 1.0,
    })
}

/// Silver → Gold (§DE.5).
pub(crate) fn normalize(rows: &[StagingRow], ctx: &RunCtx) -> anyhow::Result<Vec<GoldCandidate>> {
    let mode = IdentityMode::of(ctx);
    rows.iter().map(|row| normalize_row(row, mode)).collect()
}

fn normalize_row(staged: &StagingRow, mode: IdentityMode) -> anyhow::Result<GoldCandidate> {
    let row: SilverRow = serde_json::from_value(staged.payload.clone())
        .context("silver payload is not a Bundestag staging row")?;

    anyhow::ensure!(
        !row.entry_text_raw.trim().is_empty(),
        "empty entry_text at Bundestag row {} — reject (invariant 2)",
        row.row_ordinal
    );
    anyhow::ensure!(
        (1..=8).contains(&row.category_number),
        "category number {} outside the 1..8 census (§DE.3) — freeze",
        row.category_number
    );

    let amount = row.amount_raw.as_deref();
    let profit_before_tax = amount.is_some_and(|a| a.contains("Gewinn vor Steuern"));
    let non_quantifiable = amount.is_some_and(|a| a.contains("Rechtsposition"));

    let (period, amount_year, parsed) = amount.map_or((None, None, None), parse_income);
    let (value, value_source) = match parsed {
        Some(amount) if !non_quantifiable => {
            let interval = ValueInterval::new(amount, Some(amount), Currency::EUR)
                .map_err(|e| anyhow::anyhow!("bad Bundestag value {amount}: {e}"))?;
            (Some(interval), DeValueSource::Betragsgenau)
        }
        _ => (None, DeValueSource::NoneDeclared),
    };
    // period is only meaningful for a promoted regular amount.
    let period = value.and(period);

    let asset_class = if row.category_number == 7 {
        AssetClass::Equity
    } else {
        AssetClass::Other
    };

    let details = DeBundestagInterestDetailsV1 {
        mdb_id: row.mdb_id,
        wahlperiode: row.wahlperiode,
        category_number: row.category_number,
        category_name: row.category_name_raw.clone(),
        row_ordinal: row.row_ordinal,
        entry_text: row.entry_text_raw.clone(),
        amount_raw: row.amount_raw.clone(),
        period,
        amount_year,
        partner: row.partner_raw.clone(),
        profit_before_tax,
        non_quantifiable,
        ehrenamtlich: row.ehrenamtlich,
        language: "de".to_owned(),
        value_source,
    };

    let (filing_id, politician_id, regime_id) = resolve_ids(mode, row.mdb_id)?;

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
        notified_date: None, // no per-entry date; page-capture date is NOT fabricated (§DE.5)
        value,
        owner: Some(Owner::Self_),
        extraction_confidence: Some(staged.confidence),
        extracted_by: row.extractor.clone(),
        fingerprint: None,
        details: serde_json::to_value(details).context("serializing Bundestag details")?,
    })
}

/// The disclosure entries region: the inner content between the
/// `m-biography__infos` open and the following `infoDisclaimer` div.
fn disclosure_region(html: &str) -> Option<&str> {
    const OPEN: &str = r#"<div class="m-biography__infos">"#;
    const DISCLAIMER: &str = r#"<div class="m-biography__infoDisclaimer">"#;
    let start = html.find(OPEN)? + OPEN.len();
    let rest = &html[start..];
    let end = rest.find(DISCLAIMER).unwrap_or(rest.len());
    Some(&rest[..end])
}

/// Drops the layout-only `<p>`/`<br>` tags so the fragment is clean XML.
fn strip_ignorable(region: &str) -> String {
    let mut out = region.to_owned();
    for tag in ["<p>", "</p>", "<p/>", "<br/>", "<br>", "<br />"] {
        out = out.replace(tag, "");
    }
    out
}

/// Splits a role `<li>` into (role text, published income). The income begins at
/// the first cadence keyword; entries without one carry no income.
fn split_income(text: &str) -> (String, Option<String>) {
    const KEYWORDS: &[&str] = &[
        "monatlich",
        "jährlich",
        "einmalig",
        "vierteljährlich",
        "halbjährlich",
    ];
    // A `zuzüglich …` supplement line is an income continuation, not a role.
    if text.starts_with("zuzüglich") {
        return (String::new(), Some(text.to_owned()));
    }
    for kw in KEYWORDS {
        if text.starts_with(kw) {
            return (String::new(), Some(text.to_owned()));
        }
    }
    let mut best: Option<usize> = None;
    for kw in KEYWORDS {
        let needle = format!(", {kw}");
        if let Some(i) = text.find(&needle) {
            best = Some(best.map_or(i, |b| b.min(i)));
        }
    }
    match best {
        Some(i) => (
            text[..i].trim().to_owned(),
            Some(text[i + 2..].trim().to_owned()),
        ),
        None => (text.trim().to_owned(), None),
    }
}

/// Parses a published income string → (cadence, one-off year, exact euro amount).
fn parse_income(amount: &str) -> (Option<Period>, Option<i64>, Option<Decimal>) {
    let a = amount.trim();
    let (period, amount_year) = if a.starts_with("monatlich") {
        (Some(Period::Monthly), None)
    } else if a.starts_with("jährlich") {
        (Some(Period::Annual), None)
    } else if a.starts_with("einmalig") {
        (Some(Period::OneOff), None)
    } else if let Some(year) = leading_year(a) {
        (Some(Period::OneOff), Some(year))
    } else {
        (None, None)
    };
    (period, amount_year, first_euro_amount(a))
}

/// A 4-digit year at the start of a one-off amount (`2021, …`).
fn leading_year(a: &str) -> Option<i64> {
    let digits: String = a.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() == 4 {
        digits.parse().ok()
    } else {
        None
    }
}

/// The FIRST German-formatted euro amount (`1.250,43 EUR` / `5.400 Euro`) →
/// `Decimal`; supplements after it are ignored (§DE flags).
fn first_euro_amount(a: &str) -> Option<Decimal> {
    let bytes = a.as_bytes();
    let mut marker = None;
    for needle in ["EUR", "Euro"] {
        if let Some(i) = a.find(needle) {
            marker = Some(marker.map_or(i, |m: usize| m.min(i)));
        }
    }
    let mut end = marker?;
    while end > 0 && bytes[end - 1] == b' ' {
        end -= 1;
    }
    let mut start = end;
    while start > 0 {
        let c = bytes[start - 1];
        if c.is_ascii_digit() || c == b'.' || c == b',' {
            start -= 1;
        } else {
            break;
        }
    }
    parse_amount(&a[start..end], true)
}

/// §DE.3 category heading → number (prefix match tolerates the und/oder wording).
fn category_number(heading: &str) -> Option<u32> {
    let h = heading.trim();
    let starts = |p: &str| h.starts_with(p);
    Some(if starts("Berufliche Tätigkeit vor") {
        1
    } else if starts("Entgeltliche Tätigkeiten") {
        2
    } else if starts("Funktionen in Unternehmen") {
        3
    } else if starts("Funktionen in Körperschaften") {
        4
    } else if starts("Funktionen in Vereinen") {
        5
    } else if starts("Vereinbarungen über künftige") {
        6
    } else if starts("Beteiligungen an Kapital") {
        7
    } else if starts("Spenden und sonstige Zuwendungen") {
        8
    } else {
        return None;
    })
}

fn strip_trailing_comma(s: &str) -> String {
    let t = s.trim();
    t.strip_suffix(',').unwrap_or(t).trim().to_owned()
}

fn resolve_ids(
    mode: IdentityMode,
    mdb_id: u64,
) -> anyhow::Result<(
    govfolio_core::ids::FilingId,
    govfolio_core::ids::PoliticianId,
    govfolio_core::ids::RegimeId,
)> {
    let (_, filing, politician) = CONFORMANCE_FILINGS
        .iter()
        .find(|(id, _, _)| *id == mdb_id)
        .with_context(|| format!("no conformance ids for mdb {mdb_id} — never guess"))?;
    ids::resolve(mode, filing, politician, CONFORMANCE_REGIME_ID)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn income_split_separates_role_from_amount() {
        let (role, income) =
            split_income("Rechtsanwalt, monatlich, 1.000,00 EUR, Gewinnausschüttung");
        assert_eq!(role, "Rechtsanwalt");
        assert_eq!(
            income.as_deref(),
            Some("monatlich, 1.000,00 EUR, Gewinnausschüttung")
        );
        let (role2, income2) = split_income("Mitglied des Beirates, ehrenamtlich");
        assert_eq!(role2, "Mitglied des Beirates, ehrenamtlich");
        assert!(income2.is_none());
    }

    #[test]
    fn german_amount_parses_first_figure_only() {
        let (period, year, value) = parse_income(
            "monatlich, 300,00 EUR, Brutto; zuzüglich Summe unregelmäßiger Zahlungen 2025: 89,55 EUR, Brutto",
        );
        assert_eq!(period, Some(Period::Monthly));
        assert_eq!(year, None);
        assert_eq!(value.map(|d| d.to_string()).as_deref(), Some("300.00"));
    }

    #[test]
    fn category_headings_map_by_prefix() {
        assert_eq!(
            category_number("Entgeltliche Tätigkeiten neben dem Mandat"),
            Some(2)
        );
        assert_eq!(
            category_number("Funktionen in Körperschaften und Anstalten des öffentlichen Rechts"),
            Some(4)
        );
        assert_eq!(
            category_number("Beteiligungen an Kapital- oder Personengesellschaften"),
            Some(7)
        );
        assert_eq!(category_number("Nonsense"), None);
    }
}
