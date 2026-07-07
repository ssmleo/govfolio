//! Bronze → Silver: content-stream-order state machine over the electronic
//! PTR text layer (regime doc §3.2 grammar, §6 strategy).
//!
//! The small-caps quirk (§3.1): headings/labels lose every non-initial glyph —
//! `pdf-extract` renders the lost glyphs as NUL characters — so labels are
//! anchored on the surviving capitals (`F S:`, `S O:`, `D:`, `C:`, `L:`) after
//! NUL-stripping, never on full label text. Data cells extract verbatim.
//!
//! A second, independently real degradation pattern (goal 081 Task 4.8, real
//! 2014-2022 electronic PTR evidence): some historical-era documents instead
//! render the WHOLE heading/label word intact but with scrambled/inconsistent
//! case rather than NUL-erasing it (`tranSactionS`, `iD owner asset
//! transaction`, `FILINg STATUS:`). Heading/header-block/sub-label matching
//! accepts BOTH forms side by side — never a replacement of the NUL-survivor
//! form, which 2015+-era fixtures still depend on.
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
                // goal 081 Task 4.12(d): a row's own directly-attached `L:`
                // sub-line (`draft.location_raw`) takes precedence; falls
                // back to the Investment Vehicle Details bullet join
                // exactly as before when the row carries none.
                vehicle_location_raw: draft
                    .location_raw
                    .or_else(|| vehicle.and_then(|v| v.location.clone())),
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

/// Date from `Digitally Signed: <name> , <M/D/YYYY>` (stray pre-comma space
/// tolerated — quirks log; non-zero-padded month/day tolerated, goal 081
/// Task 4.11 — see [`is_lenient_date`]). The label is matched anywhere in the
/// line, not just as a prefix (real evidence: Filing ID #20020708, 2022, pdf
/// sha256 825a86bbd6895fc3e9d71913185bd1c2cc8a2840ca9809de26386b537cd580cb,
/// renders it glued directly onto the end of a `Filing ID #NNNNN` footer line
/// with no line break — `" Filing ID #20020708Digitally Signed: Hon. Jake
/// Auchincloss , 04/10/2022"` — the same page-footer-glue pattern
/// [`extract_doc_id`] already tolerates via `.find` instead of
/// `.starts_with`).
///
/// When the label is absent everywhere (real evidence: Filing IDs #20001674,
/// #20000708, #20004720, #20016417, #20009092 — 2014-2020, all independently
/// live-fetched and sha256-pinned this session — drop the label text
/// entirely; not NUL-degraded, genuinely gone from the extracted text layer,
/// while the signer name + date survive verbatim as the document's own last
/// non-empty line, e.g. `"Mr. Vern Buchanan , 09/15/2014"`), fall back to
/// scanning from the end of the document for the first line whose
/// comma-split tail is itself a lenient date shape — anchored on date shape,
/// not merely "has a comma", so it cannot mistake the certification
/// paragraph's own prose commas (e.g. `"...true, complete, and correct to
/// the"`) for the signature line.
fn extract_signed_date(lines: &[String]) -> anyhow::Result<String> {
    if let Some(line) = lines.iter().find(|line| line.contains("Digitally Signed:")) {
        let line = strip_signature_area_artifact(line);
        let (_, date) = line
            .rsplit_once(',')
            .with_context(|| format!("unsplittable signature line {line:?}"))?;
        let date = strip_signature_area_artifact(date.trim());
        anyhow::ensure!(
            is_lenient_date(date),
            "signature date {date:?} is not a recognizable M/D/YYYY — hard reject"
        );
        return Ok(date.to_owned());
    }
    for line in lines.iter().rev() {
        let candidate = strip_signature_area_artifact(line);
        let Some((_, date)) = candidate.rsplit_once(',') else {
            continue;
        };
        let date = strip_signature_area_artifact(date.trim());
        if is_lenient_date(date) {
            return Ok(date.to_owned());
        }
    }
    anyhow::bail!("missing `Digitally Signed:` line")
}

/// Structural `M/D/YYYY` shape, tolerant of non-zero-padded month/day (goal
/// 081 Task 4.11 — real evidence spanning 2014-2018: Filing IDs #20000800
/// (`"05/6/2014"`), #20001787 (`"10/3/2014"`), #20004485 (`"02/1/2016"`), and
/// many more, all failing the strict [`is_date10`] shape). A strict superset
/// of `is_date10`: every existing zero-padded `MM/DD/YYYY` signature date
/// still matches. Digit COUNT only, like `is_date10` — calendar-range
/// validity is `normalize::parse_source_date`'s job downstream, which already
/// tolerates non-padded `%m/%d/%Y` via chrono's own lenient numeric parsing.
fn is_lenient_date(token: &str) -> bool {
    let Some((month, rest)) = token.split_once('/') else {
        return false;
    };
    let Some((day, year)) = rest.split_once('/') else {
        return false;
    };
    let digit_run_in = |s: &str, len: std::ops::RangeInclusive<usize>| {
        len.contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit())
    };
    digit_run_in(month, 1..=2) && digit_run_in(day, 1..=2) && digit_run_in(year, 4..=4)
}

/// A checkbox-widget glyph-name-leak token (goal 081 Task 4.11) bleeding into
/// certification-section text — the SAME underlying PDF form-field artifact
/// `BAND_ARTIFACT_TOKENS` (Task 4.10) already tolerates trailing an amount
/// band, evidenced here at a DIFFERENT location: real, independently
/// live-fetched and sha256-pinned electronic PTRs show it leading the
/// certification paragraph's opening line, immediately after the
/// "Certification and Signature" heading and before "I CERTIFY..." — Filing
/// ID #20000708 (2014, sha256
/// bfa02ca731327086bd2fe6d8d61408ebecb57d69652d1341e7adb39a8f19704a): `"gfedcb
/// I CERTIFY that the statements..."`; #20004720 shows the equivalent
/// unnumbered `nmlkj`/`nmlkji` IPO-radio family immediately before it too.
/// Not directly observed attached to the `Digitally Signed:` line/value
/// itself in this session's sample — but the same font-level mechanism, so
/// [`extract_signed_date`] strips it defensively (leading OR trailing,
/// mirroring `strip_band_artifact`'s shape) wherever it appears in the
/// candidate signature text, additively: text without the artifact passes
/// through unchanged.
const SIGNATURE_AREA_ARTIFACT_TOKENS: [&str; 2] = ["gfedc", "gfedcb"];

/// Discards a standalone leading OR trailing `SIGNATURE_AREA_ARTIFACT_TOKENS`
/// token from signature-area text, if present (goal 081 Task 4.11) —
/// additive: text without the artifact passes through byte-for-byte
/// unchanged.
fn strip_signature_area_artifact(text: &str) -> &str {
    let trimmed = text.trim();
    for token in SIGNATURE_AREA_ARTIFACT_TOKENS {
        if let Some(stripped) = trimmed.strip_prefix(token)
            && stripped.starts_with(char::is_whitespace)
        {
            return stripped.trim_start();
        }
        if let Some(stripped) = trimmed.strip_suffix(token)
            && stripped.ends_with(char::is_whitespace)
        {
            return stripped.trim_end();
        }
    }
    text
}

/// The Transactions table region: after the heading — either the `T`
/// small-caps survivor of "TRANSACTIONS", or the scrambled-case full-word
/// form (`tranSactionS`, goal 081 Task 4.8 — real 2014-2022 evidence, e.g.
/// Filing ID #20000077) — up to the `* For the complete list…` footnote, or
/// (goal 081 Task 4.9 — real evidence: the footnote is genuinely absent from
/// some 2014-era filings, e.g. Filing IDs #20000077/#20001787) the next
/// section heading, whichever comes first.
fn transactions_region(lines: &[String]) -> anyhow::Result<&[String]> {
    let start = lines
        .iter()
        .position(|line| is_transactions_heading(line))
        .context("Transactions heading (`T`) not found")?;
    let end = lines[start..]
        .iter()
        .position(|line| {
            line.starts_with("* For the complete list") || is_next_section_heading(line)
        })
        .map(|offset| start + offset)
        .context("Transactions end boundary (footnote or next section heading) not found")?;
    Ok(&lines[start + 1..end])
}

