//! Bronze → Silver: html5ever DOM + CSS selectors over the electronic PTR
//! view page (regime doc §3 anatomy, §6 strategy). The document is
//! server-rendered Django-template HTML with a fixed 9-column table — text
//! extraction decodes entities (`&amp;` → `&`) and every text-node join is
//! whitespace-collapsed (the template hard-wraps inside elements).
//!
//! Hard rejects (regime doc §3.7 + §6.2): summary/row-count disagreement,
//! owner-distribution mismatch, non-contiguous printed `#` sequence, a
//! non-PTR title, unknown Owner/Type tokens, bands outside the grammar,
//! unparseable dates — errors, never low-confidence rows (invariant 6 over
//! confidence). The `--` empty-cell sentinel maps to NULL (§3.1).

use anyhow::Context as _;
use chrono::NaiveDate;
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};

use crate::tables;

/// Extractor id recorded on every Silver row (regime doc §4). The LLM seam
/// tag `us_senate_ptr/llm@1` is reserved for paper filings (§3.8).
pub(crate) const EXTRACTOR: &str = "us_senate_ptr/html@1";

/// One `stg_us_senate` payload: source-faithful verbatim strings, regime doc
/// §4 field for field (confidence lives on the
/// [`pipeline::adapter::StagingRow`] wrapper, not here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SilverRow {
    pub(crate) report_uuid: String,
    pub(crate) row_ordinal: u32,
    pub(crate) row_number_raw: String,
    pub(crate) report_title_raw: String,
    pub(crate) filer_name_raw: String,
    pub(crate) filed_at_raw: String,
    pub(crate) owner_raw: String,
    pub(crate) ticker_raw: Option<String>,
    pub(crate) asset_name_raw: String,
    pub(crate) asset_detail_raw: Option<String>,
    pub(crate) asset_type_raw: String,
    pub(crate) transaction_type_raw: String,
    pub(crate) transaction_date_raw: String,
    pub(crate) amount_raw: String,
    pub(crate) comment_raw: Option<String>,
    pub(crate) extractor: String,
}

/// A Silver row plus its §6 rule-based confidence score.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScoredRow {
    pub(crate) row: SilverRow,
    pub(crate) confidence: f32,
}

/// `Periodic Transaction Report for MM/DD/YYYY` (+ ` (Amendment N)`) →
/// (report date, amendment number). Anything else is a hard reject
/// (§3.7 check 4); normalize re-derives `details.amendment_number` from the
/// verbatim Silver title through this same grammar (§3.6).
pub(crate) fn title_parts(title: &str) -> anyhow::Result<(NaiveDate, Option<u32>)> {
    let rest = title
        .strip_prefix("Periodic Transaction Report for ")
        .with_context(|| format!("h1 {title:?} is not a PTR title — hard reject (§3.7)"))?;
    let (date_part, amendment) = match rest.split_once(" (Amendment ") {
        Some((date, tail)) => {
            let number = tail
                .strip_suffix(')')
                .with_context(|| format!("unterminated amendment suffix in title {title:?}"))?;
            let number: u32 = number
                .parse()
                .with_context(|| format!("amendment number in title {title:?}"))?;
            (date, Some(number))
        }
        None => (rest, None),
    };
    let date = NaiveDate::parse_from_str(date_part, "%m/%d/%Y")
        .with_context(|| format!("title date {date_part:?} is not MM/DD/YYYY — hard reject"))?;
    Ok((date, amendment))
}

