//! Bronze → Silver: html5ever DOM + CSS selectors over the CIEC details page
//! (regime doc §3 anatomy, §6 strategy). The document is server-rendered
//! ASP.NET-template HTML with a small closed grammar.
//!
//! LOAD-BEARING (fixtures `MANIFEST.json` `br_whitespace_rule`): every collapsed
//! text join replaces `<br>`/`<br/>` with a SPACE *before* the `\s+`→single-space
//! collapse. Several family-C `<br>` tags carry no surrounding whitespace
//! (`months:<br>- Employment…`); a naive `.text()` concatenation would corrupt
//! the row. Inline elements (`<i>`/`<a>`/`<strong>`) add NO separator.
//!
//! Hard rejects (regime doc §3.10, not confidence scores): h1↔type mismatch,
//! a `Regime` value outside the two instruments, disclosure/footer date
//! disagreement, a family-C wrapper GUID ≠ the requested id, a missing item
//! GUID, an unknown section label, a zero-row parse — freeze + `review_task`,
//! never a low-confidence Gold row (invariant 6).

use std::collections::BTreeMap;

use anyhow::Context as _;
use chrono::NaiveDate;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

use crate::tables::{self, Family};

/// Extractor id recorded on every Silver row (regime doc §4/§6 — deterministic
/// scraper path, no LLM seam on the green path).
pub(crate) const EXTRACTOR: &str = "canada_ciec/html@1";

/// One `stg_canada_ciec` payload: source-faithful verbatim strings (regime doc
/// §4). Confidence lives on the [`pipeline::adapter::StagingRow`] wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SilverRow {
    pub(crate) declaration_id: String,
    pub(crate) row_ordinal: u32,
    pub(crate) item_id_raw: Option<String>,
    pub(crate) section_label_raw: Option<String>,
    pub(crate) h1_title_raw: String,
    pub(crate) declaration_type_raw: String,
    pub(crate) law_raw: String,
    pub(crate) client_id: String,
    pub(crate) client_name_raw: String,
    pub(crate) client_title_raw: Option<String>,
    pub(crate) disclosure_date_raw: String,
    pub(crate) no_longer_applicable: bool,
    pub(crate) ociec_translation: bool,
    pub(crate) description_raw: Option<String>,
    pub(crate) fields_raw: BTreeMap<String, String>,
    pub(crate) extractor: String,
}

/// A Silver row plus its §6 rule-based confidence score.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScoredRow {
    pub(crate) row: SilverRow,
    pub(crate) confidence: f32,
}

/// Common card-header fields shared by every row of a document.
#[derive(Debug, Clone)]
struct CardHeader {
    h1_title_raw: String,
    declaration_type_raw: String,
    law_raw: String,
    client_id: String,
    client_name_raw: String,
    client_title_raw: Option<String>,
    disclosure_date_raw: String,
    no_longer_applicable: bool,
    ociec_translation: bool,
}

/// `Date of change: YYYY/MM/DD …` (SLASH format, regime doc §3.7) → the ISO
/// date. `None` when the line is absent/unparseable (fail-soft). Shared by the
/// parser (−0.05 material-change scoring, §6.2) and normalize (`date_of_change`).
pub(crate) fn parse_date_of_change(text: &str) -> Option<NaiveDate> {
    let rest = text.strip_prefix("Date of change:")?.trim_start();
    let token = rest.split_whitespace().next()?;
    NaiveDate::parse_from_str(token, "%Y/%m/%d").ok()
}