/// Whether `line` is one of the section headings that can immediately follow
/// Transactions in the document anatomy (`docs/regimes/us-house.md` §3: 4.
/// Investment Vehicle Details, 5. Comments, 6. Initial Public Offerings, 7.
/// Certification and Signature) — the same vocabulary `vehicle_region`'s own
/// end boundary already recognizes (`"I V D" | "C" | "I P O" | "C S"`).
/// Matched under either real degradation pattern: the NUL-survivor
/// abbreviation, or the scrambled-case full-word form (goal 081 Task 4.9 —
/// directly observed as `commentS` and `initial Public offeringS` across six
/// live-fetched real 2014 electronic PTRs, Filing IDs #20000077, #20000710,
/// #20000800, #20000998, #20001787, #20001934, none of which contain the
/// `* For the complete list…` footnote at all). `"I V D"`/`"C S"` full-word
/// forms were not directly observed absent the footnote (none of the sampled
/// filings had subholdings) but are included for the same structural reason
/// `vehicle_region` already relies on them: Investment Vehicle Details and
/// Certification and Signature are fixed, always-present sections at those
/// same anatomy positions.
fn is_next_section_heading(line: &str) -> bool {
    const SURVIVOR_FORMS: [&str; 4] = ["I V D", "C", "I P O", "C S"];
    const FULL_FORMS: [&str; 4] = [
        "INVESTMENT VEHICLE DETAILS",
        "COMMENTS",
        "INITIAL PUBLIC OFFERINGS",
        "CERTIFICATION AND SIGNATURE",
    ];
    let collapsed = collapse_ws(line);
    SURVIVOR_FORMS.contains(&collapsed.as_str())
        || FULL_FORMS
            .iter()
            .any(|full| collapsed.eq_ignore_ascii_case(full))
}

