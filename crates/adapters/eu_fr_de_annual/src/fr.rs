//! §FR — France HATVP déclaration d'intérêts et d'activités (DIA). A deterministic
//! `quick-xml` parse (no LLM seam, §FR.6): the Bronze document is well-formed XML
//! with a fixed, self-describing schema. `parse` walks the typed sections in
//! document order (one Silver row per declared item, unknown section ⇒ freeze),
//! stripping the excluded `activCollaborateursDto`/`observationInteretDto`.
//! `normalize` maps the LATEST declared year's `montant` (exact EUR when > 0) to
//! `value`; `activProfConjointDto` → owner spouse; `participationFinanciereDto`
//! → `asset_class` equity (its `evaluation` is NOT promoted to value).

use std::collections::BTreeMap;

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
use crate::util::{parse_amount, parse_ddmmyyyy};

/// Extractor tag recorded on every DIA Silver row (§FR.4).
pub(crate) const EXTRACTOR: &str = "fr_hatvp_dia/xml@1";
/// Regime code / `disclosure_regime` slug (§0).
pub(crate) const REGIME: &str = "fr_hatvp_dia";
/// Fixed conformance regime ULID (fixtures `MANIFEST.json`).
const CONFORMANCE_REGIME_ID: &str = "0FRHREG0000000000000000001";

/// Document sha256 → declaration stem (external id). The stem is NOT in the XML
/// body — production threads it from the discovery `FilingRef`; conformance pins
/// it here (fixtures `MANIFEST.json`).
const CONFORMANCE_STEMS: &[(&str, &str)] = &[
    (
        "ec4003d075953b88af1b69c03721537a92e27647a2ef073ffd915318c3a553c3",
        "lahmar-abdelkader-dia31320-depute-69",
    ),
    (
        "eacd17ff864b807344b12bcfd608fa1181456210cd75937d1e036b7c03cdee05",
        "pannier-runacher-agnes-dia34763-depute-62",
    ),
    (
        "af840b0294d0cbc146c982f8b39170dbdffe993827686ff8c141aaf0c33ac887",
        "lahmar-abdelkader-diam31323-depute-69",
    ),
];

/// Declaration stem → (conformance filing ULID, politician ULID). The two Lahmar
/// filings share one politician (fixtures `MANIFEST.json`).
const CONFORMANCE_FILINGS: &[(&str, &str, &str)] = &[
    (
        "pannier-runacher-agnes-dia34763-depute-62",
        "0FRHFNG0000000000000000001",
        "0FRHMBR0000000000000000001",
    ),
    (
        "lahmar-abdelkader-dia31320-depute-69",
        "0FRHFNG0000000000000000002",
        "0FRHMBR0000000000000000002",
    ),
    (
        "lahmar-abdelkader-diam31323-depute-69",
        "0FRHFNG0000000000000000003",
        "0FRHMBR0000000000000000002",
    ),
];

/// Item motif (§FR.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Motif {
    /// A newly created declaration item.
    Creation,
    /// A modification of a prior item.
    Modification,
    /// A suppression of a prior item.
    Suppression,
}

/// §FR.5 value-rule provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum FrValueSource {
    /// `value` = the latest declared year's `montant` (exact EUR).
    #[serde(rename = "montant_latest_year")]
    MontantLatestYear,
    /// No positive remuneration on the item — `value` NULL.
    #[serde(rename = "none")]
    NoneDeclared,
}

/// One declared year's remuneration, amount normalized to a decimal string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Montant {
    /// Declared year.
    pub year: i64,
    /// Amount as a decimal string (space thousands stripped; invariant 7).
    pub amount: String,
}

/// `details` payload of one DIA item (§FR.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FrHatvpDiaInterestDetailsV1 {
    /// Open-data filename stem (external id).
    pub declaration_stem: String,
    /// `<uuid>` verbatim.
    pub declaration_uuid: String,
    /// `DIA` or `DIAM` (from the filename stem).
    pub type_declaration: String,
    /// `declarationModificative` bool.
    pub is_modificative: bool,
    /// The `<...Dto>` section tag (§FR.3 vocab).
    pub section_tag: String,
    /// 1-based document order.
    #[schemars(range(min = 1))]
    pub row_ordinal: u32,
    /// Item motif.
    pub motif: Motif,
    /// The item's scalar child elements, verbatim whitespace-normalized (the
    /// per-section heterogeneous children; `australia` `entry_fields` precedent).
    pub entry_fields: BTreeMap<String, String>,
    /// Per-year remuneration amounts (decimal strings); empty when the item
    /// carries no `remuneration` year array.
    pub montants: Vec<Montant>,
    /// `remuneration/brutNet` (`Net`/`Brut`); null when no remuneration array.
    pub brut_net: Option<String>,
    /// `general/organe/labelOrgane` (département); null when absent.
    pub organe: Option<String>,
    /// Ingestion language: always `fr`.
    pub language: String,
    /// §FR.5 value provenance.
    pub value_source: FrValueSource,
}