/// Parses one electronic PTR view page into scored Silver rows. The report
/// UUID is threaded by the caller — the page never prints it (regime doc §4);
/// URL-vs-discovery identity (§3.7 check 4, second half) is the pipeline's
/// check for the same reason.
pub(crate) fn parse_document(html: &str, report_uuid: &str) -> anyhow::Result<Vec<ScoredRow>> {
    let doc = Html::parse_document(html);

    let report_title_raw = single_text(&doc, "h1.mb-2", "report title h1")?;
    title_parts(&report_title_raw)?; // §3.7 check 4
    let filer_name_raw = single_text(&doc, "h2.filedReport", "filer h2")?;
    let filed_line = single_text(&doc, "p.muted strong", "Filed stamp")?;
    let filed_at_raw = filed_line
        .strip_prefix("Filed ")
        .with_context(|| format!("Filed stamp {filed_line:?} lacks the `Filed` label"))?
        .to_owned();

    let summary = parse_summary(&doc)?;

    let table = single_element(&doc, "table.table", "transactions table")?;
    let tr_selector = selector("tbody > tr")?;
    let mut drafts = Vec::new();
    for tr in table.select(&tr_selector) {
        drafts.push(parse_row(tr)?);
    }

    // §3.7 integrity cross-checks — document REJECTS, not scores.
    anyhow::ensure!(
        !drafts.is_empty(),
        "zero transaction rows in an electronic PTR — freeze + review (§3.7)"
    );
    anyhow::ensure!(
        drafts.len() == summary.total,
        "summary says {} transaction(s) but the table has {} row(s) — hard reject (§3.7)",
        summary.total,
        drafts.len()
    );
    let total = u32::try_from(drafts.len()).context("row count overflow")?;

    let mut owner_counts = OwnerCounts::default();
    let mut rows = Vec::with_capacity(drafts.len());
    for (index, draft) in drafts.into_iter().enumerate() {
        let row_ordinal = u32::try_from(index + 1).context("row ordinal overflow")?;

        // §3.7 check 3: printed `#` cells form a contiguous descending N..1.
        let printed: u32 = draft
            .row_number_raw
            .parse()
            .with_context(|| format!("non-numeric # cell {:?}", draft.row_number_raw))?;
        let expected = total - row_ordinal + 1;
        anyhow::ensure!(
            printed == expected,
            "printed # {printed} at document position {row_ordinal} breaks the descending \
             {total}..1 sequence (expected {expected}) — hard reject (§3.7)"
        );
        owner_counts.record(&draft.owner_raw)?;

        // §6.2 confidence scoring; hard rejects already happened at parse_row.
        let mut confidence: f32 = 1.0;
        if !tables::OBSERVED_ASSET_TYPES.contains(&draft.asset_type_raw.as_str()) {
            confidence -= 0.05; // outside the §3.5 observed vocabulary
        }
        if draft.asset_detail_raw.is_some() {
            confidence -= 0.02; // sub-line shapes beyond Rate/Coupon+Matures unverified
        }

        rows.push(ScoredRow {
            row: SilverRow {
                report_uuid: report_uuid.to_owned(),
                row_ordinal,
                row_number_raw: draft.row_number_raw,
                report_title_raw: report_title_raw.clone(),
                filer_name_raw: filer_name_raw.clone(),
                filed_at_raw: filed_at_raw.clone(),
                owner_raw: draft.owner_raw,
                ticker_raw: draft.ticker_raw,
                asset_name_raw: draft.asset_name_raw,
                asset_detail_raw: draft.asset_detail_raw,
                asset_type_raw: draft.asset_type_raw,
                transaction_type_raw: draft.transaction_type_raw,
                transaction_date_raw: draft.transaction_date_raw,
                amount_raw: draft.amount_raw,
                comment_raw: draft.comment_raw,
                extractor: EXTRACTOR.to_owned(),
            },
            confidence: confidence.clamp(0.0, 1.0),
        });
    }

    // §3.7 check 2: summary owner counts equal the per-row distribution.
    owner_counts.assert_matches(&summary)?;
    Ok(rows)
}

/// One row's cell values before scoring.
// The `_raw` postfix IS the regime-doc §4 Silver vocabulary (source-faithful
// verbatim strings) — dropping it would desynchronize names from the contract.
#[allow(clippy::struct_field_names)]
struct RowDraft {
    row_number_raw: String,
    transaction_date_raw: String,
    owner_raw: String,
    ticker_raw: Option<String>,
    asset_name_raw: String,
    asset_detail_raw: Option<String>,
    asset_type_raw: String,
    transaction_type_raw: String,
    amount_raw: String,
    comment_raw: Option<String>,
}

