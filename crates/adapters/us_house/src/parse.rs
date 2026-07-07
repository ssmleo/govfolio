//! Bronze → Silver: content-stream-order state machine over the electronic
//! PTR text layer (regime doc §3.2 grammar, §6 strategy).
//!
//! The small-caps quirk (§3.1): headings/labels lose every non-initial glyph —
//! `pdf-extract` renders the lost glyphs as NUL characters — so labels are
//! anchored on the surviving capitals (`F S:`, `S O:`, `D:`, `C:`, `L:`) after
//! NUL-stripping, never on full label text. Data cells extract verbatim.
//! Rows anchor on the only place two `MM/DD/YYYY` tokens are adjacent.
//! Hard rejects (unknown type token, band outside grammar, unparseable date,
//! `Filing ID #` disagreement) are errors, never low-confidence rows
//! (invariant 6 over confidence).

use anyhow::Context as _;
use serde::{Deserialize, Serialize};

use crate::tables;

/// Extractor id recorded on every Silver row (regime doc §4).
pub(crate) const EXTRACTOR: &str = "us_house_ptr/text@1";

/// One `stg_us_house` payload: source-faithful verbatim strings, regime doc §4
/// field for field (confidence lives on the [`pipeline::adapter::StagingRow`]
/// wrapper, not here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SilverRow {
    pub(crate) doc_id: String,
    pub(crate) row_ordinal: u32,
    pub(crate) filer_name_raw: String,
    pub(crate) filer_status_raw: String,
    pub(crate) state_district_raw: String,
    pub(crate) row_id_raw: Option<String>,
    pub(crate) owner_code_raw: Option<String>,
    pub(crate) asset_raw: String,
    pub(crate) asset_type_code_raw: Option<String>,
    pub(crate) transaction_type_raw: String,
    pub(crate) transaction_date_raw: String,
    pub(crate) notification_date_raw: String,
    pub(crate) amount_raw: String,
    pub(crate) cap_gains_over_200: Option<bool>,
    pub(crate) filing_status_raw: String,
    pub(crate) subholding_of_raw: Option<String>,
    pub(crate) description_raw: Option<String>,
    pub(crate) comments_raw: Option<String>,
    pub(crate) vehicle_owner_code_raw: Option<String>,
    pub(crate) vehicle_location_raw: Option<String>,
    pub(crate) signed_date_raw: String,
    pub(crate) extractor: String,
}

/// A Silver row plus its §6 rule-based confidence score.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScoredRow {
    pub(crate) row: SilverRow,
    pub(crate) confidence: f32,
}

/// Everything the text layer yielded for one document.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedDoc {
    pub(crate) doc_id: String,
    pub(crate) rows: Vec<ScoredRow>,
}