/// Parses one CIEC details page into scored Silver rows. The declaration id is
/// threaded by the caller — families A/B never print it (regime doc §4); family
/// C prints it as the wrapper GUID, cross-checked here (§3.10 check 4).
pub(crate) fn parse_document(html: &str, declaration_id: &str) -> anyhow::Result<Vec<ScoredRow>> {
    let doc = Html::parse_document(html);
    let card = single_element(&doc, ".card.shadow-sm", "declaration card")?;
    let header = parse_header(&doc, card)?;

    // §3.10 integrity cross-checks — hard rejects, not scores.
    tables::law_code(&header.law_raw).with_context(|| {
        format!(
            "Regime {:?} is neither the Act nor the Code — hard reject (§3.10 check 2)",
            header.law_raw
        )
    })?;
    if let Some(expected) = tables::expected_h1(&header.declaration_type_raw) {
        anyhow::ensure!(
            header.h1_title_raw == expected,
            "h1 {:?} is inconsistent with declaration type {:?} (expected {expected:?}) — \
             hard reject (§3.10 check 1)",
            header.h1_title_raw,
            header.declaration_type_raw
        );
    }
    let footer_date = footer_disclosed_on(card)?;
    anyhow::ensure!(
        footer_date == header.disclosure_date_raw,
        "footer {footer_date:?} disagrees with the Disclosure date {:?} — hard reject (§3.10 check 3)",
        header.disclosure_date_raw
    );

    let family = tables::family_for_type(&header.declaration_type_raw).with_context(|| {
        format!(
            "declaration type {:?} is outside the v1 in-scope census — freeze + review (§3.1)",
            header.declaration_type_raw
        )
    })?;

    let payload = payload_dl(card)?;
    let base_confidence = base_confidence(&header);
    let rows = match family {
        Family::TypedFields => parse_typed(&header, declaration_id, payload, base_confidence)?,
        Family::Flat => parse_flat(&header, declaration_id, payload, base_confidence)?,
        Family::Itemized => parse_itemized(&header, declaration_id, payload, base_confidence)?,
    };
    anyhow::ensure!(
        !rows.is_empty(),
        "parsed zero rows from {declaration_id} — freeze + review (§3.10 check 5)"
    );
    Ok(rows)
}

/// §6.2 document-wide base score before per-row deductions.
fn base_confidence(header: &CardHeader) -> f32 {
    let mut base: f32 = 1.0;
    if header.no_longer_applicable {
        base -= 0.02;
    }
    if header.ociec_translation {
        base -= 0.02;
    }
    base
}

/// Parses the card header + first `<dl>` into the shared fields.
fn parse_header(doc: &Html, card: ElementRef<'_>) -> anyhow::Result<CardHeader> {
    let h1_title_raw = single_text(doc, ".content-header h1", "declaration h1")?;

    let anchor = card
        .select(&sel(".declaration-details-card-title a[href]")?)
        .next()
        .context("card header carries no client link — not a details page")?;
    let client_name_raw = element_text(anchor);
    anyhow::ensure!(
        !client_name_raw.is_empty(),
        "empty client name — hard reject"
    );
    let href = anchor
        .value()
        .attr("href")
        .context("client anchor has no href")?;
    let client_id = client_id_from_href(href).with_context(|| {
        format!("client href {href:?} has no well-formed clientId (§3.10 check 7)")
    })?;

    let client_title_raw = card
        .select(&sel(".declaration-details-card-title span.text-muted")?)
        .next()
        .map(strip_middot)
        .filter(|s| !s.is_empty());

    let no_longer_applicable = badge_present(card, ".bg-warning", "No Longer Applicable")?;
    let ociec_translation = badge_present(card, ".bg-info", "OCIEC Translation")?;

    let header_dl = card
        .select(&sel(".card-body dl.row")?)
        .next()
        .context("card body has no header dl — not a details page")?;
    let pairs = dl_pairs(header_dl)?;
    let lookup = |label: &str| -> anyhow::Result<String> {
        pairs
            .iter()
            .find(|(dt, _)| dt == label)
            .map(|(_, dd)| dd.clone())
            .with_context(|| format!("header dl missing {label:?} — hard reject"))
    };

    Ok(CardHeader {
        h1_title_raw,
        declaration_type_raw: lookup("Declaration type")?,
        law_raw: lookup("Regime")?,
        client_id,
        client_name_raw,
        client_title_raw,
        disclosure_date_raw: lookup("Disclosure date")?,
        no_longer_applicable,
        ociec_translation,
    })
}