/// Parses the 9 cells of one `<tr>` (§3.1 grammar; §6.2 hard rejects).
fn parse_row(tr: ElementRef<'_>) -> anyhow::Result<RowDraft> {
    let td_selector = selector("td")?;
    let cells: Vec<ElementRef<'_>> = tr.select(&td_selector).collect();
    anyhow::ensure!(
        cells.len() == 9,
        "transaction row has {} cells, expected the fixed 9-column grammar — hard reject",
        cells.len()
    );

    let row_number_raw = element_text(cells[0]);
    anyhow::ensure!(
        !row_number_raw.is_empty() && row_number_raw.bytes().all(|b| b.is_ascii_digit()),
        "non-numeric # cell {row_number_raw:?} — hard reject"
    );
    let transaction_date_raw = element_text(cells[1]);
    NaiveDate::parse_from_str(&transaction_date_raw, "%m/%d/%Y").with_context(|| {
        format!("transaction date {transaction_date_raw:?} is not MM/DD/YYYY — hard reject")
    })?;
    let owner_raw = element_text(cells[2]);
    anyhow::ensure!(
        tables::owner_for_word(&owner_raw).is_some(),
        "unknown Owner word {owner_raw:?} — hard reject (§3.2)"
    );
    let ticker_raw = ticker_cell(cells[3])?;
    let (asset_name_raw, asset_detail_raw) = asset_cell(cells[4])?;
    let asset_type_raw = element_text(cells[5]);
    anyhow::ensure!(!asset_type_raw.is_empty(), "empty Asset Type cell");
    let transaction_type_raw = element_text(cells[6]);
    anyhow::ensure!(
        tables::side_for_type(&transaction_type_raw).is_some(),
        "unknown transaction Type {transaction_type_raw:?} — hard reject (§3.3)"
    );
    let amount_raw = element_text(cells[7]);
    anyhow::ensure!(
        tables::band_bounds(&amount_raw).is_some(),
        "band {amount_raw:?} outside the grammar — hard reject (invariant 6)"
    );
    let comment_raw = sentinel_cell(cells[8], "Comment")?;

    Ok(RowDraft {
        row_number_raw,
        transaction_date_raw,
        owner_raw,
        ticker_raw,
        asset_name_raw,
        asset_detail_raw,
        asset_type_raw,
        transaction_type_raw,
        amount_raw,
        comment_raw,
    })
}

/// Ticker cell (§3.1): anchor text when the cell links a ticker, `--` = NULL.
/// Any other shape is outside the grammar — hard reject, never guessed.
fn ticker_cell(cell: ElementRef<'_>) -> anyhow::Result<Option<String>> {
    let anchor_selector = selector("a")?;
    if let Some(anchor) = cell.select(&anchor_selector).next() {
        let ticker = element_text(anchor);
        anyhow::ensure!(!ticker.is_empty(), "empty Ticker anchor — hard reject");
        return Ok(Some(ticker));
    }
    let text = element_text(cell);
    anyhow::ensure!(
        text == "--",
        "Ticker cell {text:?} is neither an anchor nor the `--` sentinel — hard reject (§3.1)"
    );
    Ok(None)
}

/// Asset Name cell (§3.1): main text is the cell minus the `div.text-muted`
/// subtree; the sub-line (`Rate/Coupon`/`Matures`) is the detail.
fn asset_cell(cell: ElementRef<'_>) -> anyhow::Result<(String, Option<String>)> {
    let detail_selector = selector("div.text-muted")?;
    let detail_element = cell.select(&detail_selector).next();
    let name = text_excluding(cell, detail_element);
    anyhow::ensure!(!name.is_empty(), "empty Asset Name cell — hard reject");
    let detail = match detail_element {
        Some(div) => {
            let text = element_text(div);
            anyhow::ensure!(!text.is_empty(), "empty Asset Name sub-line — hard reject");
            Some(text)
        }
        None => None,
    };
    Ok((name, detail))
}

/// `--` → NULL; non-empty text passes through verbatim (§3.1 sentinel rule).
fn sentinel_cell(cell: ElementRef<'_>, what: &str) -> anyhow::Result<Option<String>> {
    let text = element_text(cell);
    anyhow::ensure!(!text.is_empty(), "empty {what} cell — hard reject");
    Ok((text != "--").then_some(text))
}

/// The Transactions summary list (§3 anatomy item 5): integrity input.
struct Summary {
    total: usize,
    self_count: usize,
    joint: usize,
    spouse: usize,
    dependent: usize,
}