/// Parses one electronic PTR's extracted text into scored Silver rows.
pub(crate) fn parse_document(text: &str) -> anyhow::Result<ParsedDoc> {
    let lines: Vec<String> = text.lines().map(clean_line).collect();
    let doc_id = extract_doc_id(&lines)?;
    let filer_name_raw = labeled_value(&lines, "Name:")?;
    let filer_status_raw = labeled_value(&lines, "Status:")?;
    let state_district_raw = labeled_value(&lines, "State/District:")?;
    let signed_date_raw = extract_signed_date(&lines)?;
    let drafts = scan_rows(transactions_region(&lines)?)?;
    let vehicles = match vehicle_region(&lines) {
        Some(region) => parse_vehicles(region)?,
        None => Vec::new(),
    };

    let mut rows = Vec::with_capacity(drafts.len());
    for (index, draft) in drafts.into_iter().enumerate() {
        let row_ordinal = u32::try_from(index + 1).context("row ordinal overflow")?;
        let filing_status_raw = draft.filing_status_raw.with_context(|| {
            format!("row {row_ordinal} has no FILING STATUS sub-line — grammar break")
        })?;
        // Vehicle join: exact SUBHOLDING OF ↔ bullet-name string match (§6.1).
        let (vehicle, vehicle_unmatched) = match draft.subholding_of_raw.as_deref() {
            Some(name) => match vehicles.iter().find(|v| v.name == name) {
                Some(vehicle) => (Some(vehicle), false),
                None => (None, true),
            },
            None => (None, false),
        };
        let known_asset_code = draft
            .asset_type_code_raw
            .as_deref()
            .is_some_and(|code| tables::asset_class_for_code(code).is_some());

        // §6.2 confidence scoring. cap_gains is tri-state and v1 cannot prove a
        // positive token, so every row pays the null penalty.
        let mut confidence: f32 = 1.0;
        confidence -= 0.02; // cap_gains_over_200 null (v1 constant)
        if !known_asset_code {
            confidence -= 0.05; // unknown/missing asset-type code
        }
        if draft.page_break_join {
            confidence -= 0.05; // asset cell joined across a page break
        }
        if draft.loose_label || vehicle.is_some_and(|v| v.loose_label) {
            confidence -= 0.10; // a sub-line label matched only loosely
        }
        if vehicle_unmatched {
            confidence -= 0.10; // vehicle reference without a matching bullet
        }

        rows.push(ScoredRow {
            row: SilverRow {
                doc_id: doc_id.clone(),
                row_ordinal,
                filer_name_raw: filer_name_raw.clone(),
                filer_status_raw: filer_status_raw.clone(),
                state_district_raw: state_district_raw.clone(),
                row_id_raw: draft.row_id_raw,
                owner_code_raw: draft.owner_code_raw,
                asset_raw: draft.asset_raw,
                asset_type_code_raw: draft.asset_type_code_raw,
                transaction_type_raw: draft.transaction_type_raw,
                transaction_date_raw: draft.transaction_date_raw,
                notification_date_raw: draft.notification_date_raw,
                amount_raw: draft.amount_raw,
                cap_gains_over_200: None, // checkbox state absent from text layer (§3.2)
                filing_status_raw,
                subholding_of_raw: draft.subholding_of_raw,
                description_raw: draft.description_raw,
                comments_raw: draft.comments_raw,
                vehicle_owner_code_raw: vehicle.and_then(|v| v.owner_code.clone()),
                vehicle_location_raw: vehicle.and_then(|v| v.location.clone()),
                signed_date_raw: signed_date_raw.clone(),
                extractor: EXTRACTOR.to_owned(),
            },
            confidence: confidence.clamp(0.0, 1.0),
        });
    }
    Ok(ParsedDoc { doc_id, rows })
}

/// Strips the NUL glyphs the small-caps degradation leaves behind (§3.1) and
/// trims the line. Data cells carry no NULs and pass through verbatim.
fn clean_line(line: &str) -> String {
    let cleaned: String = line.chars().filter(|c| *c != '\u{0}').collect();
    cleaned.trim().to_owned()
}

/// Whitespace-collapsed view, used only for heading/header-block matching.
fn collapse_ws(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `Filing ID #<digits>` — every occurrence (page footers) must agree.
fn extract_doc_id(lines: &[String]) -> anyhow::Result<String> {
    let mut seen: Vec<String> = Vec::new();
    for line in lines {
        if let Some(at) = line.find("Filing ID #") {
            let digits: String = line[at + "Filing ID #".len()..]
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            anyhow::ensure!(!digits.is_empty(), "empty Filing ID on line {line:?}");
            if !seen.contains(&digits) {
                seen.push(digits);
            }
        }
    }
    match seen.as_slice() {
        [one] => {
            anyhow::ensure!(
                (4..=8).contains(&one.len()),
                "Filing ID {one:?} outside the observed 4-8 digit DocID shape"
            );
            Ok(one.clone())
        }
        [] => anyhow::bail!("no `Filing ID #` found — not an electronic PTR text layer"),
        many => anyhow::bail!("conflicting Filing IDs {many:?} — hard reject (doc_id mismatch)"),
    }
}

/// First `<prefix> <value>` line (mixed-case data labels extract intact, §3.1).
/// The label itself matches case-insensitively: pre-2022-ish historical-era
/// text layers render it lowercase (`name:`) where modern ones use `Name:`.
fn labeled_value(lines: &[String], prefix: &str) -> anyhow::Result<String> {
    lines
        .iter()
        .find_map(|line| {
            let value = strip_prefix_ignore_ascii_case(line, prefix)?.trim();
            (!value.is_empty()).then(|| value.to_owned())
        })
        .with_context(|| format!("missing filer-information line {prefix:?}"))
}

/// Case-insensitive `str::strip_prefix`: anchored at the start of `line` only
/// (never matches partway through a longer word), and byte-length-safe (never
/// panics on a `line` shorter than `prefix`).
fn strip_prefix_ignore_ascii_case<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let bytes = line.as_bytes();
    if bytes.len() < prefix.len() {
        return None;
    }
    bytes[..prefix.len()]
        .eq_ignore_ascii_case(prefix.as_bytes())
        .then(|| &line[prefix.len()..])
}