/// Whether `line` is the Transactions heading, under either real
/// degradation pattern: the NUL-survivor form (`T`) or the scrambled-case
/// full-word form (goal 081 Task 4.8) — matched case-insensitively against
/// the full undegraded word, whitespace-collapsed like every other
/// heading/header check in this module.
fn is_transactions_heading(line: &str) -> bool {
    let collapsed = collapse_ws(line);
    collapsed == "T" || collapsed.eq_ignore_ascii_case("TRANSACTIONS")
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

/// The 2014-era table-header block's genuinely different, SHORTER shape
/// (goal 081 Task 4.12(e)): real 2014 electronic PTRs (e.g. Filing ID
/// #20000077, sha256
/// ea936ce15201393a2fbfc61c9e9670e016fd5c6b0010aae8b750e34ebc924691 — line
/// 16-18 of its real `pdf_extract::extract_text_from_mem` text: `"iD owner
/// asset transaction"` / `"type Date notification"` / `"Date amount"`) never
/// render a "Cap. Gains > $200?" continuation at all — a column the
/// 2014-era paper form appears to genuinely lack, not a rendering
/// degradation. Also observed exact-case (not scrambled): Filing ID
/// #20002042 (2014) renders `"ID Owner Asset Transaction"` / `"Type Date
/// Notification"` / `"Date Amount"` verbatim. An ADDITIVE alternative
/// checked alongside `HEADER_BLOCK`, never a replacement — the modern
/// 5-line block (any year that has it) keeps matching exactly as before.
const HEADER_BLOCK_SHORT: [&str; 3] = [
    "ID Owner Asset Transaction",
    "Type Date Notification",
    "Date Amount",
];

/// Whether `line` is one of the fixed-vocabulary tokens that can appear as
/// page-boundary furniture inside the Transactions region: a blank line, the
/// per-page `Filing ID #` footer, or either table-header-block shape's own
/// line text (in any position, not just a full in-order block — a cheap,
/// safe over-approximation, since none of these fixed strings could ever be
/// a real amount-band continuation, which always starts with `$`). Goal 081
/// Task 4.12(c) uses this to look PAST such furniture when a wrapped band's
/// hyphen and its `$…` continuation land on opposite sides of a page break.
fn is_page_boundary_furniture(line: &str) -> bool {
    if line.is_empty() || line.starts_with("Filing ID #") {
        return true;
    }
    let collapsed = collapse_ws(line);
    HEADER_BLOCK
        .iter()
        .chain(HEADER_BLOCK_SHORT.iter())
        .any(|expected| collapsed.eq_ignore_ascii_case(expected))
}

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
/// match and costs confidence (§6.2). A third form (goal 081 Task 4.8) is
/// also loose: the FULL, undegraded label text in scrambled case (e.g.
/// `FILINg STATUS:`) instead of NUL-erased to its surviving capitals —
/// directly confirmed by real 2014-2022 evidence for `FilingStatus`,
/// `SubholdingOf` and `Description` (Filing IDs #20000077, #20001787,
/// #20016985, #20020448); extended to `Comments`/`Location` by the same
/// general font-level mechanism, using the full label text already documented
/// in `docs/regimes/us-house.md` for those abbreviations.
fn match_sublabel(line: &str) -> Option<(SubLabel, String, bool)> {
    const LABELS: [(SubLabel, &str, &[char], &str); 5] = [
        (SubLabel::FilingStatus, "F S:", &['F', 'S'], "FILING STATUS"),
        (SubLabel::SubholdingOf, "S O:", &['S', 'O'], "SUBHOLDING OF"),
        (SubLabel::Description, "D:", &['D'], "DESCRIPTION"),
        (SubLabel::Comments, "C:", &['C'], "COMMENTS"),
        (SubLabel::Location, "L:", &['L'], "LOCATION"),
    ];
    for (label, strict, letters, full) in LABELS {
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
        if let Some(content) = full_text_label(line, full)
            && !content.is_empty()
        {
            return Some((label, content, true));
        }
    }
    None
}

/// Case-insensitive, whitespace-tolerant match against the FULL, undegraded
/// sub-label text (e.g. `"FILING STATUS"`, no trailing colon) — the
/// scrambled-case degradation pattern (goal 081 Task 4.8) applied to
/// sub-line labels. Anchored on the label portion before the line's first
/// `:` so it never matches into the value.
fn full_text_label(line: &str, full: &str) -> Option<String> {
    let (label_part, content) = line.split_once(':')?;
    collapse_ws(label_part)
        .eq_ignore_ascii_case(full)
        .then(|| content.trim().to_owned())
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
    /// A `L:` sub-line attached directly to this row (goal 081 Task 4.12(d))
    /// rather than to an Investment Vehicle Details bullet — feeds
    /// `vehicle_location_raw` in [`parse_document`] alongside (falling back
    /// to) the vehicle-bullet join.
    location_raw: Option<String>,
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

/// Trailing PDF checkbox-widget artifact tokens (goal 081 Task 4.10): real
/// 2018-2022 electronic PTR evidence (independently live-fetched and
/// sha256-pinned this session — Filing IDs #20016985 (2020, sha256
/// ce68b1f8b7def98256506531edd2c98557a0844e481ce0126a4cfec510202d6a),
/// #20009788 (2018, sha256
/// 38bb4d144e279c9ff999e6330e7ab90f2b5af86c6a705167da87fdd891e1755e),
/// #20016326 (2020, sha256
/// 50218765b6aed95559b71d556e36e2e59b772c6195f39a443716c3cc57a4ef25),
/// #20019793 (2021, sha256
/// 90663eab7fd7922e6d9533db8e220ca7f5f288047d76d7624650676f120575f2),
/// #20020251 (2022, sha256
/// 94542d4fec1917c208a02da0a5dd40b8a38414e4e5940defc29be68d86c98040)) shows a
/// standalone, whitespace-separated token trailing the amount band whenever
/// the "Cap. Gains > $200?" checkbox column is present — a PDF form-field
/// glyph-name leak (the same font-level mechanism that renders `nmlkj` /
/// `nmlkji` for the IPO Yes/No radio and a leading `gfedcb` before the
/// certification paragraph elsewhere in these same documents, per Task 4.9's
/// note — unrelated to case degradation). Always exactly one of these two
/// literal, case-sensitive forms; never a partial/embedded match.
const BAND_ARTIFACT_TOKENS: [&str; 2] = ["gfedc", "gfedcb"];

/// Discards a trailing `BAND_ARTIFACT_TOKENS` token from an amount band
/// string, if present as a standalone trailing token (goal 081 Task 4.10) —
/// additive: a band with no artifact passes through unchanged, byte-for-byte.
fn strip_band_artifact(amount: &str) -> String {
    for token in BAND_ARTIFACT_TOKENS {
        if let Some(stripped) = amount.strip_suffix(token)
            && stripped.ends_with(char::is_whitespace)
        {
            return stripped.trim_end().to_owned();
        }
    }
    amount.to_owned()
}

/// Content-order scan of the Transactions region into row drafts.
#[allow(clippy::too_many_lines)] // one state machine, one place (§3.2 grammar)
fn scan_rows(region: &[String]) -> anyhow::Result<Vec<RowDraft>> {
    let mut drafts: Vec<RowDraft> = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    let mut pending_page_break = false;
    // goal 081 Task 4.12(b): which sub-line (if any) a plain orphan line
    // should be joined onto as a continuation of that sub-line's own
    // free-text VALUE, when that value itself wraps across further physical
    // lines with no repeated label. Only ever `Some` immediately after a
    // `match_sublabel` hit, with nothing but further orphan lines in
    // between — cleared on a blank line, a page-boundary footer, a
    // header-block reprint, or the next anchor, so it can never fire on a
    // document's ordinary single-line sub-line values (real evidence:
    // Filing IDs #20021740 (2022), #20022126 (2023), #20034044/#20034201
    // (2026)).
    let mut mid_sublabel: Option<SubLabel> = None;
    let mut i = 0;
    while i < region.len() {
        let line = &region[i];
        if line.is_empty() {
            mid_sublabel = None;
            i += 1;
            continue;
        }
        // Page-boundary artifacts: per-page `Filing ID #` footer (already
        // cross-checked by extract_doc_id) and the repeated header block.
        if line.starts_with("Filing ID #") {
            // Asset cell still open ⇒ it continues across the page break.
            pending_page_break |= !pending.is_empty();
            mid_sublabel = None;
            i += 1;
            continue;
        }
        // Case-insensitive: some historical-era (2014-2022) documents render
        // this whole block in scrambled case (`iD owner asset transaction`
        // — goal 081 Task 4.8, real evidence e.g. Filing ID #20016985)
        // instead of the modern exact-case rendering; both forms are
        // accepted the same way, whitespace-collapsed as before.
        if collapse_ws(line).eq_ignore_ascii_case(HEADER_BLOCK[0]) {
            mid_sublabel = None;
            let got1 = region.get(i + 1).map(|l| collapse_ws(l));
            anyhow::ensure!(
                got1.as_deref()
                    .is_some_and(|g| g.eq_ignore_ascii_case(HEADER_BLOCK[1])),
                "unrecognized table header block at {line:?} (expected {:?}, got {got1:?})",
                HEADER_BLOCK[1]
            );
            let got2 = region.get(i + 2).map(|l| collapse_ws(l));
            if got2
                .as_deref()
                .is_some_and(|g| g.eq_ignore_ascii_case(HEADER_BLOCK_SHORT[2]))
            {
                // goal 081 Task 4.12(e): the 2014-era 3-line shape — no
                // "Cap. Gains > $200?" continuation at all.
                i += HEADER_BLOCK_SHORT.len();
                continue;
            }
            for (offset, expected) in HEADER_BLOCK.iter().enumerate().skip(2) {
                let got = region.get(i + offset).map(|l| collapse_ws(l));
                anyhow::ensure!(
                    got.as_deref()
                        .is_some_and(|g| g.eq_ignore_ascii_case(expected)),
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
                // goal 081 Task 4.12(d): a `L:` sub-line can also appear
                // directly inside the Transactions region, attached to the
                // row itself — not only inside an Investment Vehicle
                // Details bullet, the only place it was previously
                // recognized. Real evidence: Filing IDs #20020708 (2022),
                // #20022368/#20022428/#20024042 (2023), #20016088 (2020, the
                // scrambled-case full-text form `"LoCaTIoN: ..."`),
                // #20034201/#20033744 and many more (2026). Fed into
                // `location_raw`, joined onto the existing
                // `vehicle_location_raw` Gold field in `parse_document`
                // (falling back to the vehicle-bullet join when this row
                // carries none) — no schema change, that field already
                // means "the location tied to this row's holding/vehicle".
                SubLabel::Location => &mut draft.location_raw,
            };
            anyhow::ensure!(slot.is_none(), "duplicate sub-line label on {line:?}");
            *slot = Some(content);
            draft.loose_label |= loose;
            mid_sublabel = Some(label);
            i += 1;
            continue;
        }
        if let Some(anchor) = find_anchor(line)? {
            mid_sublabel = None;
            let mut amount = anchor.amount;
            if amount.ends_with('-') {
                // Long bands wrap after the hyphen (§3.2). goal 081 Task
                // 4.12(c): a page break can fall between the hyphen and its
                // `$…` continuation, landing blank lines / the `Filing ID #`
                // footer / a repeated header-block reprint in between — real
                // evidence: Filing IDs #20023082 (2023, blank + header-block
                // reprint), #20023623 (2023, the footer directly, then a
                // header-block reprint, no blank at all before the footer).
                // Skip any such furniture additively; a continuation on the
                // very next line (the common case) is unaffected.
                let mut next_index = i + 1;
                while region
                    .get(next_index)
                    .is_some_and(|l| is_page_boundary_furniture(l))
                {
                    next_index += 1;
                }
                let next = region
                    .get(next_index)
                    .with_context(|| format!("band {amount:?} wraps past the region end"))?;
                anyhow::ensure!(
                    next.starts_with('$'),
                    "band {amount:?} wrap not followed by a `$…` continuation: {next:?}"
                );
                amount.push(' ');
                amount.push_str(next);
                i = next_index;
            }
            // goal 081 Task 4.10: discard a trailing PDF checkbox-widget
            // artifact before grammar-checking (never before — the artifact
            // always trails the FINAL closing amount, wrapped or not, per
            // real evidence).
            amount = strip_band_artifact(&amount);
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
        if let Some(label) = mid_sublabel {
            // goal 081 Task 4.12(b): a sub-line's own free-text VALUE can
            // itself wrap onto one or more further physical lines with no
            // repeated label — real evidence: Filing IDs #20021740 (2022,
            // Comments), #20022126 (2023, Comments), #20034044 (2026,
            // Comments), #20034201 (2026, Description). Join with a space,
            // additively: `mid_sublabel` is only ever `Some` right after a
            // fresh match with nothing but orphan lines since, so a
            // document's pre-existing single-line sub-line values (the
            // overwhelming common case — always followed by a blank line in
            // every real document sampled) are completely unaffected.
            let draft = drafts
                .last_mut()
                .context("sub-line continuation before any transaction row")?;
            let slot = match label {
                SubLabel::FilingStatus => &mut draft.filing_status_raw,
                SubLabel::SubholdingOf => &mut draft.subholding_of_raw,
                SubLabel::Description => &mut draft.description_raw,
                SubLabel::Comments => &mut draft.comments_raw,
                SubLabel::Location => &mut draft.location_raw,
            };
            if let Some(existing) = slot {
                existing.push(' ');
                existing.push_str(line);
            }
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
        // goal 081 Task 4.12(a): the same scrambled-case degradation Task
        // 4.8 documented for headings/labels also hits the row-level type
        // token, arbitrarily per document (real evidence: Filing IDs
        // #20016288 (2020, `"s"` alone), #20009743/#20010366 (2018, `"s
        // (partial)"`), #20000077 (2014, `"s"` alone)) — matched
        // case-insensitively and normalized to the canonical uppercase form
        // `normalize::normalize_row` expects, a strict superset: an already
        // exact-case token round-trips unchanged.
        let (type_token, pre_end) = match i.checked_sub(1).map(|j| tokens[j]) {
            Some((start, _, token))
                if token.eq_ignore_ascii_case("P")
                    || token.eq_ignore_ascii_case("S")
                    || token.eq_ignore_ascii_case("E") =>
            {
                (token.to_ascii_uppercase(), start)
            }
            Some((_, _, token))
                if token.eq_ignore_ascii_case("(partial)")
                    && i >= 2
                    && tokens[i - 2].2.eq_ignore_ascii_case("S") =>
            {
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
    fn transactions_region_accepts_real_scrambled_case_heading() {
        // Goal 081 Task 4.8: a SECOND real degradation pattern, distinct from
        // the NUL-survivor form above — the whole "TRANSACTIONS" word survives
        // intact but with scrambled/inconsistent case (`tranSactionS`) instead
        // of being NUL-erased to `T`. Real `pdf_extract::extract_text_from_mem`
        // text-layer line from a live 2014 electronic PTR: Filing ID #20000077,
        // fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/20000077.pdf
        // (pdf sha256 ea936ce15201393a2fbfc61c9e9670e016fd5c6b0010aae8b750e34ebc924691).
        // This exact shape previously failed `transactions_region` with
        // "Transactions heading (`T`) not found" (goal 081 Task 4.6's live
        // sweep finding) — it must now succeed, while the pre-existing
        // NUL-survivor form (2015+-era fixtures) keeps working unchanged.
        let lines: Vec<String> = vec![
            "tranSactionS".to_owned(),
            "some row content".to_owned(),
            "* For the complete list of asset type abbreviations".to_owned(),
        ];
        let region = transactions_region(&lines).unwrap();
        assert_eq!(region.len(), 1);
        assert_eq!(region[0], "some row content");

        // The existing NUL-survivor form must still match unchanged.
        let nul_survivor_lines: Vec<String> = vec![
            "T".to_owned(),
            "row".to_owned(),
            "* For the complete list".to_owned(),
        ];
        assert!(transactions_region(&nul_survivor_lines).is_ok());
    }

    #[test]
    fn transactions_region_accepts_a_genuinely_absent_footnote() {
        // Goal 081 Task 4.9: real finding, distinct from Task 4.8's
        // scrambled-case heading fix above — some 2014-era filings never
        // render the `* For the complete list…` footnote AT ALL (not
        // scrambled, genuinely absent). Confirmed directly against SIX
        // independently live-fetched real 2014 electronic PTRs, none of
        // which contain any case-variant of "complete list" anywhere in
        // their extracted text: Filing IDs #20000077 (sha256
        // ea936ce15201393a2fbfc61c9e9670e016fd5c6b0010aae8b750e34ebc924691),
        // #20000710 (sha256
        // 80a4bc944f3e59d85c59d59647e292144b37ca2985789beb5b063739a48b0963),
        // #20000800 (sha256
        // 40babda90c0d13a76da969956206164657a5d7004c8e49809fdfecf8f024ac9c),
        // #20000998 (sha256
        // 49ff83fd5abb33ffc234cf748065c3bb64c053926f6a85da60e3c92fa8554c62),
        // #20001787 (sha256
        // 29bfb95acf4679614ded1fb085743c9eb4220bb9964169b850307f584b06d11c),
        // #20001934 (sha256
        // 035ddd992057a2e608b3a0720eff31ee9b0a2fd6d7e813172150502fca9f9dfb),
        // all fetched from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/<id>.pdf.
        // In 5 of the 6, the real next line after the row content is the
        // "Comments" section heading (rendered `commentS`, scrambled-case);
        // in the 6th (#20000077) that section renders no text at all and the
        // real next line is "Initial Public Offerings" (`initial Public
        // offeringS`) directly. Both real shapes below are verbatim from
        // that live sample.
        let ends_via_comments_heading: Vec<String> = vec![
            "tranSactionS".to_owned(),
            "some row content".to_owned(),
            "commentS".to_owned(),
            "initial Public offeringS".to_owned(),
        ];
        let region = transactions_region(&ends_via_comments_heading).unwrap();
        assert_eq!(region, ["some row content".to_owned()]);

        let ends_via_ipo_heading_directly: Vec<String> = vec![
            "tranSactionS".to_owned(),
            "some row content".to_owned(),
            "initial Public offeringS".to_owned(),
        ];
        let region = transactions_region(&ends_via_ipo_heading_directly).unwrap();
        assert_eq!(region, ["some row content".to_owned()]);

        // The NUL-survivor forms of the same section headings must be
        // recognized too (goal 081 Task 4.8's own dual-form precedent).
        let nul_survivor_ipo: Vec<String> =
            vec!["T".to_owned(), "row".to_owned(), "I P O".to_owned()];
        assert!(transactions_region(&nul_survivor_ipo).is_ok());
    }

    #[test]
    fn transactions_region_prefers_the_footnote_when_both_are_present() {
        // Regression guard: existing footnote-present fixtures (2015+) must
        // keep stopping at the footnote, never scanning past it to a later
        // section heading, once this task's fallback marker is added.
        let lines: Vec<String> = vec![
            "T".to_owned(),
            "row one".to_owned(),
            "* For the complete list of asset type abbreviations".to_owned(),
            "I V D".to_owned(),
            "some vehicle".to_owned(),
            "C".to_owned(),
        ];
        let region = transactions_region(&lines).unwrap();
        assert_eq!(region, ["row one".to_owned()]);
    }

    #[test]
    fn scan_rows_accepts_real_scrambled_case_header_block() {
        // Goal 081 Task 4.8: the table-header block rendered in the SAME
        // scrambled-case pattern as the heading, instead of the modern
        // exact-case rendering `HEADER_BLOCK` matches today. Real
        // `pdf_extract::extract_text_from_mem` lines from a live 2020
        // electronic PTR: Filing ID #20016985, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2020/20016985.pdf
        // (pdf sha256 ce68b1f8b7def98256506531edd2c98557a0844e481ce0126a4cfec510202d6a).
        // The row line itself is clean synthetic grammar (matching this
        // suite's existing `anchor_splits_type_dates_and_band` convention) —
        // the real document's own row hits a separate, out-of-scope artifact
        // (a trailing `gfedc` checkbox-widget token after the amount band),
        // not something this task fixes.
        let region: Vec<String> = vec![
            "iD owner asset transaction".to_owned(),
            "type Date notification".to_owned(),
            "Date amount cap.".to_owned(),
            "gains >".to_owned(),
            "$200?".to_owned(),
            String::new(),
            "Example Corp (EX) [ST] P 06/15/2020 06/20/2020 $1,001 - $15,000".to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].transaction_type_raw, "P");
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
    fn match_sublabel_accepts_real_scrambled_case_full_text_form() {
        // Goal 081 Task 4.8: the SAME scrambled-case degradation pattern
        // confirmed on the Transactions heading/header block also affects
        // sub-line labels — but as the FULL, undegraded label word in
        // scrambled case (`FILINg STATUS:`), not the abbreviated
        // NUL-survivor form (`F S:`) `match_sublabel` already recognized.
        // Real `pdf_extract::extract_text_from_mem` lines, all independently
        // live-fetched this session:
        //   - "FIlINg sTATus: New" — Filing ID #20000077 (2014),
        //     https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/20000077.pdf
        //     (sha256 ea936ce15201393a2fbfc61c9e9670e016fd5c6b0010aae8b750e34ebc924691)
        //   - "DESCRIPTIoN: Sale of 230 shares of Apple, Inc. (AAPL) at
        //     $99.6401/share. Total proceeds of $22,916.71." — Filing ID
        //     #20001787 (2014),
        //     https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/20001787.pdf
        //     (sha256 29bfb95acf4679614ded1fb085743c9eb4220bb9964169b850307f584b06d11c)
        //   - "SubHOlDINg OF: Charles Schwab IRA" — Filing ID #20020448
        //     (2022),
        //     https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2022/20020448.pdf
        //     (sha256 8f7c44affce207b7cc84cc2c74fb514eb37a33d118377f9c974e8710075f27fa)
        // `Comments`/`Location` were not directly observed scrambled in this
        // session's sample, but share the same font-level mechanism and full
        // label text already documented in `docs/regimes/us-house.md`.
        assert_eq!(
            match_sublabel("FIlINg sTATus: New"),
            Some((SubLabel::FilingStatus, "New".to_owned(), true))
        );
        assert_eq!(
            match_sublabel(
                "DESCRIPTIoN: Sale of 230 shares of Apple, Inc. (AAPL) at $99.6401/share. \
                 Total proceeds of $22,916.71."
            ),
            Some((
                SubLabel::Description,
                "Sale of 230 shares of Apple, Inc. (AAPL) at $99.6401/share. Total proceeds \
                 of $22,916.71."
                    .to_owned(),
                true
            ))
        );
        assert_eq!(
            match_sublabel("SubHOlDINg OF: Charles Schwab IRA"),
            Some((
                SubLabel::SubholdingOf,
                "Charles Schwab IRA".to_owned(),
                true
            ))
        );
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

    #[test]
    fn is_lenient_date_tolerates_non_zero_padded_month_and_day() {
        // Goal 081 Task 4.11: a strict superset of `is_date10` — every
        // existing zero-padded shape still matches, plus 1-digit month/day.
        assert!(is_lenient_date("06/17/2026"));
        assert!(is_lenient_date("02/1/2016"));
        assert!(is_lenient_date("5/6/2014"));
        assert!(!is_lenient_date("2016/02/1"));
        assert!(!is_lenient_date("02/1/16"));
        assert!(!is_lenient_date("02//2016"));
        assert!(!is_lenient_date("not a date"));
    }

    #[test]
    fn extract_signed_date_accepts_real_non_zero_padded_date_evidence() {
        // Goal 081 Task 4.11 sub-issue (a): real `pdf_extract::
        // extract_text_from_mem` line verbatim from a live 2016 electronic
        // PTR, Filing ID #20004485, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2016/20004485.pdf
        // (pdf sha256 58e99632e80ebe5418c206bdfa970056f6e3ff7f11217e71bc02208d7cd7dbf5)
        // — previously hard-rejected with `signature date "02/1/2016" is not
        // MM/DD/YYYY`. `signed_date_raw` stays verbatim (raw is sacred) —
        // `normalize::parse_source_date` already tolerates the non-padded
        // form downstream via chrono's own lenient `%m/%d/%Y` parsing.
        let lines = vec!["Digitally Signed: Hon. Brad Ashford , 02/1/2016".to_owned()];
        assert_eq!(extract_signed_date(&lines).unwrap(), "02/1/2016");
    }

    #[test]
    fn extract_signed_date_accepts_real_filing_id_glued_line_evidence() {
        // Goal 081 Task 4.11 sub-issue (b): real line verbatim from a live
        // 2022 electronic PTR, Filing ID #20020708, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2022/20020708.pdf
        // (pdf sha256 825a86bbd6895fc3e9d71913185bd1c2cc8a2840ca9809de26386b537cd580cb)
        // — the `Digitally Signed:` label is glued directly onto the end of
        // a `Filing ID #NNNNN` footer line with no line break, so the old
        // `starts_with` prefix match missed it entirely (`missing
        // \`Digitally Signed:\` line`), the same page-footer-glue pattern
        // `extract_doc_id` already tolerates via `.find`.
        let lines = vec![
            " Filing ID #20020708Digitally Signed: Hon. Jake Auchincloss , 04/10/2022".to_owned(),
        ];
        assert_eq!(extract_signed_date(&lines).unwrap(), "04/10/2022");
    }

    #[test]
    fn extract_signed_date_falls_back_to_the_last_line_when_the_label_is_genuinely_absent() {
        // Goal 081 Task 4.11 sub-issue (b): real lines verbatim from a live
        // 2014 electronic PTR, Filing ID #20001674, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/20001674.pdf
        // (pdf sha256 65803f4906c94339619c94cc75550d610084d5898e458f53187c37d7c8352b6a)
        // — the `Digitally Signed:` label text is genuinely absent (not
        // NUL-degraded — checked directly: no case-variant of "digitally" or
        // "signed" appears anywhere in the real extracted text), while the
        // signer name + date survive verbatim as the document's own last
        // non-empty line. The certification paragraph's own prose commas
        // (real evidence too) must NOT be mistaken for the signature line.
        let lines = vec![
            "certification anD Signature".to_owned(),
            " Filing ID #20001674".to_owned(),
            "/  I CERTIFY that the statements I have made on the attached Periodic \
             Transaction Report are true, complete, and correct to the"
                .to_owned(),
            "best of my knowledge and belief.".to_owned(),
            String::new(),
            "Mr. Vern Buchanan , 09/15/2014".to_owned(),
        ];
        assert_eq!(extract_signed_date(&lines).unwrap(), "09/15/2014");
    }

    #[test]
    fn strip_signature_area_artifact_discards_a_leading_or_trailing_token_only() {
        // Goal 081 Task 4.11 sub-issue (c): mirrors `strip_band_artifact`'s
        // own (Task 4.10) unit-test shape, applied to leading position too.
        assert_eq!(
            strip_signature_area_artifact("gfedcb Hon. Jane Filer , 06/17/2026"),
            "Hon. Jane Filer , 06/17/2026"
        );
        assert_eq!(
            strip_signature_area_artifact("Hon. Jane Filer , 06/17/2026 gfedc"),
            "Hon. Jane Filer , 06/17/2026"
        );
        assert_eq!(
            strip_signature_area_artifact("Hon. Jane Filer , 06/17/2026"),
            "Hon. Jane Filer , 06/17/2026"
        );
        // Not a standalone token (no preceding/following whitespace — an
        // embedded/partial match) — must not strip real data.
        assert_eq!(
            strip_signature_area_artifact("xgfedcHon. Jane Filer , 06/17/2026"),
            "xgfedcHon. Jane Filer , 06/17/2026"
        );
    }

    #[test]
    fn extract_signed_date_fallback_tolerates_the_real_gfedcb_certification_paragraph_artifact() {
        // Goal 081 Task 4.11 sub-issue (c): the `gfedcb` checkbox-widget
        // artifact really does bleed into certification-section text — real
        // lines verbatim from a live 2014 electronic PTR, Filing ID
        // #20000708, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/20000708.pdf
        // (pdf sha256 bfa02ca731327086bd2fe6d8d61408ebecb57d69652d1341e7adb39a8f19704a)
        // — leading the certification paragraph's opening line, immediately
        // after the "Certification and Signature" heading. This document
        // also has NO `Digitally Signed:` label text anywhere (sub-issue
        // (b)'s fallback fires), proving the fallback scan correctly skips
        // the artifact-prefixed prose (no date-shaped comma tail) and lands
        // on the real signature line.
        let lines = vec![
            "certification anD Signature".to_owned(),
            String::new(),
            "gfedcb  I CERTIFY that the statements I have made on the attached Periodic \
             Transaction Report are true, complete, and correct to the"
                .to_owned(),
            String::new(),
            "Filing ID #20000708".to_owned(),
            String::new(),
            "/  I CERTIFY that the statements I have made on the attached Periodic \
             Transaction Report are true, complete, and correct to the"
                .to_owned(),
            "best of my knowledge and belief.".to_owned(),
            String::new(),
            "Mr. Michael C. Burgess , 04/14/2014".to_owned(),
        ];
        assert_eq!(extract_signed_date(&lines).unwrap(), "04/14/2014");
    }

    #[test]
    fn parse_document_succeeds_against_real_2014_2022_scrambled_case_evidence() {
        // Goal 081 Task 4.8 end-to-end proof: `parse_document` now succeeds
        // on a document exhibiting the scrambled-case degradation pattern
        // that previously failed closed with "Transactions heading (`T`)
        // not found". The heading, header block, and FILING STATUS sub-line
        // below are REAL `pdf_extract::extract_text_from_mem` text verbatim
        // from a live 2020 electronic PTR: Filing ID #20016985, fetched
        // directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2020/20016985.pdf
        // (pdf sha256 ce68b1f8b7def98256506531edd2c98557a0844e481ce0126a4cfec510202d6a).
        // Everything else (doc id, filer name, dates, the one row) is clean
        // synthetic grammar, NOT a quote from that real filer — this
        // document's own actual row hits a separate, out-of-scope artifact
        // (a trailing `gfedc` checkbox-widget token after the amount band)
        // this task does not fix; splicing in real evidence only for the
        // elements this fix targets keeps the test isolated to what it
        // proves, matching this suite's existing convention (synthetic rows
        // in `anchor_splits_type_dates_and_band` et al.).
        let text = "\
Filing ID #20099999

name: Jane Filer
Status: Member
State/District: XX00

tranSactionS

iD owner asset transaction
type Date notification
Date amount cap.
gains >
$200?

Example Corp (EX) [ST] P 06/15/2020 06/20/2020 $1,001 - $15,000

FIlINg STATuS: New

* For the complete list of asset type abbreviations, please visit https://fd.house.gov/reference/asset-type-codes.aspx.

Digitally Signed: Jane Filer , 07/01/2020
";
        let doc = parse_document(text).unwrap();
        assert_eq!(doc.doc_id, "20099999");
        assert_eq!(doc.rows.len(), 1);
        assert_eq!(doc.rows[0].row.filing_status_raw, "New");
        assert_eq!(doc.rows[0].row.transaction_type_raw, "P");
        assert!(doc.rows[0].row.asset_raw.contains("Example Corp"));
    }

    #[test]
    fn parse_document_succeeds_against_real_2014_evidence_lacking_the_footnote() {
        // Goal 081 Task 4.9 end-to-end proof: `parse_document` now succeeds
        // on a document with NO Transactions footnote at all, which
        // previously failed closed with "Transactions footnote (`* For the
        // complete list…`) not found". The Transactions heading and the
        // closing "Comments"/"Initial Public Offerings" section headings
        // below are REAL `pdf_extract::extract_text_from_mem` text verbatim
        // from a live 2014 electronic PTR: Filing ID #20001787, fetched
        // directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/20001787.pdf
        // (pdf sha256 29bfb95acf4679614ded1fb085743c9eb4220bb9964169b850307f584b06d11c)
        // — that real document's own text contains no case-variant of
        // "complete list" anywhere. The header block, row, and sub-line are
        // the same clean/real-evidence forms Task 4.8's own end-to-end test
        // (immediately above) already proved — this test changes ONLY the
        // ending, isolating the footnote-absence fix; the real 2014 header
        // block is a separate, shorter shape (no Cap. Gains columns) that is
        // its own out-of-scope gap, not conflated with this fix. The real
        // row itself also hits the separate, out-of-scope non-zero-padded-
        // date gap (`10/1/2014`), so this test's row stays clean synthetic
        // grammar too, matching Task 4.8's own convention.
        let text = "\
Filing ID #20099998

name: Jane Filer
Status: Member
State/District: XX00

tranSactionS

iD owner asset transaction
type Date notification
Date amount cap.
gains >
$200?

Example Corp (EX) [ST] P 06/15/2014 06/20/2014 $1,001 - $15,000

FILINg sTATus: New

commentS

initial Public offeringS

nmlkj  Yes  nmlkji  No

certification anD Signature

Digitally Signed: Jane Filer , 07/01/2014
";
        let doc = parse_document(text).unwrap();
        assert_eq!(doc.doc_id, "20099998");
        assert_eq!(doc.rows.len(), 1);
        assert_eq!(doc.rows[0].row.filing_status_raw, "New");
        assert_eq!(doc.rows[0].row.transaction_type_raw, "P");
        assert!(doc.rows[0].row.asset_raw.contains("Example Corp"));
    }

    #[test]
    fn strip_band_artifact_discards_known_trailing_tokens_only() {
        // Goal 081 Task 4.10: both real forms observed in the live
        // 2018-2022 sample strip cleanly.
        assert_eq!(
            strip_band_artifact("$1,001 - $15,000 gfedc"),
            "$1,001 - $15,000"
        );
        assert_eq!(
            strip_band_artifact("$15,001 - $50,000 gfedcb"),
            "$15,001 - $50,000"
        );
        // ...an artifact-free band passes through unchanged, byte-for-byte.
        assert_eq!(strip_band_artifact("$1,001 - $15,000"), "$1,001 - $15,000");
        // Not a standalone trailing token (no preceding whitespace — an
        // embedded/partial match) — must not strip real data.
        assert_eq!(
            strip_band_artifact("$1,001 - $15,000xgfedc"),
            "$1,001 - $15,000xgfedc"
        );
    }

    #[test]
    fn scan_rows_accepts_real_gfedc_artifact_evidence() {
        // Goal 081 Task 4.10: real `pdf_extract::extract_text_from_mem` lines
        // from a live 2020 electronic PTR, Filing ID #20016985, fetched
        // directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2020/20016985.pdf
        // (pdf sha256 ce68b1f8b7def98256506531edd2c98557a0844e481ce0126a4cfec510202d6a).
        // Two real rows: a single-line band carrying the trailing `gfedc`
        // artifact, and a long/wrapped band whose continuation line ALSO
        // carries it (Task 4.9's own wrap-join logic joins them into one
        // string BEFORE this fix strips the trailing token) — both
        // previously failed closed with `band "..." outside the grammar`.
        let region: Vec<String> = vec![
            "iD owner asset transaction".to_owned(),
            "type Date notification".to_owned(),
            "Date amount cap.".to_owned(),
            "gains >".to_owned(),
            "$200?".to_owned(),
            String::new(),
            "Alphabet Inc. - Class A (gOOgl)".to_owned(),
            "[ST] P 02/28/2020 03/20/2020 $1,001 - $15,000 gfedc".to_owned(),
            String::new(),
            "FIlINg STATuS: New".to_owned(),
            String::new(),
            "Royal Dutch Shell PlC Royal".to_owned(),
            "Dutch Shell PlC American".to_owned(),
            "Depositary Shares (RDS.B) [ST]".to_owned(),
            "S 02/21/2020 03/20/2020 $15,001 -".to_owned(),
            "$50,000 gfedc".to_owned(),
            String::new(),
            "FIlINg STATuS: New".to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 2);
        assert_eq!(drafts[0].amount_raw, "$1,001 - $15,000");
        assert_eq!(drafts[1].amount_raw, "$15,001 - $50,000");
    }

    #[test]
    fn scan_rows_accepts_real_gfedcb_variant_evidence() {
        // Goal 081 Task 4.10: the second real trailing-artifact form
        // (`gfedcb`, one letter longer than `gfedc`) confirmed against a
        // live 2020 electronic PTR distinct from the one above: Filing ID
        // #20016326, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2020/20016326.pdf
        // (pdf sha256 50218765b6aed95559b71d556e36e2e59b772c6195f39a443716c3cc57a4ef25).
        let region: Vec<String> = vec![
            "iD owner asset transaction".to_owned(),
            "type Date notification".to_owned(),
            "Date amount cap.".to_owned(),
            "gains >".to_owned(),
            "$200?".to_owned(),
            String::new(),
            "Chicago IL Met Wtr R 5%".to_owned(),
            "12/01/23 [gS] S 02/27/2020 03/10/2020 $50,001 -".to_owned(),
            "$100,000 gfedcb".to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].amount_raw, "$50,001 - $100,000");
    }

    #[test]
    fn parse_document_succeeds_against_real_2018_2022_gfedc_artifact_evidence() {
        // Goal 081 Task 4.10 end-to-end proof: `parse_document` now succeeds
        // on real rows carrying the `gfedc` PDF checkbox-widget artifact
        // trailing the amount band, which previously failed closed with
        // `band "..." outside the grammar`. The heading/header block and
        // both row lines below (one single-line band, one wrapped) are REAL
        // `pdf_extract::extract_text_from_mem` text verbatim from a live
        // 2020 electronic PTR: Filing ID #20016985, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2020/20016985.pdf
        // (pdf sha256 ce68b1f8b7def98256506531edd2c98557a0844e481ce0126a4cfec510202d6a).
        // Doc id / filer info / signature are clean synthetic grammar,
        // matching this suite's existing convention (real evidence spliced
        // in only for the elements this fix targets).
        let text = "\
Filing ID #20099997

name: Jane Filer
Status: Member
State/District: XX00

tranSactionS

iD owner asset transaction
type Date notification
Date amount cap.
gains >
$200?

Alphabet Inc. - Class A (gOOgl)
[ST] P 02/28/2020 03/20/2020 $1,001 - $15,000 gfedc

FIlINg STATuS: New

Royal Dutch Shell PlC Royal
Dutch Shell PlC American
Depositary Shares (RDS.B) [ST]
S 02/21/2020 03/20/2020 $15,001 -
$50,000 gfedc

FIlINg STATuS: New

* For the complete list of asset type abbreviations, please visit https://fd.house.gov/reference/asset-type-codes.aspx.

Digitally Signed: Jane Filer , 07/01/2020
";
        let doc = parse_document(text).unwrap();
        assert_eq!(doc.doc_id, "20099997");
        assert_eq!(doc.rows.len(), 2);
        assert_eq!(doc.rows[0].row.amount_raw, "$1,001 - $15,000");
        assert_eq!(doc.rows[1].row.amount_raw, "$15,001 - $50,000");
    }

    #[test]
    fn parse_document_succeeds_against_real_2016_non_zero_padded_signature_date_evidence() {
        // Goal 081 Task 4.11 sub-issue (a) end-to-end proof: `parse_document`
        // now succeeds when the signature date is non-zero-padded, which
        // previously failed closed with `signature date "02/1/2016" is not
        // MM/DD/YYYY`. The `Digitally Signed:` line is REAL
        // `pdf_extract::extract_text_from_mem` text verbatim from a live 2016
        // electronic PTR: Filing ID #20004485, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2016/20004485.pdf
        // (pdf sha256 58e99632e80ebe5418c206bdfa970056f6e3ff7f11217e71bc02208d7cd7dbf5).
        // Everything else is clean synthetic grammar, matching this suite's
        // existing splicing convention.
        let text = "\
Filing ID #20099996

name: Jane Filer
Status: Member
State/District: XX00

tranSactionS

iD owner asset transaction
type Date notification
Date amount cap.
gains >
$200?

Example Corp (EX) [ST] P 06/15/2016 06/20/2016 $1,001 - $15,000

FIlINg STATuS: New

* For the complete list of asset type abbreviations, please visit https://fd.house.gov/reference/asset-type-codes.aspx.

Digitally Signed: Hon. Brad Ashford , 02/1/2016
";
        let doc = parse_document(text).unwrap();
        assert_eq!(doc.doc_id, "20099996");
        assert_eq!(doc.rows.len(), 1);
        assert_eq!(doc.rows[0].row.signed_date_raw, "02/1/2016");
    }

    #[test]
    fn parse_document_succeeds_against_real_2014_missing_signature_label_evidence() {
        // Goal 081 Task 4.11 sub-issues (b)+(c) end-to-end proof:
        // `parse_document` now succeeds when the `Digitally Signed:` label is
        // genuinely absent AND the certification paragraph carries the
        // `gfedcb` artifact — previously failed closed with `missing
        // \`Digitally Signed:\` line`. The certification-area lines below are
        // REAL `pdf_extract::extract_text_from_mem` text verbatim from a live
        // 2014 electronic PTR: Filing ID #20000708, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/20000708.pdf
        // (pdf sha256 bfa02ca731327086bd2fe6d8d61408ebecb57d69652d1341e7adb39a8f19704a).
        // The heading/header block/row are clean synthetic grammar (that real
        // document's own row hits the separate, out-of-scope 2014 header-
        // block-shape gap), matching this suite's existing splicing
        // convention.
        let text = "\
Filing ID #20000708

name: Jane Filer
Status: Member
State/District: XX00

tranSactionS

iD owner asset transaction
type Date notification
Date amount cap.
gains >
$200?

Example Corp (EX) [ST] P 06/15/2014 06/20/2014 $1,001 - $15,000

FIlINg STATuS: New

* For the complete list of asset type abbreviations, please visit https://fd.house.gov/reference/asset-type-codes.aspx.

certification anD Signature

gfedcb  I CERTIFY that the statements I have made on the attached Periodic Transaction Report are true, complete, and correct to the

Filing ID #20000708

/  I CERTIFY that the statements I have made on the attached Periodic Transaction Report are true, complete, and correct to the
best of my knowledge and belief.

Mr. Michael C. Burgess , 04/14/2014
";
        let doc = parse_document(text).unwrap();
        assert_eq!(doc.doc_id, "20000708");
        assert_eq!(doc.rows.len(), 1);
        assert_eq!(doc.rows[0].row.signed_date_raw, "04/14/2014");
    }

    #[test]
    fn find_anchor_accepts_real_scrambled_case_type_tokens() {
        // Goal 081 Task 4.12(a): the same scrambled-case degradation Task 4.8
        // documented for headings/labels also hits the row-level type
        // token — arbitrarily per document, not a fixed positional rule.
        // Real `pdf_extract::extract_text_from_mem` lines, both
        // independently live-fetched and sha256-pinned this session:
        //   - a lowercase `"s"` alone — Filing ID #20016288 (2020),
        //     https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2020/20016288.pdf
        //     (sha256 7774958acf4269ed3270638a520b2f61fe6b908d62d053ae226425788c2f86f7)
        //   - a lowercase `"s (partial)"` — Filing ID #20009743 (2018),
        //     https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2018/20009743.pdf
        //     (sha256 2300e59b82f02b7d23b4df3457a603cfd5e83819c0280fa5462775c81bccfa61)
        // Both previously hard-rejected with `unknown transaction type token`.
        // The token is normalized to the canonical uppercase form
        // `normalize::normalize_row` expects (raw ticker/owner-code case
        // elsewhere in these same lines is untouched — a separate,
        // out-of-scope harmless-casing concern per Task 4.8's own note).
        let anchor = find_anchor("(BsX) [sT] s 02/05/2020 02/05/2020 $1,001 - $15,000 gfedc")
            .unwrap()
            .unwrap();
        assert_eq!(anchor.type_token, "S");

        let anchor = find_anchor(
            "JT alpha Pro Tech, ltd. (aPT) [sT] s (partial) 05/25/2018 05/25/2018 \
             $1,001 - $15,000 gfedcb",
        )
        .unwrap()
        .unwrap();
        assert_eq!(anchor.type_token, "S (partial)");

        // Already exact-case tokens keep matching unchanged (strict
        // superset — no regression to `anchor_splits_type_dates_and_band`).
        let anchor = find_anchor("Foo Corp [ST] P 01/02/2020 01/03/2020 $1,001 - $15,000")
            .unwrap()
            .unwrap();
        assert_eq!(anchor.type_token, "P");
    }

    #[test]
    fn scan_rows_joins_a_real_wrapped_comments_sub_line_continuation() {
        // Goal 081 Task 4.12(b): a sub-line's own free-text VALUE can wrap
        // onto further physical lines with no repeated label — previously
        // left as unattached `pending` text, hard-rejecting the whole
        // document (`unattached asset text after the last row` /
        // `sub-line ... amid unattached asset text`). Real
        // `pdf_extract::extract_text_from_mem` lines verbatim from a live
        // 2022 electronic PTR: Filing ID #20021740, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2022/20021740.pdf
        // (pdf sha256 6ba941b0a3d5047c95d1eb4724322ce46ec3cde0ff153b3aa591f7d6c06d697f).
        let region: Vec<String> = vec![
            "Apple Inc. (AAPL) [ST] P 08/26/2022 08/26/2022 $1,001 - $15,000".to_owned(),
            String::new(),
            "F S: New".to_owned(),
            "C: Purchase of AAPL Stock in three separate transactions on same day. \
             Individual transactions are below the"
                .to_owned(),
            "required threshold.".to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(
            drafts[0].comments_raw.as_deref(),
            Some(
                "Purchase of AAPL Stock in three separate transactions on same day. \
                 Individual transactions are below the required threshold."
            )
        );
    }

    #[test]
    fn scan_rows_joins_a_real_wrapped_description_sub_line_continuation() {
        // Goal 081 Task 4.12(b): the same wrap pattern also hits
        // Description, not only Comments — real `pdf_extract::
        // extract_text_from_mem` lines verbatim from a live 2026 electronic
        // PTR: Filing ID #20034201, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20034201.pdf
        // (pdf sha256 6372d59d32a7e54b69e4b456c315670456aa625572a5c78e5c29b72a81de2d43).
        let region: Vec<String> = vec![
            "Apple Inc. (AAPL) [ST] P 01/02/2026 01/03/2026 $1,001 - $15,000".to_owned(),
            String::new(),
            "F S: New".to_owned(),
            "S O: Putnam Investments".to_owned(),
            "D: The full transaction included the following sales: T \u{2013} 37.426 shares \
             sold @ $27.645/share BRK/B \u{2013} 3 shares"
                .to_owned(),
            "sold @ $493.42/share SPY \u{2013} 8.318 shares sold @ $670.024/share".to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(
            drafts[0].description_raw.as_deref(),
            Some(
                "The full transaction included the following sales: T \u{2013} 37.426 shares \
                 sold @ $27.645/share BRK/B \u{2013} 3 shares sold @ $493.42/share SPY \u{2013} \
                 8.318 shares sold @ $670.024/share"
            )
        );
        // A pre-existing single-line sub-line value (the overwhelming
        // common case) is unaffected: no continuation to join, and the next
        // row's own asset-name preamble must still land in `pending`, not
        // get absorbed as a "continuation".
        let region_single_line: Vec<String> = vec![
            "Apple Inc. (AAPL) [ST] P 01/02/2026 01/03/2026 $1,001 - $15,000".to_owned(),
            String::new(),
            "F S: New".to_owned(),
            String::new(),
            "Boeing Company".to_owned(),
            "(BA) [ST] S 02/02/2026 02/03/2026 $1,001 - $15,000".to_owned(),
        ];
        let drafts = scan_rows(&region_single_line).unwrap();
        assert_eq!(drafts.len(), 2);
        assert!(drafts[1].asset_raw.contains("Boeing Company"));
        assert!(!drafts[1].asset_raw.contains("New"));
    }

    #[test]
    fn scan_rows_accepts_a_real_band_wrap_across_a_page_break_with_a_header_reprint() {
        // Goal 081 Task 4.12(c): a page break can fall between a wrapped
        // band's hyphen and its `$…` continuation, landing a blank line and
        // a repeated header block in between — previously hard-rejected
        // (`band "..." wrap not followed by a `$…` continuation`) because
        // the old code only ever peeked exactly one line ahead. Real
        // `pdf_extract::extract_text_from_mem` lines verbatim from a live
        // 2023 electronic PTR: Filing ID #20023082, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2023/20023082.pdf
        // (pdf sha256 35f8d99e4c84d26ddebb219499c9f41bbff56dcdc0ef893e4962623642e0316e).
        let region: Vec<String> = vec![
            "ID Owner Asset Transaction".to_owned(),
            "Type Date Notification".to_owned(),
            "Date Amount Cap.".to_owned(),
            "Gains >".to_owned(),
            "$200?".to_owned(),
            String::new(),
            "2000074180 SP Dominion Energy, Inc. (D) [ST] S 12/15/2020 12/18/2020 $15,001 -"
                .to_owned(),
            String::new(),
            "ID Owner Asset Transaction".to_owned(),
            "Type Date Notification".to_owned(),
            "Date Amount Cap.".to_owned(),
            "Gains >".to_owned(),
            "$200?".to_owned(),
            String::new(),
            "$50,000".to_owned(),
            String::new(),
            "F S: Amended".to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].amount_raw, "$15,001 - $50,000");
    }

    #[test]
    fn scan_rows_accepts_a_real_band_wrap_split_by_a_filing_id_footer_and_header_reprint() {
        // Goal 081 Task 4.12(c): a second real shape — the page break lands
        // the `Filing ID #` footer directly after the hyphen (no blank line
        // at all), then the header-block reprint, before the real `$…`
        // continuation. Real `pdf_extract::extract_text_from_mem` lines
        // verbatim from a live 2023 electronic PTR: Filing ID #20023623,
        // fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2023/20023623.pdf
        // (pdf sha256 f0d3292c5db2b46013ef382bf0fc3e673144ba7820f225c845beea57ff812463).
        let region: Vec<String> = vec![
            "ID Owner Asset Transaction".to_owned(),
            "Type Date Notification".to_owned(),
            "Date Amount Cap.".to_owned(),
            "Gains >".to_owned(),
            "$200?".to_owned(),
            String::new(),
            "SP Royal Bank Of Canada (RY) [ST] P 08/09/2023 08/11/2023 $15,001 -".to_owned(),
            "Filing ID #20023623".to_owned(),
            "ID Owner Asset Transaction".to_owned(),
            "Type Date Notification".to_owned(),
            "Date Amount Cap.".to_owned(),
            "Gains >".to_owned(),
            "$200?".to_owned(),
            String::new(),
            "$50,000".to_owned(),
            String::new(),
            "F S: New".to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].amount_raw, "$15,001 - $50,000");
    }

    #[test]
    fn scan_rows_accepts_a_real_row_level_location_sub_line_inside_the_transactions_region() {
        // Goal 081 Task 4.12(d): a `L:` sub-line attached directly to a
        // transaction row (not only to an Investment Vehicle Details
        // bullet) — previously a hard reject (`L: sub-line inside the
        // Transactions region`). Real `pdf_extract::extract_text_from_mem`
        // lines verbatim from a live 2026 electronic PTR: Filing ID
        // #20034201, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20034201.pdf
        // (pdf sha256 6372d59d32a7e54b69e4b456c315670456aa625572a5c78e5c29b72a81de2d43).
        let region: Vec<String> = vec![
            "Invesco QQQ [OT] S (partial) 03/16/2026 03/16/2026 $1,001 - $15,000".to_owned(),
            String::new(),
            "F S: New".to_owned(),
            "S O: Putnam Investments".to_owned(),
            "L: US".to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].location_raw.as_deref(), Some("US"));
    }

    #[test]
    fn scan_rows_accepts_a_real_scrambled_case_full_text_location_sub_line_in_a_row() {
        // Goal 081 Task 4.12(d): the same row-level `L:` sub-line also
        // renders in the scrambled-case full-word form (Task 4.8's
        // mechanism) — real content verbatim from the live dry-run's own
        // fail-closed report against a 2020 electronic PTR: Filing ID
        // #20016088, sha256
        // 1ea1a47b83870a9f3ff0bf3310f56ee285dacc13530afbbf48509a6bca57f34c
        // (`cargo run -p worker --bin backfill -- --adapter us_house --from
        // 2020 --to 2020 --dry-run`, `L: sub-line inside the Transactions
        // region: "LoCaTIoN: Malvern, Pa, US"`).
        let region: Vec<String> = vec![
            "Foo Corp [ST] P 01/02/2020 01/03/2020 $1,001 - $15,000".to_owned(),
            String::new(),
            "F S: New".to_owned(),
            "LoCaTIoN: Malvern, Pa, US".to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].location_raw.as_deref(), Some("Malvern, Pa, US"));
    }

    #[test]
    fn parse_document_feeds_a_rows_own_location_sub_line_into_vehicle_location_raw() {
        // Goal 081 Task 4.12(d) end-to-end: no schema change — the row's
        // own `L:` sub-line feeds the SAME `vehicle_location_raw` Gold field
        // a vehicle-bullet join would otherwise populate. Real evidence
        // (Filing ID #20034201, see above) spliced with clean synthetic
        // filer/doc-id/signature grammar, matching this suite's convention.
        let text = "\
Filing ID #20099995

name: Jane Filer
Status: Member
State/District: XX00

TRANSACTIONS

ID Owner Asset Transaction
Type Date Notification
Date Amount Cap.
Gains >
$200?

Invesco QQQ [OT] S (partial) 03/16/2026 03/16/2026 $1,001 - $15,000

F S: New
S O: Putnam Investments
L: US

* For the complete list of asset type abbreviations, please visit https://fd.house.gov/reference/asset-type-codes.aspx.

Digitally Signed: Jane Filer , 03/31/2026
";
        let doc = parse_document(text).unwrap();
        assert_eq!(doc.rows.len(), 1);
        assert_eq!(doc.rows[0].row.vehicle_location_raw.as_deref(), Some("US"));
    }

    #[test]
    fn parse_document_still_falls_back_to_the_vehicle_bullet_location_without_a_row_l_sub_line() {
        // Regression guard for the same `vehicle_location_raw` wiring
        // change: a row with NO own `L:` sub-line must still fall back to
        // the Investment Vehicle Details bullet join exactly as before.
        let text = "\
Filing ID #20099994

name: Jane Filer
Status: Member
State/District: XX00

TRANSACTIONS

ID Owner Asset Transaction
Type Date Notification
Date Amount Cap.
Gains >
$200?

Invesco QQQ [OT] S (partial) 03/16/2026 03/16/2026 $1,001 - $15,000

F S: New
S O: My Trust

* For the complete list of asset type abbreviations, please visit https://fd.house.gov/reference/asset-type-codes.aspx.

I V D

My Trust
L: US

I P O

Digitally Signed: Jane Filer , 03/31/2026
";
        let doc = parse_document(text).unwrap();
        assert_eq!(doc.rows.len(), 1);
        assert_eq!(doc.rows[0].row.vehicle_location_raw.as_deref(), Some("US"));
    }

    #[test]
    fn scan_rows_accepts_the_real_2014_three_line_header_block_shape() {
        // Goal 081 Task 4.12(e): the 2014-era table-header block is a
        // genuinely different, SHORTER shape — no "Cap. Gains > $200?"
        // continuation at all — previously hard-rejected
        // (`unrecognized table header block`). Real `pdf_extract::
        // extract_text_from_mem` lines verbatim from a live 2014 electronic
        // PTR: Filing ID #20000077, fetched directly from
        // https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2014/20000077.pdf
        // (pdf sha256 ea936ce15201393a2fbfc61c9e9670e016fd5c6b0010aae8b750e34ebc924691).
        // The row itself also carries real evidence of Task 4.12(a)'s
        // scrambled-case type token (`s` for `S`) from this SAME document.
        let region: Vec<String> = vec![
            "iD owner asset transaction".to_owned(),
            "type Date notification".to_owned(),
            "Date amount".to_owned(),
            String::new(),
            "sP Hill International, Inc. (HIl) s 12/26/2013 12/30/2013 $15,001 - $50,000"
                .to_owned(),
        ];
        let drafts = scan_rows(&region).unwrap();
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].transaction_type_raw, "S");
        assert_eq!(drafts[0].amount_raw, "$15,001 - $50,000");

        // The exact-case rendering of this same shorter shape is also real
        // (confirmed via the live dry-run's own fail-closed report against
        // a second 2014 electronic PTR, Filing ID #20002042, sha256
        // c0921665f767172970259a4b2fb7e727af03a64aec0136ff8bb92909db84dd3b:
        // `unrecognized table header block at "ID Owner Asset Transaction"
        // (expected "Date Amount Cap.", got Some("Date Amount"))`).
        let region_exact_case: Vec<String> = vec![
            "ID Owner Asset Transaction".to_owned(),
            "Type Date Notification".to_owned(),
            "Date Amount".to_owned(),
            String::new(),
            "Foo Corp [ST] P 01/02/2014 01/03/2014 $1,001 - $15,000".to_owned(),
        ];
        let drafts = scan_rows(&region_exact_case).unwrap();
        assert_eq!(drafts.len(), 1);

        // The modern 5-line block (any year) keeps matching exactly as
        // before — never weakened by this additive alternative.
        let region_modern: Vec<String> = vec![
            "ID Owner Asset Transaction".to_owned(),
            "Type Date Notification".to_owned(),
            "Date Amount Cap.".to_owned(),
            "Gains >".to_owned(),
            "$200?".to_owned(),
            String::new(),
            "Foo Corp [ST] P 01/02/2020 01/03/2020 $1,001 - $15,000".to_owned(),
        ];
        let drafts = scan_rows(&region_modern).unwrap();
        assert_eq!(drafts.len(), 1);
    }

    #[test]
    fn parse_document_succeeds_against_the_real_2014_three_line_header_and_type_token_evidence() {
        // Goal 081 Task 4.12(a)+(e) end-to-end proof, both from the SAME
        // real document: Filing ID #20000077 (see above). The heading,
        // 3-line header block, and full row (including its scrambled-case
        // `s` type token) below are REAL `pdf_extract::extract_text_from_mem`
        // text verbatim; doc id / filer info / signature are clean
        // synthetic grammar, matching this suite's existing convention.
        let text = "\
Filing ID #20099993

name: Jane Filer
Status: Member
State/District: XX00

tranSactionS

iD owner asset transaction
type Date notification
Date amount

sP Hill International, Inc. (HIl) s 12/26/2013 12/30/2013 $15,001 - $50,000

FIlINg sTATus: New

initial Public offeringS

Digitally Signed: Jane Filer , 01/13/2014
";
        let doc = parse_document(text).unwrap();
        assert_eq!(doc.rows.len(), 1);
        assert_eq!(doc.rows[0].row.transaction_type_raw, "S");
        assert!(doc.rows[0].row.asset_raw.contains("Hill International"));
        assert_eq!(doc.rows[0].row.filing_status_raw, "New");
    }
}