/// Parses the 5-item summary list: `(N transaction[s] total)` then the
/// `Self`/`Joint`/`Spouse`/`Dependent Child` counts in template order.
fn parse_summary(doc: &Html) -> anyhow::Result<Summary> {
    let item_selector = selector("section.card ul li")?;
    let items: Vec<String> = doc.select(&item_selector).map(element_text).collect();
    anyhow::ensure!(
        items.len() == 5,
        "transactions summary has {} items, expected 5 — hard reject (§3.7)",
        items.len()
    );
    let total_text = items[0]
        .strip_prefix('(')
        .and_then(|t| t.strip_suffix(" total)"))
        .with_context(|| format!("unrecognized summary total {:?}", items[0]))?;
    let (count, noun) = total_text
        .split_once(' ')
        .with_context(|| format!("unrecognized summary total {:?}", items[0]))?;
    let total: usize = count
        .parse()
        .with_context(|| format!("summary total count {count:?}"))?;
    let expected_noun = if total == 1 {
        "transaction"
    } else {
        "transactions"
    };
    anyhow::ensure!(
        noun == expected_noun,
        "summary noun {noun:?} disagrees with count {total} — hard reject"
    );
    Ok(Summary {
        total,
        self_count: owner_count(&items[1], "Self")?,
        joint: owner_count(&items[2], "Joint")?,
        spouse: owner_count(&items[3], "Spouse")?,
        dependent: owner_count(&items[4], "Dependent Child")?,
    })
}

/// `<count> <label>` summary item → count.
fn owner_count(item: &str, label: &str) -> anyhow::Result<usize> {
    let count = item
        .strip_suffix(label)
        .map(str::trim)
        .with_context(|| format!("summary item {item:?} is not a {label:?} count"))?;
    count
        .parse()
        .with_context(|| format!("summary {label} count {count:?}"))
}

/// Per-row owner distribution, compared against the summary (§3.7 check 2).
#[derive(Debug, Default)]
struct OwnerCounts {
    self_count: usize,
    joint: usize,
    spouse: usize,
    dependent: usize,
}

impl OwnerCounts {
    fn record(&mut self, owner_raw: &str) -> anyhow::Result<()> {
        match owner_raw {
            "Self" => self.self_count += 1,
            "Joint" => self.joint += 1,
            "Spouse" => self.spouse += 1,
            "Child" => self.dependent += 1,
            other => anyhow::bail!("unknown Owner word {other:?} — hard reject (§3.2)"),
        }
        Ok(())
    }

    fn assert_matches(&self, summary: &Summary) -> anyhow::Result<()> {
        let pairs = [
            ("Self", self.self_count, summary.self_count),
            ("Joint", self.joint, summary.joint),
            ("Spouse", self.spouse, summary.spouse),
            ("Dependent Child", self.dependent, summary.dependent),
        ];
        for (label, rows, summarized) in pairs {
            anyhow::ensure!(
                rows == summarized,
                "summary says {summarized} {label} but the rows carry {rows} — hard reject (§3.7)"
            );
        }
        Ok(())
    }
}

/// Compiles a static CSS selector.
fn selector(css: &'static str) -> anyhow::Result<Selector> {
    Selector::parse(css).map_err(|e| anyhow::anyhow!("selector {css:?}: {e}"))
}

/// The single element matching `css` — zero or several is a template fork
/// (escalation criterion §6.4b), a hard reject.
fn single_element<'a>(
    doc: &'a Html,
    css: &'static str,
    what: &str,
) -> anyhow::Result<ElementRef<'a>> {
    let sel = selector(css)?;
    let mut matches = doc.select(&sel);
    let first = matches
        .next()
        .with_context(|| format!("missing {what} ({css}) — not an electronic PTR page"))?;
    anyhow::ensure!(
        matches.next().is_none(),
        "multiple {what} elements ({css}) — template fork, hard reject (§6.4)"
    );
    Ok(first)
}

/// Collapsed text of the single element matching `css`.
fn single_text(doc: &Html, css: &'static str, what: &str) -> anyhow::Result<String> {
    let element = single_element(doc, css, what)?;
    let text = element_text(element);
    anyhow::ensure!(!text.is_empty(), "empty {what} ({css}) — hard reject");
    Ok(text)
}