/// Date from `Digitally Signed: <name> , <MM/DD/YYYY>` (stray pre-comma space
/// tolerated — quirks log).
fn extract_signed_date(lines: &[String]) -> anyhow::Result<String> {
    let line = lines
        .iter()
        .find(|line| line.starts_with("Digitally Signed:"))
        .context("missing `Digitally Signed:` line")?;
    let (_, date) = line
        .rsplit_once(',')
        .with_context(|| format!("unsplittable signature line {line:?}"))?;
    let date = date.trim();
    anyhow::ensure!(
        is_date10(date),
        "signature date {date:?} is not MM/DD/YYYY — hard reject"
    );
    Ok(date.to_owned())
}

/// The Transactions table region: after the `T` heading (small-caps survivor
/// of "TRANSACTIONS") up to the `* For the complete list…` footnote.
fn transactions_region(lines: &[String]) -> anyhow::Result<&[String]> {
    let start = lines
        .iter()
        .position(|line| collapse_ws(line) == "T")
        .context("Transactions heading (`T`) not found")?;
    let end = lines[start..]
        .iter()
        .position(|line| line.starts_with("* For the complete list"))
        .map(|offset| start + offset)
        .context("Transactions footnote (`* For the complete list…`) not found")?;
    Ok(&lines[start + 1..end])
}

/// The Investment Vehicle Details region (`I V D` heading), when present:
/// up to the next section heading (`C` comments / `I P O` / `C S`).
fn vehicle_region(lines: &[String]) -> Option<&[String]> {
    let start = lines.iter().position(|line| collapse_ws(line) == "I V D")?;
    let end = lines[start + 1..]
        .iter()
        .position(|line| matches!(collapse_ws(line).as_str(), "C" | "I P O" | "C S"))
        .map_or(lines.len(), |offset| start + 1 + offset);
    Some(&lines[start + 1..end])
}

/// The table-header block as the text layer yields it (repeats on page
/// breaks, §3 anatomy); matched whitespace-collapsed, skipped strictly.
const HEADER_BLOCK: [&str; 5] = [
    "ID Owner Asset Transaction",
    "Type Date Notification",
    "Date Amount Cap.",
    "Gains >",
    "$200?",
];

/// Sub-line labels that survive small-caps degradation (§3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubLabel {
    FilingStatus,
    SubholdingOf,
    Description,
    Comments,
    Location,
}

/// Matches a degraded sub-line label. Strict form is the exact surviving-
/// capital shape (`F S:` …); anything needing whitespace tolerance is a loose
/// match and costs confidence (§6.2).
fn match_sublabel(line: &str) -> Option<(SubLabel, String, bool)> {
    const LABELS: [(SubLabel, &str, &[char]); 5] = [
        (SubLabel::FilingStatus, "F S:", &['F', 'S']),
        (SubLabel::SubholdingOf, "S O:", &['S', 'O']),
        (SubLabel::Description, "D:", &['D']),
        (SubLabel::Comments, "C:", &['C']),
        (SubLabel::Location, "L:", &['L']),
    ];
    for (label, strict, letters) in LABELS {
        if let Some(rest) = line.strip_prefix(strict) {
            let content = rest.trim();
            if !content.is_empty() {
                return Some((label, content.to_owned(), false));
            }
        }
        if let Some(content) = tolerant_label(line, letters)
            && !content.is_empty()
        {
            return Some((label, content, true));
        }
    }
    None
}