/// JSON Schema for the `(fr_hatvp_dia, interest)` details contract.
#[must_use]
pub fn interest_details_schema() -> schemars::Schema {
    schemars::schema_for!(FrHatvpDiaInterestDetailsV1)
}

/// One year's remuneration in Silver — source-shaped (`annee`/`montant`,
/// amount kept verbatim including space thousands).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawMontant {
    annee: i64,
    montant: String,
}

/// The item's `remuneration` block in Silver — source-shaped.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Remuneration {
    #[serde(rename = "brutNet")]
    brut_net: String,
    montant: Vec<RawMontant>,
}

/// One DIA Silver staging row (§FR.4).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SilverRow {
    declaration_stem: String,
    declaration_uuid: String,
    type_declaration: String,
    is_modificative: bool,
    type_mandat_raw: String,
    organe_raw: Option<String>,
    declarant_nom_raw: String,
    declarant_prenom_raw: String,
    date_depot_raw: String,
    section_tag: String,
    row_ordinal: u32,
    motif_raw: String,
    entry_fields_raw: BTreeMap<String, String>,
    remuneration_raw: Option<Remuneration>,
    neant_section: bool,
    extractor: String,
}

/// Sections promoted to Silver, with the child element that becomes the Gold
/// `asset_description_raw` (§FR.3, MANIFEST `builder_notes.fr`).
fn primary_field(section: &str) -> Option<&'static str> {
    Some(match section {
        "activProfCinqDerniereDto" | "activConsultantDto" => "description",
        "activProfConjointDto" => "activiteProf",
        "fonctionBenevoleDto" => "nomStructure",
        "mandatElectifDto" => "descriptionMandat",
        "participationDirigeantDto" | "participationFinanciereDto" => "nomSociete",
        _ => return None,
    })
}

/// Sections deliberately excluded from Silver/Gold (§FR.3; MANIFEST flags).
const EXCLUDED_SECTIONS: &[&str] = &["activCollaborateursDto", "observationInteretDto"];

/// Bronze XML → Silver (§FR.4). `sha256` binds the conformance declaration stem.
pub(crate) fn parse(bytes: &[u8], sha256: &str) -> anyhow::Result<Vec<StagingRow>> {
    let stem = CONFORMANCE_STEMS
        .iter()
        .find(|(sha, _)| *sha == sha256)
        .map(|(_, stem)| (*stem).to_owned())
        .with_context(|| {
            format!(
                "no runner binding for DIA document {sha256} — production threads the \
                 declaration stem from the discovery FilingRef (follow-up); freeze (invariant 6)"
            )
        })?;

    let root = dom::parse(bytes).context("parsing HATVP DIA XML")?;
    let declaration = root
        .child("declaration")
        .context("DIA XML has no <declaration> root — freeze")?;
    let general = declaration
        .child("general")
        .context("DIA declaration has no <general> block — freeze")?;

    let type_declaration = type_from_stem(&stem)?;
    let declaration_uuid = text_of(declaration.child("uuid"));
    let declarant = general.child("declarant");
    let meta = Meta {
        declaration_stem: stem.clone(),
        declaration_uuid,
        type_declaration,
        is_modificative: text_of(general.child("declarationModificative")) == "true",
        type_mandat_raw: text_of(
            general
                .child("qualiteMandat")
                .and_then(|q| q.child("codTypeMandatFichier")),
        ),
        organe_raw: opt_text(general.child("organe").and_then(|o| o.child("labelOrgane"))),
        declarant_nom_raw: text_of(declarant.and_then(|d| d.child("nom"))),
        declarant_prenom_raw: text_of(declarant.and_then(|d| d.child("prenom"))),
        date_depot_raw: text_of(declaration.child("dateDepot")),
    };

    let mut rows = Vec::new();
    let mut ordinal: u32 = 0;
    for section in &declaration.children {
        if !section.name.ends_with("Dto") {
            continue;
        }
        if EXCLUDED_SECTIONS.contains(&section.name.as_str()) {
            continue;
        }
        anyhow::ensure!(
            primary_field(&section.name).is_some(),
            "unknown DIA section {:?} outside the §FR.3 vocabulary — freeze (invariant 6)",
            section.name
        );
        let Some(items_wrapper) = section.child("items") else {
            continue; // neant / empty section
        };
        for item in items_wrapper.children_named("items") {
            ordinal += 1;
            rows.push(build_silver_row(&meta, &section.name, ordinal, item)?);
        }
    }

    anyhow::ensure!(
        !rows.is_empty(),
        "DIA parse produced zero rows for {sha256} — freeze (invariant 6)"
    );
    Ok(rows)
}