/// Whitespace-collapse: every text-node join trims and single-spaces (§4 —
/// the template hard-wraps inside elements).
fn collapse(raw: &str) -> String {
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collapsed text of an element's whole subtree (entities already decoded by
/// html5ever text nodes — MANIFEST `entity_decoding`).
fn element_text(element: ElementRef<'_>) -> String {
    collapse(&element.text().collect::<String>())
}

/// Collapsed text of `element` excluding one descendant subtree (the Asset
/// Name main text vs its `div.text-muted` sub-line).
fn text_excluding(element: ElementRef<'_>, excluded: Option<ElementRef<'_>>) -> String {
    let excluded_id = excluded.map(|e| e.id());
    let mut raw = String::new();
    for node in element.descendants() {
        if let Some(text) = node.value().as_text() {
            let inside = excluded_id.is_some_and(|id| node.ancestors().any(|a| a.id() == id));
            if !inside {
                raw.push_str(text);
            }
        }
    }
    collapse(&raw)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn titles_parse_with_and_without_amendment_suffix() {
        let (date, amendment) = title_parts("Periodic Transaction Report for 06/12/2026").unwrap();
        assert_eq!(date.to_string(), "2026-06-12");
        assert_eq!(amendment, None);
        let (date, amendment) =
            title_parts("Periodic Transaction Report for 06/16/2026 (Amendment 1)").unwrap();
        assert_eq!(date.to_string(), "2026-06-16");
        assert_eq!(amendment, Some(1));
    }

    #[test]
    fn non_ptr_titles_hard_reject() {
        assert!(title_parts("Annual Report for 2026").is_err());
        assert!(title_parts("Periodic Transaction Report for June 2026").is_err());
        assert!(
            title_parts("Periodic Transaction Report for 06/16/2026 (Amendment )").is_err(),
            "empty amendment number"
        );
    }

    fn cell_doc(td_inner: &str) -> Html {
        Html::parse_document(&format!(
            "<table><tbody><tr><td>{td_inner}</td></tr></tbody></table>"
        ))
    }

    fn first_td(doc: &Html) -> ElementRef<'_> {
        let sel = selector("td").unwrap();
        doc.select(&sel).next().unwrap()
    }

    #[test]
    fn ticker_cell_maps_anchor_sentinel_and_rejects_bare_text() {
        let doc = cell_doc(
            "\n <a href=\"https://finance.yahoo.com/quote/ORCL\"\n target=\"_blank\">ORCL</a> ",
        );
        assert_eq!(
            ticker_cell(first_td(&doc)).unwrap(),
            Some("ORCL".to_owned())
        );
        let doc = cell_doc("\n --\n ");
        assert_eq!(ticker_cell(first_td(&doc)).unwrap(), None);
        let doc = cell_doc("ORCL");
        assert!(
            ticker_cell(first_td(&doc)).is_err(),
            "bare-text ticker is outside the grammar"
        );
    }

    #[test]
    fn asset_cell_splits_name_from_sub_line_and_decodes_entities() {
        let doc = cell_doc(
            "\n EXPEDIA GROUP INC NOTE\n <div class=\"text-muted\"><em>Rate/Coupon:</em> \
             5.5%<br> <em>Matures:</em> 2036-04-15</div>\n ",
        );
        let (name, detail) = asset_cell(first_td(&doc)).unwrap();
        assert_eq!(name, "EXPEDIA GROUP INC NOTE");
        assert_eq!(
            detail.as_deref(),
            Some("Rate/Coupon: 5.5% Matures: 2036-04-15")
        );
        // Entities decode in text nodes (MANIFEST entity_decoding); the
        // name-embedded-ticker quirk stays verbatim, never split.
        let doc = cell_doc("iShares Core S&amp;P 500 ETF");
        let (name, detail) = asset_cell(first_td(&doc)).unwrap();
        assert_eq!(name, "iShares Core S&P 500 ETF");
        assert_eq!(detail, None);
        let doc = cell_doc("SPYM - Tradr 2X Long SPY Monthly ETF");
        let (name, _) = asset_cell(first_td(&doc)).unwrap();
        assert_eq!(name, "SPYM - Tradr 2X Long SPY Monthly ETF");
    }

    #[test]
    fn summary_grammar_enforces_noun_agreement() {
        assert_eq!(owner_count("18 Joint", "Joint").unwrap(), 18);
        assert!(owner_count("18 Joint", "Self").is_err());
        let doc = Html::parse_document(
            "<section class=\"card\"><ul>\
             <li>(2 transaction total)</li><li>1 Self</li><li>0 Joint</li>\
             <li>1 Spouse</li><li>0 Dependent Child</li></ul></section>",
        );
        assert!(
            parse_summary(&doc).is_err(),
            "plural count with singular noun must hard-reject"
        );
    }

    #[test]
    fn descending_row_numbers_are_enforced() {
        // A minimal 2-row document whose # cells ascend (1, 2) instead of
        // descending (2, 1) — §3.7 check 3 must reject it.
        let html = MINI_DOC.replace("<td>2</td>", "<td>1</td>");
        let err = parse_document(&html, "u").unwrap_err().to_string();
        assert!(err.contains("descending"), "{err}");
        // The untouched descending document parses.
        assert_eq!(parse_document(MINI_DOC, "u").unwrap().len(), 2);
    }

    #[test]
    fn summary_count_mismatch_hard_rejects() {
        let html = MINI_DOC.replace("(2 transactions total)", "(3 transactions total)");
        let err = parse_document(&html, "u").unwrap_err().to_string();
        assert!(err.contains("summary says 3"), "{err}");
    }

    #[test]
    fn owner_distribution_mismatch_hard_rejects() {
        let html = MINI_DOC
            .replace("<li>1 Self</li>", "<li>2 Self</li>")
            .replace("<li>1 Spouse</li>", "<li>0 Spouse</li>");
        let err = parse_document(&html, "u").unwrap_err().to_string();
        assert!(err.contains("hard reject (§3.7)"), "{err}");
    }

    #[test]
    fn confidence_scores_per_the_regime_doc() {
        let rows = parse_document(MINI_DOC, "some-uuid").unwrap();
        // Row 1 carries a sub-line: 1.00 - 0.02; row 2 is clean: exactly 1.0.
        #[allow(clippy::float_cmp)] // bit-equality IS the contract (MANIFEST)
        {
            assert_eq!(rows[0].confidence, 1.0f32 - 0.02f32);
            assert_eq!(rows[1].confidence, 1.0f32);
        }
        assert_eq!(rows[0].row.report_uuid, "some-uuid");
        assert_eq!(rows[0].row.row_ordinal, 1);
        assert_eq!(rows[0].row.row_number_raw, "2");
        assert_eq!(rows[1].row.ticker_raw, None);
        assert_eq!(rows[1].row.comment_raw, None);
    }

    /// Minimal well-formed electronic PTR: 2 rows, descending # (2, 1),
    /// one Self + one Spouse, row 1 with ticker anchor + sub-line.
    const MINI_DOC: &str = "\
        <h1 class=\"mb-2\">Periodic Transaction Report\n for 06/12/2026\n</h1>\
        <h2 class=\"filedReport\">The Honorable John\nFetterman\n(Fetterman, John)</h2>\
        <p class=\"muted\"><strong class=\"noWrap\"><i class=\"fa fa-folder\"></i> \
        Filed  06/12/2026 @ 1:23 PM</strong></p>\
        <section class=\"card\"><ul>\
        <li>(2 transactions total)</li><li>1 Self</li><li>0 Joint</li>\
        <li>1 Spouse</li><li>0 Dependent Child</li></ul>\
        <table class=\"table table-striped\"><thead><tr class=\"header\"><th>#</th></tr></thead>\
        <tbody>\
        <tr><td>2</td><td>\n05/06/2026\n</td><td>Self</td>\
        <td><a href=\"https://finance.yahoo.com/quote/ORCL\" target=\"_blank\">ORCL</a></td>\
        <td>Oracle Corporation Common Stock\
        <div class=\"text-muted\"><em>Rate/Coupon:</em> 5.5%</div></td>\
        <td>Stock</td><td>Purchase</td><td>$1,001 - $15,000</td><td>ok</td></tr>\
        <tr><td>1</td><td>05/07/2026</td><td>Spouse</td><td> -- </td>\
        <td>EXPEDIA GROUP INC NOTE</td>\
        <td>Corporate Bond</td><td>Sale (Full)</td><td>$15,001 - $50,000</td><td>--</td></tr>\
        </tbody></table></section>";
}