/// `^F\s*S\s*:\s*(.+)$`-style tolerant matcher over the surviving capitals.
fn tolerant_label(line: &str, letters: &[char]) -> Option<String> {
    let mut rest = line;
    for (position, letter) in letters.iter().enumerate() {
        if position > 0 {
            rest = rest.trim_start();
        }
        rest = rest.strip_prefix(*letter)?;
    }
    let rest = rest.trim_start().strip_prefix(':')?;
    Some(rest.trim().to_owned())
}

/// One transaction row mid-assembly.
#[derive(Debug, Default)]
struct RowDraft {
    row_id_raw: Option<String>,
    owner_code_raw: Option<String>,
    asset_raw: String,
    asset_type_code_raw: Option<String>,
    transaction_type_raw: String,
    transaction_date_raw: String,
    notification_date_raw: String,
    amount_raw: String,
    filing_status_raw: Option<String>,
    subholding_of_raw: Option<String>,
    description_raw: Option<String>,
    comments_raw: Option<String>,
    loose_label: bool,
    page_break_join: bool,
}

/// The row anchor: `<type_token> <MM/DD/YYYY> <MM/DD/YYYY> <band…>` — the only
/// place two date tokens are adjacent (§6.1).
#[derive(Debug)]
struct Anchor {
    /// Asset-cell tail preceding the type token on the anchor line.
    pre: String,
    type_token: String,
    transaction_date: String,
    notification_date: String,
    amount: String,
}

/// Content-order scan of the Transactions region into row drafts.
#[allow(clippy::too_many_lines)] // one state machine, one place (§3.2 grammar)
fn scan_rows(region: &[String]) -> anyhow::Result<Vec<RowDraft>> {
    let mut drafts: Vec<RowDraft> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut pending_page_break = false;
    let mut i = 0;
    while i < region.len() {
        let line = &region[i];
        if line.is_empty() {
            i += 1;
            continue;
        }
        // Page-boundary artifacts: per-page `Filing ID #` footer (already
        // cross-checked by extract_doc_id) and the repeated header block.
        if line.starts_with("Filing ID #") {
            // Asset cell still open ⇒ it continues across the page break.
            pending_page_break |= !pending.is_empty();
            i += 1;
            continue;
        }
        if collapse_ws(line) == HEADER_BLOCK[0] {
            for (offset, expected) in HEADER_BLOCK.iter().enumerate().skip(1) {
                let got = region.get(i + offset).map(|l| collapse_ws(l));
                anyhow::ensure!(
                    got.as_deref() == Some(*expected),
                    "unrecognized table header block at {line:?} (expected {expected:?}, got {got:?})"
                );
            }
            i += HEADER_BLOCK.len();
            continue;
        }
        if let Some((label, content, loose)) = match_sublabel(line) {
            anyhow::ensure!(
                pending.is_empty(),
                "sub-line {line:?} amid unattached asset text — grammar break"
            );
            let draft = drafts
                .last_mut()
                .with_context(|| format!("sub-line {line:?} before any transaction row"))?;
            let slot = match label {
                SubLabel::FilingStatus => &mut draft.filing_status_raw,
                SubLabel::SubholdingOf => &mut draft.subholding_of_raw,
                SubLabel::Description => &mut draft.description_raw,
                SubLabel::Comments => &mut draft.comments_raw,
                SubLabel::Location => {
                    anyhow::bail!("L: sub-line inside the Transactions region: {line:?}")
                }
            };
            anyhow::ensure!(slot.is_none(), "duplicate sub-line label on {line:?}");
            *slot = Some(content);
            draft.loose_label |= loose;
            i += 1;
            continue;
        }
        if let Some(anchor) = find_anchor(line)? {
            let mut amount = anchor.amount;
            if amount.ends_with('-') {
                // Long bands wrap after the hyphen (§3.2); join with a space.
                let next = region
                    .get(i + 1)
                    .with_context(|| format!("band {amount:?} wraps past the region end"))?;
                anyhow::ensure!(
                    next.starts_with('$'),
                    "band {amount:?} wrap not followed by a `$…` continuation: {next:?}"
                );
                amount.push(' ');
                amount.push_str(next);
                i += 1;
            }
            // Hard rejects (§6.2): band outside grammar, unparseable dates.
            anyhow::ensure!(
                tables::band_bounds(&amount).is_some(),
                "band {amount:?} outside the grammar — hard reject (invariant 6)"
            );
            for date in [&anchor.transaction_date, &anchor.notification_date] {
                chrono::NaiveDate::parse_from_str(date, "%m/%d/%Y")
                    .with_context(|| format!("unparseable date {date:?} — hard reject"))?;
            }
            let mut cell = std::mem::take(&mut pending).join(" ");
            if !anchor.pre.is_empty() {
                if !cell.is_empty() {
                    cell.push(' ');
                }
                cell.push_str(&anchor.pre);
            }
            let (row_id_raw, owner_code_raw, asset_raw) = split_cell(&cell)?;
            let asset_type_code_raw = trailing_asset_code(&asset_raw);
            drafts.push(RowDraft {
                row_id_raw,
                owner_code_raw,
                asset_type_code_raw,
                asset_raw,
                transaction_type_raw: anchor.type_token,
                transaction_date_raw: anchor.transaction_date,
                notification_date_raw: anchor.notification_date,
                amount_raw: amount,
                page_break_join: std::mem::take(&mut pending_page_break),
                ..RowDraft::default()
            });
            i += 1;
            continue;
        }
        pending.push(line.clone());
        i += 1;
    }
    anyhow::ensure!(
        pending.is_empty(),
        "unattached asset text after the last row: {pending:?}"
    );
    Ok(drafts)
}