/// Families A/A′: the second `<dl>` of typed `dt`/`dd` pairs → `fields_raw`
/// (exactly one row, regime doc §3.5).
fn parse_typed(
    header: &CardHeader,
    declaration_id: &str,
    payload: ElementRef<'_>,
    confidence: f32,
) -> anyhow::Result<Vec<ScoredRow>> {
    let pairs = dl_pairs(payload)?;
    anyhow::ensure!(
        !pairs.is_empty(),
        "typed-field declaration has no dt/dd pairs — hard reject"
    );
    let fields_raw: BTreeMap<String, String> = pairs.into_iter().collect();
    let row = silver_row(header, declaration_id, 1, None, None, None, fields_raw);
    Ok(vec![ScoredRow { row, confidence }])
}

/// Family B: the `Description` `<dd>` flat text → `description_raw` (exactly
/// one row, regime doc §3.4). `<br>`→space handles the divestment block.
fn parse_flat(
    header: &CardHeader,
    declaration_id: &str,
    payload: ElementRef<'_>,
    confidence: f32,
) -> anyhow::Result<Vec<ScoredRow>> {
    let dd = payload
        .select(&sel("dd")?)
        .next()
        .context("flat declaration has no Description dd — hard reject")?;
    let description = element_text(dd);
    anyhow::ensure!(
        !description.is_empty(),
        "empty flat Description — hard reject (invariant 2)"
    );
    let row = silver_row(
        header,
        declaration_id,
        1,
        None,
        None,
        Some(description),
        BTreeMap::new(),
    );
    Ok(vec![ScoredRow { row, confidence }])
}

/// Family C: one row per `ciec-declaration-disclosureitem`, document order
/// (regime doc §3.4/§3.5). Cross-checks the wrapper GUID (§3.10 check 4).
fn parse_itemized(
    header: &CardHeader,
    declaration_id: &str,
    payload: ElementRef<'_>,
    base_confidence: f32,
) -> anyhow::Result<Vec<ScoredRow>> {
    let dd = payload
        .select(&sel("dd")?)
        .next()
        .context("itemized declaration has no Description dd — hard reject")?;
    let wrapper = dd
        .select(&sel("div[id][title]")?)
        .next()
        .context("itemized dd has no titled wrapper div — hard reject")?;
    let wrapper_id = wrapper
        .value()
        .attr("id")
        .context("wrapper div has no id")?;
    anyhow::ensure!(
        wrapper_id.eq_ignore_ascii_case(declaration_id),
        "wrapper GUID {wrapper_id:?} ≠ requested declaration {declaration_id:?} — hard reject (§3.10 check 4)"
    );

    let is_material_change = tables::is_material_change(&header.declaration_type_raw);
    let mut rows = Vec::new();
    let mut ordinal: u32 = 0;
    for field in wrapper.select(&sel(".ciec-summary-field")?) {
        let label = field
            .select(&sel(".ciec-declaration-disclosurelabel")?)
            .next()
            .map(element_text)
            .filter(|s| !s.is_empty())
            .context("summary field has an empty section label — hard reject (§3.10 check 4)")?;
        // §3.5: an unknown label is rejected here, never a low-confidence row.
        let (_, is_dependent) = tables::family_c_owner(&label).with_context(|| {
            format!(
                "section label {label:?} is outside the archived grammar — reject → review (§3.5)"
            )
        })?;

        for item in field.select(&sel(".ciec-declaration-disclosureitem")?) {
            let item_id = item
                .value()
                .attr("id")
                .context("disclosure item has no GUID id — hard reject (§3.10 check 4)")?
                .to_owned();
            let description = element_text(item);
            anyhow::ensure!(
                !description.is_empty(),
                "empty disclosure item — hard reject"
            );

            let mut confidence = base_confidence;
            if is_dependent {
                confidence -= 0.05;
            }
            if is_material_change && parse_date_of_change(&description).is_none() {
                confidence -= 0.05; // date lost to details; raw text survives (§6.2)
            }

            ordinal += 1;
            let row = silver_row(
                header,
                declaration_id,
                ordinal,
                Some(item_id),
                Some(label.clone()),
                Some(description),
                BTreeMap::new(),
            );
            rows.push(ScoredRow { row, confidence });
        }
    }
    Ok(rows)
}