struct Meta {
    declaration_stem: String,
    declaration_uuid: String,
    type_declaration: String,
    is_modificative: bool,
    type_mandat_raw: String,
    organe_raw: Option<String>,
    declarant_nom_raw: String,
    declarant_prenom_raw: String,
    date_depot_raw: String,
}

fn build_silver_row(
    meta: &Meta,
    section_tag: &str,
    ordinal: u32,
    item: &Node,
) -> anyhow::Result<StagingRow> {
    let mut motif_raw = String::new();
    let mut remuneration_raw = None;
    let mut entry_fields_raw = BTreeMap::new();

    for child in &item.children {
        match child.name.as_str() {
            "motif" => motif_raw = text_of(child.child("id")),
            "remuneration" if child.has_children() => {
                remuneration_raw = Some(parse_remuneration(child)?);
            }
            _ => {
                anyhow::ensure!(
                    !child.has_children(),
                    "unexpected complex child {:?} in DIA {section_tag} item — freeze",
                    child.name
                );
                let value = dom::normalize_ws(&child.text);
                if !value.is_empty() {
                    entry_fields_raw.insert(child.name.clone(), value);
                }
            }
        }
    }

    let row = SilverRow {
        declaration_stem: meta.declaration_stem.clone(),
        declaration_uuid: meta.declaration_uuid.clone(),
        type_declaration: meta.type_declaration.clone(),
        is_modificative: meta.is_modificative,
        type_mandat_raw: meta.type_mandat_raw.clone(),
        organe_raw: meta.organe_raw.clone(),
        declarant_nom_raw: meta.declarant_nom_raw.clone(),
        declarant_prenom_raw: meta.declarant_prenom_raw.clone(),
        date_depot_raw: meta.date_depot_raw.clone(),
        section_tag: section_tag.to_owned(),
        row_ordinal: ordinal,
        motif_raw,
        entry_fields_raw,
        remuneration_raw,
        neant_section: false,
        extractor: EXTRACTOR.to_owned(),
    };

    Ok(StagingRow {
        payload: serde_json::to_value(&row).context("serializing DIA silver row")?,
        confidence: 1.0,
    })
}

fn parse_remuneration(node: &Node) -> anyhow::Result<Remuneration> {
    let brut_net = text_of(node.child("brutNet"));
    let mut montant = Vec::new();
    if let Some(wrapper) = node.child("montant") {
        for m in wrapper.children_named("montant") {
            let annee: i64 = dom::normalize_ws(&text_of(m.child("annee")))
                .parse()
                .with_context(|| "unparseable montant annee — freeze")?;
            montant.push(RawMontant {
                annee,
                montant: dom::normalize_ws(&text_of(m.child("montant"))),
            });
        }
    }
    Ok(Remuneration { brut_net, montant })
}

/// Silver → Gold (§FR.5).
pub(crate) fn normalize(rows: &[StagingRow], ctx: &RunCtx) -> anyhow::Result<Vec<GoldCandidate>> {
    let mode = IdentityMode::of(ctx);
    rows.iter().map(|row| normalize_row(row, mode)).collect()
}

fn normalize_row(staged: &StagingRow, mode: IdentityMode) -> anyhow::Result<GoldCandidate> {
    let row: SilverRow = serde_json::from_value(staged.payload.clone())
        .context("silver payload is not a DIA staging row")?;

    let primary = primary_field(&row.section_tag).with_context(|| {
        format!(
            "unknown DIA section {:?} at normalize — freeze",
            row.section_tag
        )
    })?;
    let asset_description_raw = row
        .entry_fields_raw
        .get(primary)
        .filter(|v| !v.is_empty())
        .with_context(|| {
            format!(
                "DIA {} row {} has no {primary} — reject (invariant 2)",
                row.section_tag, row.row_ordinal
            )
        })?
        .clone();

    let owner = if row.section_tag == "activProfConjointDto" {
        Owner::Spouse
    } else {
        Owner::Self_
    };
    let asset_class = if row.section_tag == "participationFinanciereDto" {
        AssetClass::Equity
    } else {
        AssetClass::Other
    };

    let mut montants = Vec::new();
    let mut brut_net = None;
    if let Some(rem) = &row.remuneration_raw {
        brut_net = Some(rem.brut_net.clone());
        for m in &rem.montant {
            let amount = parse_amount(&m.montant, false)
                .with_context(|| format!("unparseable DIA montant {:?}", m.montant))?;
            montants.push(Montant {
                year: m.annee,
                amount: amount.to_string(),
            });
        }
    }

    let (value, value_source) = latest_year_value(row.remuneration_raw.as_ref())?;

    let motif = parse_motif(&row.motif_raw)?;

    let details = FrHatvpDiaInterestDetailsV1 {
        declaration_stem: row.declaration_stem.clone(),
        declaration_uuid: row.declaration_uuid.clone(),
        type_declaration: row.type_declaration.clone(),
        is_modificative: row.is_modificative,
        section_tag: row.section_tag.clone(),
        row_ordinal: row.row_ordinal,
        motif,
        entry_fields: row.entry_fields_raw.clone(),
        montants,
        brut_net,
        organe: row.organe_raw.clone(),
        language: "fr".to_owned(),
        value_source,
    };

    let notified_date = parse_ddmmyyyy(&row.date_depot_raw);
    let (filing_id, politician_id, regime_id) = resolve_ids(mode, &row.declaration_stem)?;

    Ok(GoldCandidate {
        filing_id,
        politician_id,
        regime_id,
        instrument_id: None,
        asset_description_raw,
        record_type: RecordType::Interest,
        asset_class,
        side: None,
        transaction_date: None,
        as_of_date: None,
        notified_date,
        value,
        owner: Some(owner),
        extraction_confidence: Some(staged.confidence),
        extracted_by: row.extractor.clone(),
        fingerprint: None,
        details: serde_json::to_value(details).context("serializing DIA details")?,
    })
}