/// Finds the row anchor on one line. `Err` = the line looks anchor-shaped but
/// violates the grammar (unknown type token, date pair without a band) — a
/// hard reject, never a silently absorbed line.
fn find_anchor(line: &str) -> anyhow::Result<Option<Anchor>> {
    let tokens = tokens_with_spans(line);
    for i in 0..tokens.len() {
        if !is_date10(tokens[i].2) {
            continue;
        }
        let Some(&(_, _, second)) = tokens.get(i + 1) else {
            break;
        };
        if !is_date10(second) {
            continue;
        }
        let Some(&(amount_start, _, _)) = tokens.get(i + 2) else {
            anyhow::bail!("date pair without an amount cell: {line:?}");
        };
        let amount = line[amount_start..].trim().to_owned();
        anyhow::ensure!(
            amount.starts_with('$') || amount.starts_with("Over $"),
            "date pair not followed by an amount band: {line:?}"
        );
        let (type_token, pre_end) = match i.checked_sub(1).map(|j| tokens[j]) {
            Some((start, _, token @ ("P" | "S" | "E"))) => (token.to_owned(), start),
            Some((_, _, "(partial)")) if i >= 2 && tokens[i - 2].2 == "S" => {
                ("S (partial)".to_owned(), tokens[i - 2].0)
            }
            other => anyhow::bail!(
                "unknown transaction type token {:?} — hard reject (§3.4): {line:?}",
                other.map(|(_, _, token)| token)
            ),
        };
        return Ok(Some(Anchor {
            pre: line[..pre_end].trim().to_owned(),
            type_token,
            transaction_date: tokens[i].2.to_owned(),
            notification_date: second.to_owned(),
            amount,
        }));
    }
    Ok(None)
}

/// Whitespace-split tokens with byte spans, so cells can be cut verbatim.
fn tokens_with_spans(line: &str) -> Vec<(usize, usize, &str)> {
    let mut tokens = Vec::new();
    let mut start: Option<usize> = None;
    for (index, ch) in line.char_indices() {
        if ch.is_whitespace() {
            if let Some(s) = start.take() {
                tokens.push((s, index, &line[s..index]));
            }
        } else if start.is_none() {
            start = Some(index);
        }
    }
    if let Some(s) = start {
        tokens.push((s, line.len(), &line[s..]));
    }
    tokens
}