/// Assembles a [`SilverRow`] from the shared header and per-row payload.
fn silver_row(
    header: &CardHeader,
    declaration_id: &str,
    row_ordinal: u32,
    item_id_raw: Option<String>,
    section_label_raw: Option<String>,
    description_raw: Option<String>,
    fields_raw: BTreeMap<String, String>,
) -> SilverRow {
    SilverRow {
        declaration_id: declaration_id.to_owned(),
        row_ordinal,
        item_id_raw,
        section_label_raw,
        h1_title_raw: header.h1_title_raw.clone(),
        declaration_type_raw: header.declaration_type_raw.clone(),
        law_raw: header.law_raw.clone(),
        client_id: header.client_id.clone(),
        client_name_raw: header.client_name_raw.clone(),
        client_title_raw: header.client_title_raw.clone(),
        disclosure_date_raw: header.disclosure_date_raw.clone(),
        no_longer_applicable: header.no_longer_applicable,
        ociec_translation: header.ociec_translation,
        description_raw,
        fields_raw,
        extractor: EXTRACTOR.to_owned(),
    }
}

/// The payload (second) `<dl>` in the card body (regime doc §3.3 item 5).
fn payload_dl(card: ElementRef<'_>) -> anyhow::Result<ElementRef<'_>> {
    card.select(&sel(".card-body dl.row")?)
        .nth(1)
        .context("card body has no payload dl (after the <hr>) — hard reject")
}

/// Footer `Disclosed on YYYY-MM-DD` date (regime doc §3.3 item 6).
fn footer_disclosed_on(card: ElementRef<'_>) -> anyhow::Result<String> {
    let footer = single_child_text(card, ".card-footer", "card footer")?;
    footer
        .strip_prefix("Disclosed on ")
        .map(str::to_owned)
        .with_context(|| format!("footer {footer:?} lacks the `Disclosed on` label — hard reject"))
}

/// `clientId` GUID (lowercase) from a `/en/client?clientId={guid}` href,
/// shape-checked (regime doc §3.10 check 7).
fn client_id_from_href(href: &str) -> Option<String> {
    let (_, rest) = href.split_once("clientId=")?;
    let guid = rest.split(['&', '#']).next()?;
    is_guid(guid).then(|| guid.to_owned())
}

/// True for a lowercase 8-4-4-4-12 hex GUID.
fn is_guid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(at, c)| {
            if matches!(at, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit() && !c.is_ascii_uppercase()
            }
        })
}

/// True when a `.bg-*` badge with the exact `text` sits inside the card.
fn badge_present(card: ElementRef<'_>, css: &'static str, text: &str) -> anyhow::Result<bool> {
    Ok(card.select(&sel(css)?).any(|b| element_text(b) == text))
}

/// `dt`/`dd` pairs of a `<dl>` in document order (equal counts required).
fn dl_pairs(dl: ElementRef<'_>) -> anyhow::Result<Vec<(String, String)>> {
    let dts: Vec<String> = dl.select(&sel("dt")?).map(element_text).collect();
    let dds: Vec<String> = dl.select(&sel("dd")?).map(element_text).collect();
    anyhow::ensure!(
        dts.len() == dds.len(),
        "dl has {} dt but {} dd — malformed, hard reject",
        dts.len(),
        dds.len()
    );
    Ok(dts.into_iter().zip(dds).collect())
}

/// Card-header `<span class="text-muted">` → title, leading `·` (`&middot;`)
/// and whitespace stripped (fixtures `MANIFEST.json` `client_title_rule`).
fn strip_middot(span: ElementRef<'_>) -> String {
    let collapsed = element_text(span);
    collapsed
        .strip_prefix('\u{00B7}')
        .unwrap_or(&collapsed)
        .trim()
        .to_owned()
}

/// Compiles a static CSS selector.
fn sel(css: &'static str) -> anyhow::Result<Selector> {
    Selector::parse(css).map_err(|e| anyhow::anyhow!("selector {css:?}: {e}"))
}