/// The latest declared year's `montant` → exact EUR when > 0 (§FR.5), else NULL.
fn latest_year_value(
    rem: Option<&Remuneration>,
) -> anyhow::Result<(Option<ValueInterval>, FrValueSource)> {
    let Some(rem) = rem else {
        return Ok((None, FrValueSource::NoneDeclared));
    };
    let Some(latest) = rem.montant.iter().max_by_key(|m| m.annee) else {
        return Ok((None, FrValueSource::NoneDeclared));
    };
    let amount = parse_amount(&latest.montant, false)
        .with_context(|| format!("unparseable latest DIA montant {:?}", latest.montant))?;
    if amount > Decimal::ZERO {
        let interval = ValueInterval::new(amount, Some(amount), Currency::EUR)
            .map_err(|e| anyhow::anyhow!("bad DIA value {amount}: {e}"))?;
        Ok((Some(interval), FrValueSource::MontantLatestYear))
    } else {
        Ok((None, FrValueSource::NoneDeclared))
    }
}

fn parse_motif(raw: &str) -> anyhow::Result<Motif> {
    Ok(match raw {
        "CREATION" => Motif::Creation,
        "MODIFICATION" => Motif::Modification,
        "SUPPRESSION" => Motif::Suppression,
        other => anyhow::bail!("unknown DIA motif {other:?} — freeze (invariant 6)"),
    })
}

/// `DIA`/`DIAM` from the filename stem's `dia{n}`/`diam{n}` segment (§FR flags:
/// the XML `typeDeclaration/id` is always `DIA`, so the stem is authoritative).
fn type_from_stem(stem: &str) -> anyhow::Result<String> {
    for seg in stem.split('-') {
        let alpha: String = seg.chars().take_while(char::is_ascii_alphabetic).collect();
        let rest = &seg[alpha.len()..];
        if (alpha == "dia" || alpha == "diam")
            && !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_digit())
        {
            return Ok(alpha.to_uppercase());
        }
    }
    anyhow::bail!("declaration stem {stem:?} has no dia/diam segment — freeze")
}

fn resolve_ids(
    mode: IdentityMode,
    stem: &str,
) -> anyhow::Result<(
    govfolio_core::ids::FilingId,
    govfolio_core::ids::PoliticianId,
    govfolio_core::ids::RegimeId,
)> {
    let (_, filing, politician) = CONFORMANCE_FILINGS
        .iter()
        .find(|(s, _, _)| *s == stem)
        .with_context(|| format!("no conformance ids for DIA stem {stem:?} — never guess"))?;
    ids::resolve(mode, filing, politician, CONFORMANCE_REGIME_ID)
}

fn text_of(node: Option<&Node>) -> String {
    node.map(|n| dom::normalize_ws(&n.text)).unwrap_or_default()
}

fn opt_text(node: Option<&Node>) -> Option<String> {
    let text = text_of(node);
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn type_declaration_reads_dia_or_diam_from_stem() {
        assert_eq!(
            type_from_stem("pannier-runacher-agnes-dia34763-depute-62").unwrap(),
            "DIA"
        );
        assert_eq!(
            type_from_stem("lahmar-abdelkader-diam31323-depute-69").unwrap(),
            "DIAM"
        );
    }

    #[test]
    fn motif_vocabulary_is_closed() {
        assert_eq!(parse_motif("CREATION").unwrap(), Motif::Creation);
        assert!(parse_motif("BOGUS").is_err());
    }
}