/// Strict `MM/DD/YYYY` (zero-padded, as printed in the table).
fn is_date10(token: &str) -> bool {
    let bytes = token.as_bytes();
    bytes.len() == 10
        && bytes[2] == b'/'
        && bytes[5] == b'/'
        && [0, 1, 3, 4, 6, 7, 8, 9]
            .iter()
            .all(|&at| bytes[at].is_ascii_digit())
}

/// `[row_id]? [owner_code]? <asset…>` — leading cells of the joined row text
/// (§3.2): a 10-digit eFD id (amended rows only), then an exact owner token.
fn split_cell(cell: &str) -> anyhow::Result<(Option<String>, Option<String>, String)> {
    let mut rest = cell.trim();
    let mut row_id = None;
    let mut owner = None;
    if let Some((first, tail)) = rest.split_once(' ')
        && first.len() == 10
        && first.bytes().all(|b| b.is_ascii_digit())
    {
        row_id = Some(first.to_owned());
        rest = tail.trim_start();
    }
    if let Some((first, tail)) = rest.split_once(' ')
        && tables::ROW_OWNER_TOKENS.contains(&first)
    {
        owner = Some(first.to_owned());
        rest = tail.trim_start();
    }
    anyhow::ensure!(!rest.is_empty(), "empty asset cell in row {cell:?}");
    Ok((row_id, owner, rest.to_owned()))
}

/// The trailing `[XX]` asset-type code, when present (stays inside
/// `asset_raw` either way — raw is sacred).
fn trailing_asset_code(asset: &str) -> Option<String> {
    let (_, code) = asset.strip_suffix(']')?.rsplit_once('[')?;
    (code.len() == 2
        && code
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit()))
    .then(|| code.to_owned())
}

/// One Investment Vehicle Details bullet.
#[derive(Debug, Clone, PartialEq)]
struct Vehicle {
    name: String,
    owner_code: Option<String>,
    location: Option<String>,
    loose_label: bool,
}

/// Parses the `I V D` region: blank-line-separated bullets, each a name
/// (optionally suffixed `(Owner: XX)`) plus an optional `L:` sub-line.
fn parse_vehicles(region: &[String]) -> anyhow::Result<Vec<Vehicle>> {
    let mut vehicles = Vec::new();
    let mut name_parts: Vec<String> = Vec::new();
    let mut location: Option<String> = None;
    let mut loose = false;
    for line in region {
        if line.is_empty() || line.starts_with("Filing ID #") {
            flush_vehicle(&mut vehicles, &mut name_parts, &mut location, &mut loose);
            continue;
        }
        match match_sublabel(line) {
            Some((SubLabel::Location, content, was_loose)) => {
                anyhow::ensure!(
                    !name_parts.is_empty() && location.is_none(),
                    "dangling L: sub-line in Investment Vehicle Details: {line:?}"
                );
                location = Some(content);
                loose |= was_loose;
            }
            Some(_) => anyhow::bail!("unexpected sub-line in Investment Vehicle Details: {line:?}"),
            None => {
                anyhow::ensure!(
                    location.is_none(),
                    "vehicle name after its L: sub-line — grammar break: {line:?}"
                );
                name_parts.push(line.clone());
            }
        }
    }
    flush_vehicle(&mut vehicles, &mut name_parts, &mut location, &mut loose);
    Ok(vehicles)
}