/// The single element matching `css` under `doc` — zero or several is a
/// template fork (escalation criterion §6.4), a hard reject.
fn single_element<'a>(
    doc: &'a Html,
    css: &'static str,
    what: &str,
) -> anyhow::Result<ElementRef<'a>> {
    let selector = sel(css)?;
    let mut matches = doc.select(&selector);
    let first = matches
        .next()
        .with_context(|| format!("missing {what} ({css}) — not a CIEC details page"))?;
    anyhow::ensure!(
        matches.next().is_none(),
        "multiple {what} elements ({css}) — template fork, hard reject (§6.4)"
    );
    Ok(first)
}

/// Collapsed text of the single element matching `css` under `doc`.
fn single_text(doc: &Html, css: &'static str, what: &str) -> anyhow::Result<String> {
    let text = element_text(single_element(doc, css, what)?);
    anyhow::ensure!(!text.is_empty(), "empty {what} ({css}) — hard reject");
    Ok(text)
}

/// Collapsed text of the first descendant of `parent` matching `css`.
fn single_child_text(
    parent: ElementRef<'_>,
    css: &'static str,
    what: &str,
) -> anyhow::Result<String> {
    let element = parent
        .select(&sel(css)?)
        .next()
        .with_context(|| format!("missing {what} ({css}) — hard reject"))?;
    let text = element_text(element);
    anyhow::ensure!(!text.is_empty(), "empty {what} ({css}) — hard reject");
    Ok(text)
}

/// Whitespace-collapse: `\s+` → single space, trimmed (NBSP collapses too).
fn collapse(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collapsed text of an element's subtree with the LOAD-BEARING `<br>`→space
/// rule: `<br>`/`<br/>` become a space boundary *before* the collapse; inline
/// elements add no separator (module header + MANIFEST `br_whitespace_rule`).
fn element_text(element: ElementRef<'_>) -> String {
    let mut raw = String::new();
    for node in element.descendants() {
        let value = node.value();
        if let Some(text) = value.as_text() {
            raw.push_str(text);
        } else if value.as_element().is_some_and(|e| e.name() == "br") {
            raw.push(' ');
        }
    }
    collapse(&raw)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn br_becomes_a_space_before_collapse() {
        // The MANIFEST trap: <br> with no surrounding whitespace must not fuse.
        let doc = Html::parse_document(
            "<div id=\"x\">Last and next 12 months:<br>- Employment income<br>- Rental income</div>",
        );
        let el = doc.select(&sel("#x").unwrap()).next().unwrap();
        assert_eq!(
            element_text(el),
            "Last and next 12 months: - Employment income - Rental income"
        );
    }

    #[test]
    fn double_br_collapses_and_inline_tags_do_not_separate() {
        let doc = Html::parse_document(
            "<div id=\"x\">comply with the <i>Conflict of Interest Act</i>.<br><br>Divestment of:<br>- one</div>",
        );
        let el = doc.select(&sel("#x").unwrap()).next().unwrap();
        assert_eq!(
            element_text(el),
            "comply with the Conflict of Interest Act. Divestment of: - one"
        );
    }

    #[test]
    fn client_id_extracts_and_shape_checks() {
        assert_eq!(
            client_id_from_href("/en/client?clientId=5b99c2bd-7b2a-f011-8195-001dd8b72449"),
            Some("5b99c2bd-7b2a-f011-8195-001dd8b72449".to_owned())
        );
        assert_eq!(client_id_from_href("/en/client?clientId=NOT-A-GUID"), None);
        assert_eq!(client_id_from_href("/en/client"), None);
    }

    #[test]
    fn date_of_change_parses_slash_format() {
        assert_eq!(
            parse_date_of_change("Date of change: 2025/07/22 I am now President")
                .unwrap()
                .to_string(),
            "2025-07-22"
        );
        assert_eq!(parse_date_of_change("No date here"), None);
        assert_eq!(
            parse_date_of_change("Date of change: 2025-07-22"),
            None,
            "ISO ≠ slash"
        );
    }

    #[test]
    fn middot_is_stripped_from_the_title_span() {
        let doc = Html::parse_document(
            "<span class=\"text-muted\"> \u{00B7} Secretary of State (Defence Procurement)</span>",
        );
        let span = doc.select(&sel("span").unwrap()).next().unwrap();
        assert_eq!(
            strip_middot(span),
            "Secretary of State (Defence Procurement)"
        );
    }
}