/// Closes the current vehicle bullet, splitting a trailing `(Owner: XX)`.
fn flush_vehicle(
    vehicles: &mut Vec<Vehicle>,
    name_parts: &mut Vec<String>,
    location: &mut Option<String>,
    loose: &mut bool,
) {
    if name_parts.is_empty() {
        return;
    }
    let full = std::mem::take(name_parts).join(" ");
    let (name, owner_code) = match full
        .strip_suffix(')')
        .and_then(|s| s.rsplit_once(" (Owner: "))
    {
        Some((name, code)) if code.len() == 2 && code.bytes().all(|b| b.is_ascii_uppercase()) => {
            (name.to_owned(), Some(code.to_owned()))
        }
        _ => (full, None),
    };
    vehicles.push(Vehicle {
        name,
        owner_code,
        location: location.take(),
        loose_label: std::mem::take(loose),
    });
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn strip_prefix_ignore_ascii_case_is_byte_length_safe_and_anchored() {
        // Shorter than the prefix: must not panic, must miss.
        assert_eq!(strip_prefix_ignore_ascii_case("Na", "Name:"), None);
        assert_eq!(strip_prefix_ignore_ascii_case("", "Name:"), None);
        // Case-insensitive on the label, whatever case the value carries.
        assert_eq!(
            strip_prefix_ignore_ascii_case("NAME: Jane Doe", "Name:"),
            Some(" Jane Doe")
        );
        // Anchored at the start only, over the label's full byte span
        // (colon included) — a longer word sharing a leading run of letters
        // must not match partway through it.
        assert_eq!(
            strip_prefix_ignore_ascii_case("Named: Someone", "Name:"),
            None
        );
        assert_eq!(
            strip_prefix_ignore_ascii_case("Status: Member", "Name:"),
            None
        );
    }

    #[test]
    fn labeled_value_matches_real_historical_lowercase_name_label() {
        // Real `pdf_extract::extract_text_from_mem` text-layer lines (after
        // `clean_line`'s NUL/whitespace cleanup — no NULs were present in
        // this document, unlike the small-caps-degraded modern fixtures)
        // from a live 2015 electronic PTR: Filing ID #20002776, Rep. Brad
        // Ashford (NE-02), filed 2015-03-24, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2015/20002776.pdf
        // (pdf sha256 9fa13801b0271971f090ad1e1cc9f6ffd6b3bd002134c56fa4306560ac0297ff).
        // This real document's text layer renders the "Name:"
        // filer-information label lowercase (`name:`) — the exact shape
        // goal 081's live 2012-2026 dry-run sweep found failing near-100%
        // across 2014-2017 with `missing filer-information line "Name:"`
        // under the old exact-case `strip_prefix`. `Status:`/
        // `State/District:` happen to keep modern case in this particular
        // document; all three go through the same shared `labeled_value`.
        let lines: Vec<String> = vec![
            "filer information".to_owned(),
            "name: Brad Ashford".to_owned(),
            "Status: Member".to_owned(),
            "State/District: NE02".to_owned(),
        ];

        assert_eq!(labeled_value(&lines, "Name:").unwrap(), "Brad Ashford");
        assert_eq!(labeled_value(&lines, "Status:").unwrap(), "Member");
        assert_eq!(labeled_value(&lines, "State/District:").unwrap(), "NE02");
    }

    #[test]
    fn nul_stripping_recovers_degraded_labels() {
        assert_eq!(clean_line("F\u{0}\u{0}\u{0} S\u{0}\u{0}: New"), "F S: New");
        assert_eq!(
            clean_line("S\u{0}\u{0}\u{0} O\u{0}: Interactive Brokers LLC"),
            "S O: Interactive Brokers LLC"
        );
        assert_eq!(clean_line("T\u{0}\u{0}\u{0}\u{0}"), "T");
    }

    #[test]
    fn sublabels_match_strict_without_penalty_and_loose_with() {
        assert_eq!(
            match_sublabel("F S: Amended"),
            Some((SubLabel::FilingStatus, "Amended".to_owned(), false))
        );
        assert_eq!(
            match_sublabel("D: Purchased 200 call options."),
            Some((
                SubLabel::Description,
                "Purchased 200 call options.".to_owned(),
                false
            ))
        );
        // Extra whitespace = loose match (confidence penalty, §6.2).
        assert_eq!(
            match_sublabel("F  S : New"),
            Some((SubLabel::FilingStatus, "New".to_owned(), true))
        );
        // Headings carry no colon and data lines no label shape.
        assert_eq!(match_sublabel("C"), None);
        assert_eq!(match_sublabel("Boeing Company (BA) [ST]"), None);
    }

    #[test]
    fn anchor_splits_type_dates_and_band() {
        let anchor = find_anchor("Listen Ventures IV, LP [HN] P 05/13/2026 05/13/2026 $250,001 -")
            .unwrap()
            .unwrap();
        assert_eq!(anchor.pre, "Listen Ventures IV, LP [HN]");
        assert_eq!(anchor.type_token, "P");
        assert_eq!(anchor.transaction_date, "05/13/2026");
        assert_eq!(anchor.notification_date, "05/13/2026");
        assert_eq!(anchor.amount, "$250,001 -");
    }

    #[test]
    fn anchor_rejects_unknown_type_token() {
        let err = find_anchor("Something [ST] X 05/13/2026 05/13/2026 $1,001 - $15,000")
            .unwrap_err()
            .to_string();
        assert!(err.contains("unknown transaction type token"), "{err}");
    }

    #[test]
    fn non_anchor_lines_pass_through() {
        assert!(
            find_anchor("Fulton Financial Corporation -")
                .unwrap()
                .is_none()
        );
        // A 2-digit-year date pair is not the strict MM/DD/YYYY anchor.
        assert!(find_anchor("expiry 3/19/27 3/20/27 $50").unwrap().is_none());
    }

    #[test]
    fn cell_splits_row_id_and_owner_tokens_exactly() {
        assert_eq!(
            split_cell("2000152831 Boeing Company (BA) [ST]").unwrap(),
            (
                Some("2000152831".to_owned()),
                None,
                "Boeing Company (BA) [ST]".to_owned()
            )
        );
        assert_eq!(
            split_cell("SP Intel Corporation - Common Stock (INTC) [OP]").unwrap(),
            (
                None,
                Some("SP".to_owned()),
                "Intel Corporation - Common Stock (INTC) [OP]".to_owned()
            )
        );
        // "SPDR…" must NOT shed a phantom SP owner token.
        assert_eq!(
            split_cell("SPDR S&P 500 ETF (SPY) [EF]").unwrap(),
            (None, None, "SPDR S&P 500 ETF (SPY) [EF]".to_owned())
        );
    }

    #[test]
    fn trailing_code_extracts_and_stays_in_asset() {
        assert_eq!(
            trailing_asset_code("Listen Ventures IV, LP [HN]"),
            Some("HN".to_owned())
        );
        assert_eq!(trailing_asset_code("No code here"), None);
        assert_eq!(
            trailing_asset_code("Odd [X]"),
            None,
            "one-char code refused"
        );
    }

    #[test]
    fn vehicles_split_owner_suffix_and_location() {
        let region = vec![
            "Sale of Spouse Inherited Assets (Owner: SP)".to_owned(),
            "L: US".to_owned(),
            String::new(),
            "Interactive Brokers LLC".to_owned(),
        ];
        let vehicles = parse_vehicles(&region).unwrap();
        assert_eq!(vehicles.len(), 2);
        assert_eq!(vehicles[0].name, "Sale of Spouse Inherited Assets");
        assert_eq!(vehicles[0].owner_code.as_deref(), Some("SP"));
        assert_eq!(vehicles[0].location.as_deref(), Some("US"));
        assert_eq!(vehicles[1].name, "Interactive Brokers LLC");
        assert_eq!(vehicles[1].owner_code, None);
    }

    #[test]
    fn conflicting_filing_ids_hard_reject() {
        let lines = vec![
            " Filing ID #20020055".to_owned(),
            " Filing ID #20020056".to_owned(),
        ];
        let err = extract_doc_id(&lines).unwrap_err().to_string();
        assert!(err.contains("conflicting Filing IDs"), "{err}");
    }

    #[test]
    fn signed_date_tolerates_stray_pre_comma_space() {
        let lines = vec!["Digitally Signed: Hon. Steve Cohen , 06/17/2026".to_owned()];
        assert_eq!(extract_signed_date(&lines).unwrap(), "06/17/2026");
    }
}
