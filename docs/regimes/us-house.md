---
# RegimeSurvey front-matter (validated). Every claim: {claim, evidence:[files]}
jurisdiction: "us"
bodies: ["US House"]
legal_basis:
  claim: "Ethics in Government Act 1978 as amended by the STOCK Act 2012 (5 U.S.C. §13105(l)): PTRs due ≤30 days after notification of a covered transaction >$1,000 and ≤45 days after the transaction date. Statute text NOT yet archived (see open_questions); every archived PTR carries the certification 'I have disclosed all transactions as required by the STOCK Act'."
  evidence: ["E4 20020055.pdf", "E7 20033759.pdf"]
who_files:
  claim: "House Members ('Status: Member' on every sampled PTR), officers and covered staff. Candidate PTRs not observed; candidates file FD reports (FilingType C)."
  evidence: ["E4", "E5", "E6", "E7", "E8"]
record_types: [transaction]
value_precision: "banded"
band_table:
  - {raw: "$1,001 - $15,000",          low: "1001.00",     high: "15000.00",    observed: true}
  - {raw: "$15,001 - $50,000",         low: "15001.00",    high: "50000.00",    observed: true}
  - {raw: "$50,001 - $100,000",        low: "50001.00",    high: "100000.00",   observed: true}
  - {raw: "$100,001 - $250,000",       low: "100001.00",   high: "250000.00",   observed: true}
  - {raw: "$250,001 - $500,000",       low: "250001.00",   high: "500000.00",   observed: true}
  - {raw: "$500,001 - $1,000,000",     low: "500001.00",   high: "1000000.00",  observed: true}
  - {raw: "$1,000,001 - $5,000,000",   low: "1000001.00",  high: "5000000.00",  observed: true}
  - {raw: "$5,000,001 - $25,000,000",  low: "5000001.00",  high: "25000000.00", observed: false}  # UNCERTAIN — form-standard band, string not yet observed; grammar accepts, exact string TBC
  - {raw: "$25,000,001 - $50,000,000", low: "25000001.00", high: "50000000.00", observed: false}  # UNCERTAIN — as above
  - {raw: "Over $50,000,000",          low: "50000000.00", high: null,          observed: false}  # UNCERTAIN — as above; open-ended: low = stated threshold (codebase convention, cf. UK 70000-open)
cadence_and_lag:
  claim: "Rolling (transaction-triggered): statutory ≤30 days from notification / ≤45 days from transaction. Observed: Begich notified 05/13, signed 06/12 (30 days); Smucker notified 04/17–04/23, signed 04/30. Index zip regenerated at least daily (Last-Modified 2026-07-03 for a zip containing filings signed 07/02)."
  evidence: ["E1", "E4", "E6"]
formats: [pdf_text, pdf_scanned]
access: {method: "anonymous HTTPS GET", session_required: false, captcha: "none observed", notes: "No robots.txt (404, E11). ETag + Last-Modified served on index zip → conditional GETs work."}
historical_depth: {from: "assumed 2012 (STOCK Act era, backfill target per design §5.6); only 2026FD.zip verified", evidence: ["E1"]}
identifiers_available: {politician: "name + StateDst only — no bioguide/official id in index or PDF", instrument: "ticker in parentheses inside asset name; no ISIN/CUSIP/FIGI"}
amendment_mechanism:
  claim: "PTR amendments are NEW documents with FilingType P and a NEW DocID; amendment is visible ONLY per-row inside the PDF ('FILING STATUS: Amended') plus a populated row ID (eFD transaction id). Index gives no amendment signal and no link to the original. FilingType A is an FD (annual/candidate) amendment, NOT a PTR amendment."
  evidence: ["E7 20033759.pdf", "E9 10079846.pdf", "E1 index"]
personal_data_to_redact: []   # nothing beyond public-official name/district observed on PTRs; signature is typed text
tos_and_politeness: {claim: "No ToS page or robots.txt found on disclosures-clerk.house.gov (404). Apply house defaults: identified UA with contact, concurrency 1, min interval, conditional GETs.", evidence: ["E11"]}
language: [en]
open_questions:
  - {question: "Archive statutory text for 30/45-day + $1,000 threshold (5 U.S.C. §13105(l))", tried: ["2026-07-04 uscode.house.gov connect timeout", "2026-07-04 ethics.house.gov PTR page soft-404 (E10)"]}
  - {question: "FilingType semantics for C/D/H/T/W/X (P and A verified by document fetch)", tried: ["2026-07-04 fd-search page is a JS app, no static legend (E3)"]}
  - {question: "Exact strings of the three top bands and of the 'Spouse/DC over $1,000,000' variant", tried: ["not present in 274 sampled-index P filings' fetched subset"]}
  - {question: "'S (partial)' and 'E' transaction-type token exact rendering", tried: ["not present in 5 fetched PTRs"]}
  - {question: "DC / JT owner codes rendering", tried: ["not present in 5 fetched PTRs"]}
  - {question: "Checked-state rendering of Cap-Gains checkbox and IPO radio in the text layer", tried: ["all 5 fetched PTRs have them unchecked/No; text layer carries no state token"]}
  - {question: "Official instruction confirming blank Owner column = filer (self)", tried: ["2026-07-04 ethics.house.gov soft-404 (E10)"]}
  - {question: "Do paper PTRs (7-digit DocIDs) have any text layer?", tried: ["only HEAD-verified URL (E12); body not fetched"]}
  - {question: "Does {YYYY}FD.zip exist for historical years (backfill, goal 080)?", tried: ["only 2026 fetched; one-download budget for this task"]}
  - {question: "Is the original DocID de-listed or kept in its year's index after amendment?", tried: ["needs a cross-year index diff; original of 20033759 is in the 2025 index, out of this task's single-download budget"]}
regime_versions:
  - {effective_from: "2012-07-03", change: "STOCK Act PTR obligation (assumed; statute archive pending)", evidence: []}
---

# US House (PTR) — Source Authority File

Living canonical context for the `us_house` adapter (goal 001 Task 8). Specialists MUST
load this before any source-scoped task and MUST write back new learnings in the same PR.

Scope: **Periodic Transaction Reports only** (`FilingType == "P"`). Annual FDs, candidate
reports, extensions etc. are separate regimes/goals. All money is `USD`.

Evidence citations `E1..E12` refer to the Evidence log at the bottom. All retrievals
2026-07-04 (UTC), UA `govfolio.io research (contact: ssm.leo@outlook.com)`.

## 1. Regime metadata

| Field | Value |
|---|---|
| jurisdiction | `us` (national) |
| body | `US House` |
| regime_type | `transaction_report` |
| value_precision | `banded` (front-matter band_table) |
| cadence | rolling; statutory ≤30d from notification, ≤45d from transaction (archive pending, see open_questions) |
| disclosure_lag_days | 45 (statutory max) |
| source_url | https://disclosures-clerk.house.gov/FinancialDisclosure |
| index_url | https://disclosures-clerk.house.gov/public_disc/financial-pdfs/{YYYY}FD.zip |
| pdf_url (PTR) | https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/{Year}/{DocID}.pdf |
| currency | USD always |
| cadence tier | 1 (design §5.5): discover 1–5 min in publication windows |

## 2. Discovery

### 2.1 Index mechanics (E1)

- `GET https://disclosures-clerk.house.gov/public_disc/financial-pdfs/2026FD.zip` →
  200, `Content-Type: application/x-zip-compressed`, 48,167 bytes,
  `ETag: "3e8597f0ebadd1:0"`, `Last-Modified: Fri, 03 Jul 2026 13:00:36 GMT`,
  `Cache-Control: max-age=10058`.
- Zip contains `2026FD.txt` (TSV, header row, tab-separated) and `2026FD.xml`
  (UTF-8 **with BOM**; root `FinancialDisclosure`, repeated `Member` elements).
  Parse the XML as authoritative; TXT is a redundant rendering.
- 1,289 entries on 2026-07-04; FilingType census: A=29, C=605, D=54, H=2, **P=274**,
  T=2, W=90, X=233.

### 2.2 Index fields (`Member` element, E1)

| XML field | Semantics (evidence) | Notes |
|---|---|---|
| `Prefix` | honorific, usually `Hon.` for sitting members | UNRELIABLE as member test: blank for members Fields (LA06), Wied (WI08) |
| `Last`, `First`, `Suffix` | filer name parts | `Suffix` e.g. `III`, `Jr` |
| `FilingType` | single letter; `P` = PTR (verified: all 5 fetched P docs render "Periodic Transaction Report"); `A` = FD amendment — annual/candidate, NOT PTR (verified E9) | C/D/H/T/W/X semantics unverified (open question); adapter filters `== "P"` and ignores the rest |
| `StateDst` | 2-letter state + 2-digit district, `00` = at-large (`AK00`) | blank on some `W` rows |
| `Year` | index year | |
| `FilingDate` | `M/D/YYYY`, no zero-pad | blank on some `W` rows; for P rows equals PDF signed date in all 5 samples |
| `DocID` | numeric string, 4–8 digits, **opaque** | treat as string; do NOT infer time from it (E4: DocID 20020055 far below the contemporaneous 20033xxx–20034xxx range yet `FILING STATUS: New`, signed 06/12/2026 — allocation time ≠ filing time) |

### 2.3 DocID → PDF URL (P filings only)

`https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/{Year}/{DocID}.pdf`

- Verified for electronic 8-digit DocIDs beginning `2` (E4–E8, five bodies fetched).
- Verified for paper 7-digit DocID `9115726` (HEAD 200 at ptr-pdfs; 404 at
  financial-pdfs — E12). FD-type documents (A/C…) live under `financial-pdfs/` (E9).
- 2026 P DocID shapes: 246× 8-digit `2…` (electronic), 7× 7-digit `8…` + 21× 7-digit
  `9…` (paper/scanned, ~10%).

### 2.4 Discover algorithm + politeness

1. Conditional `GET` of current-year zip (`If-None-Match` / `If-Modified-Since`);
   304 → done. Poll at tier-1 cadence (1–5 min) only in publication windows, else
   relax; observed server `max-age=10058` (~2.8 h) is advisory — 304s are cheap.
2. On 200: parse XML, filter `FilingType == "P"`, emit `FilingRef` per row:
   `external_id = DocID`, `year`, name parts, `StateDst`, `FilingDate`.
   New filing ⇔ unseen `(regime_id, external_id)`; amended PTRs arrive as NEW DocIDs
   (E7) so this rule captures them. Idempotent: `ON CONFLICT DO NOTHING`.
3. `fetch`: GET the PDF once → Bronze by sha256 (invariant 2). PDFs are immutable;
   never re-fetch a stored sha.
4. Politeness (invariant 10): identified UA above; concurrency 1; min interval 2 s
   between requests to the host; exponential backoff on 429/5xx; no robots.txt
   exists (E11) so these self-imposed limits govern.

## 3. Document anatomy (electronic PTR)

Layout identical across all 5 fetched documents (E4–E8), 1–2 pages:

1. Header: `Filing ID #<DocID>` (top right), title "Periodic Transaction Report",
   Clerk address line.
2. **Filer Information**: `Name:` (e.g. `Hon. David Rouzer`), `Status:` (`Member`),
   `State/District:` (`NC07`).
3. **Transactions** table, columns: `ID | Owner | Asset | Transaction Type | Date |
   Notification Date | Amount | Cap. Gains > $200?`. Under each row up to four
   labelled sub-lines: `FILING STATUS:`, `SUBHOLDING OF:`, `DESCRIPTION:`, `COMMENTS:`.
   Footer: `* For the complete list of asset type abbreviations, please visit
   https://fd.house.gov/reference/asset-type-codes.aspx.`
4. **Investment Vehicle Details** (only when subholdings exist): bullet per vehicle,
   optionally `(Owner: SP)` suffix and a `LOCATION:` sub-line (E5, E6).
5. **Comments** section (may be empty even when a row has a comment, E7).
6. **Initial Public Offerings**: Yes/No radio (state NOT in text layer — open question).
7. **Certification and Signature**: fixed STOCK Act certification sentence +
   `Digitally Signed: <name> , <MM/DD/YYYY>` (note stray space before comma, E5–E8).

Multi-page: table header block repeats on page 2 (E6); rows flow across pages.

### 3.1 Text layer (the small-caps quirk — load-bearing)

The text layer is PRESENT on electronic PTRs and every **data** cell survives
extraction verbatim (verified with two independent extractors: xpdf `pdftotext 4.06`
and a second reader, E4–E8). BUT all headings/labels are set in a small-caps style
whose reduced glyphs have no usable ToUnicode mapping: **only word-initial capitals
survive**. Deterministic consequences (identical across extractors):

| Rendered label | Text layer yields |
|---|---|
| `FILING STATUS: New` | `F S : New` (whitespace varies) |
| `SUBHOLDING OF: X` | `S O : X` |
| `DESCRIPTION: X` | `D : X` |
| `COMMENTS: X` | `C : X` |
| `LOCATION: US` | `L : US` |
| section headings | `F I`, `T`, `I V D`, `I P O`, `C S`, `P T R` |

Anchor sub-line grammar on the surviving capitals + colon (tolerant whitespace):
`^F\s*S\s*:\s*(.+)$`, `^S\s*O\s*:\s*(.+)$`, `^D\s*:\s*(.+)$`, `^C\s*:\s*(.+)$`,
`^L\s*:\s*(.+)$`. Data labels with real mixed-case text (`Name:`, `Status:`,
`State/District:`, `Digitally Signed:`) extract intact.

### 3.2 Row grammar (content-stream order)

In content order (what `pdf-extract` emits) each row's cells are contiguous (E4–E8):

```
[row_id]? [owner_code]? <asset line(s)…> <type_token> <MM/DD/YYYY> <MM/DD/YYYY> <amount line(s)…>
F S : <status>
[S O : <vehicle>]?
[D : <description>]?
[C : <comment>]?
```

- `row_id`: 10-digit eFD transaction id; populated ONLY on amended rows (E7:
  `2000152831`); blank on all new rows (E4–E6, E8).
- `owner_code`: `SP` observed (E8); `JT`/`DC` unobserved (open question); blank
  otherwise.
- Asset cell wraps across lines; join with single spaces. Verbatim examples:
  `Boeing Company (BA) [ST]`, `Listen Ventures IV, LP [HN]`,
  `US Treas Bills MAT 11/19/26 [GS]`, `Intel Corporation - Common Stock (INTC) [OP]`.
  Trailing `[XX]` = asset-type code (legend E2); trailing `(TICK)` before the code =
  ticker candidate. Both stay inside `asset_description_raw` (raw is sacred).
- `type_token`: `P` | `S` observed; grammar also accepts `S (partial)` | `E`
  (form-standard, UNOBSERVED — fail closed on any other token).
- Two dates `MM/DD/YYYY`: transaction date, then notification date. Transaction date
  may precede the index year (E7: `12/09/2025` in the 2026 index).
- Amount: verbatim band string; wraps after the hyphen on long bands
  (`$1,000,001 -` ⏎ `$5,000,000`, E8); join with single space. Must match the
  band_table grammar; unknown band string ⇒ freeze + review_task (invariant 6).
- `Cap. Gains > $200?` checkbox: unchecked yields NOTHING in the text layer; checked
  rendering unknown (open question) ⇒ field is tri-state (true/false/null) and v1
  emits `null` unless a positive token is proven; `null` costs confidence (see §6).

### 3.3 Owner code map

| Source | Gold `owner` | Evidence |
|---|---|---|
| `SP` | `spouse` | E8 |
| `DC` | `dependent` | form-standard, UNOBSERVED |
| `JT` | `joint` | form-standard, UNOBSERVED |
| blank, vehicle has `(Owner: XX)` | map XX via this table; `details.owner_source = "vehicle"` | E6: blank rows under vehicle `Sale of Spouse Inherited Assets (Owner: SP)` ⇒ `spouse` |
| blank, no vehicle owner | `self`; `details.owner_source = "default_self"` | FLAGGED ASSUMPTION: standard reading (filer's own asset), official instruction not yet archived (open question). Raw stays null in Silver so a reread costs nothing. |

Any other/unparseable code ⇒ `unknown` + review_task (never guess).

### 3.4 Transaction side map

| `type_token` | Gold `side` | details | Evidence |
|---|---|---|---|
| `P` | `buy` | `partial_sale=false` | E4, E8 |
| `S` | `sell` | `partial_sale=false` | E6, E7 |
| `S (partial)` | `sell` | `partial_sale=true` | UNOBSERVED — exact string TBC |
| `E` | `exchange` | `partial_sale=false` | UNOBSERVED |
| anything else | reject row → review_task | | fail closed |

### 3.5 Amount band → ValueInterval

Front-matter `band_table` is normative. Rules: strip `$`/commas; decimals as strings
(invariant 7); open-ended band stores the stated threshold as `low`, `high = NULL`
(codebase convention, cf. UK 70000-open in `crates/core/src/domain/gold.rs`).
A possible `Spouse/DC over $1,000,000` variant (form-standard, UNOBSERVED) would map
to `low=1000000.00, high=NULL`; until its exact string is archived it is NOT in the
accepted grammar — fail closed.

### 3.6 Asset-type code → `asset_class`

Full 46-code legend archived (E2). Buckets for Gold `asset_class` (code always kept
in `details.asset_type_code`; reclassification never needs a reparse):

| Gold `asset_class` | Codes |
|---|---|
| equity | ST, PS, RS, SA |
| bond | CS, GS, AB, ET |
| fund | EF, MF, HE, HN, RF, RE, RN, 5C, 5F, 5P, 4K, IR, IH, IC, MA, BK |
| option | OP |
| crypto | CT |
| commodity | PM, FU, FE, CO |
| real_estate | RP, FA, MO, DS |
| private | OI, OL |
| other | OT, BA, TR, EQ, DB, PE, DO, IP, FN, VA, VI, WU |

UNCERTAIN cells (judgment calls, flagged for review, not blocking): SA/RS→equity,
ET→bond, HN→fund (vs private), REIT split, FU/FE/CO→commodity, MO/DS→real_estate,
BK/MA/IR/IH→fund. Missing `[XX]` ⇒ `other` + confidence penalty. Unknown code ⇒
`other` + review_task.

### 3.7 Amendment handling (evidence corrects the task brief)

The brief assumed "FilingType A supersedes → supersedes_filing". **Evidence says
otherwise** (E7, E9): PTR amendments are FilingType `P` documents with a new DocID;
`A` is the annual/candidate FD amendment type. Handling:

- Detection: per-row `F S : Amended` (E7). A document may in principle mix
  New/Amended rows — treat status row-locally.
- Amended rows carry `row_id` (eFD transaction id). Original rows print NO id
  (E4–E6, E8), and neither PDF nor index references the original DocID ⇒ the
  original filing CANNOT be identified deterministically today.
- Fail-closed rule (invariants 1, 3, 6): promote amended rows as normal Gold inserts
  with `details.filing_status_raw = "Amended"` and `details.row_id` set;
  leave `filing.supersedes_filing_id` and `supersedes_record_id` NULL; open a
  review_task `reason = "ptr_amendment_unlinked"`. Supersession happens later via
  the §7 promotion machinery (Task 11), never by guessed matching.
- If a future evidence pass shows amended `row_id`s matching ids that eFD exposes
  elsewhere, upgrade this rule here first (SAF-first discipline).

## 4. Silver contract — `StagingRow` (stg_us_house)

Source-faithful; verbatim strings, no normalization, no entity resolution. This is
the shape `expected.silver.json` asserts (array of rows, document order).
`null` = absent in source. test-designer authors against THIS table, not parser code.

| Field | Type | Req | Content (verbatim unless noted) |
|---|---|---|---|
| `doc_id` | string | yes | DocID; must equal PDF `Filing ID #` (cross-check, else reject doc) |
| `row_ordinal` | integer ≥1 | yes | 1-based across the whole document |
| `filer_name_raw` | string | yes | `Name:` value, e.g. `Hon. David Rouzer` |
| `filer_status_raw` | string | yes | `Status:` value (`Member`) |
| `state_district_raw` | string | yes | `State/District:` value (`NC07`) |
| `row_id_raw` | string\|null | yes | eFD transaction id (amended rows only) |
| `owner_code_raw` | string\|null | yes | `SP`/`DC`/`JT` or null |
| `asset_raw` | string | yes | full asset cell, wraps joined by single space, incl. `(TICK)` and `[XX]` |
| `asset_type_code_raw` | string\|null | yes | the `XX` from trailing `[XX]`, also kept inside `asset_raw` |
| `transaction_type_raw` | string | yes | `P`/`S`/`S (partial)`/`E` token as printed |
| `transaction_date_raw` | string | yes | `MM/DD/YYYY` as printed |
| `notification_date_raw` | string | yes | `MM/DD/YYYY` as printed |
| `amount_raw` | string | yes | band string, wraps joined by single space |
| `cap_gains_over_200` | boolean\|null | yes | null = indeterminate from text layer (v1 default) |
| `filing_status_raw` | string | yes | from `F S :` line (`New`, `Amended`, …) |
| `subholding_of_raw` | string\|null | yes | from `S O :` line |
| `description_raw` | string\|null | yes | from `D :` line |
| `comments_raw` | string\|null | yes | from `C :` line |
| `vehicle_owner_code_raw` | string\|null | yes | `XX` from `(Owner: XX)` of the row's matching Investment-Vehicle bullet |
| `vehicle_location_raw` | string\|null | yes | `L :` line of the matching vehicle bullet |
| `signed_date_raw` | string | yes | date from `Digitally Signed:` line |
| `confidence` | number [0,1] | yes | §6 scoring |
| `extractor` | string | yes | `us_house_ptr/text@1` |

Zero rows parsed from a fetched P document ⇒ freeze adapter + review_task
(invariant 6); every real PTR has ≥1 transaction row.

## 5. `details` contract — (us_house, transaction)

Schemars type `UsHousePtrTransactionDetailsV1` in
`crates/adapters/us_house/src/details.rs`, snapshot committed at
`crates/pipeline/schemas/details/us_house.transaction.json` (T8d audit ruling
2026-07-04: adapter-local placement wins — design §5.1 "core never changes when
coverage grows" + §9 crate map `crates/adapters/<x>/ … schemas/` supersede
§4.3's `crates/core/src/schemas/` wording for regime-specific types; both
placements are Rust, so the language boundary is untouched; promotion-time
validation stays central via the pipeline registry). Doc comments are contract
surface — schema-contracts skill learnings apply. Field list (no Rust here by task rule):

| Field | JSON type | Req | Source |
|---|---|---|---|
| `doc_id` | string | yes | StagingRow.doc_id |
| `row_ordinal` | integer ≥1 | yes | StagingRow.row_ordinal |
| `row_id` | string\|null | no | StagingRow.row_id_raw (amendment linkage key) |
| `asset_type_code` | string\|null | no | StagingRow.asset_type_code_raw |
| `amount_band_raw` | string | yes | StagingRow.amount_raw verbatim |
| `transaction_type_raw` | string | yes | StagingRow.transaction_type_raw verbatim |
| `partial_sale` | boolean | yes | derived, §3.4 |
| `cap_gains_over_200` | boolean\|null | no | StagingRow.cap_gains_over_200 |
| `filing_status_raw` | string | yes | StagingRow.filing_status_raw (`New`/`Amended`) |
| `owner_source` | string enum `row`\|`vehicle`\|`default_self`\|null | no | provenance of the Gold `owner` mapping (§3.3 auditability) |
| `subholding_of` | string\|null | no | StagingRow.subholding_of_raw |
| `vehicle_owner_code` | string\|null | no | StagingRow.vehicle_owner_code_raw |
| `vehicle_location` | string\|null | no | StagingRow.vehicle_location_raw |
| `description` | string\|null | no | StagingRow.description_raw |
| `comments` | string\|null | no | StagingRow.comments_raw |
| `signed_date` | string date (ISO) | yes | parsed StagingRow.signed_date_raw |

### 5.1 StagingRow → GoldCandidate mapping (cite: fixture fields per E4–E8)

| GoldCandidate field | Rule |
|---|---|
| `record_type` | `transaction` always |
| `asset_description_raw` | `asset_raw` verbatim (invariant 2) |
| `asset_class` | §3.6 map over `asset_type_code_raw` |
| `side` | §3.4 map |
| `transaction_date` | parse `transaction_date_raw` as `MM/DD/YYYY` |
| `notified_date` | parse `notification_date_raw` |
| `as_of_date` | NULL |
| `value` | §3.5 band map; low/high as decimal strings, currency `USD` |
| `owner` | §3.3 map |
| `instrument_id` | NULL at parse; resolution waterfall (design §5.4) may fill later; below threshold stays NULL + review_task (invariant 3) |
| `extraction_confidence` | StagingRow.confidence |
| `extracted_by` | StagingRow.extractor |
| `fingerprint` | Task 6 canonical sha256 over (filing_id, ordinal, content) |
| `details` | §5 object, validated against the snapshot schema at promotion (invariant 5) |
| filing: `external_id` | DocID; `filing_type` `P`; `filed_date` from index `FilingDate`; `supersedes_filing_id` NULL (§3.7) |

Politician resolution: index name parts + `StateDst` against the mandate roster
(official member list + Wikidata seed). No match or >1 match ⇒ filing-level
review_task, rows NOT promoted (Gold `politician_id` is NOT NULL; never guess).

## 6. Extraction strategy (spec-writer exclusive; builders read it HERE)

**Decision: deterministic first** (extraction-strategy skill; design §5.3). The text
layer is complete for every data cell on all sampled electronic PTRs (§3.1) and the
document is machine-generated with a fixed template — LLM-first would be an
anti-pattern here.

1. **Primary path** — `pdf-extract` text-layer read of electronic PTRs, content-order
   state machine over §3.2's grammar: locate the Transactions region, split rows on
   the `type_token + two dates + band` anchor (the only place two `MM/DD/YYYY` tokens
   are adjacent), attach preceding lines to asset/owner/id cells and following
   `F S :`/`S O :`/`D :`/`C :` sub-lines to the row; skip the repeated table-header
   block on page breaks (E6); read vehicle bullets from the `I V D` region and join
   to rows via exact `subholding_of_raw` string match.
2. **Confidence scoring** (recorded per row): start 1.00; −0.02 `cap_gains_over_200`
   null (v1 constant); −0.05 unknown asset-type code; −0.05 asset cell joined across
   a page break; −0.10 any sub-line label matched only loosely; −0.10 vehicle
   reference without a matching `I V D` bullet. Hard REJECTS (not scores): unknown
   `type_token`, band string outside grammar, unparseable date, doc_id mismatch —
   row/doc goes to review, never low-confidence Gold (invariant 6 over confidence).
3. **LLM-fallback seam** (goal 021 wires it; v1 = stub): route a DOCUMENT to the
   `Extractor` trait when (a) text layer yields zero Transactions rows, or (b) mean
   row confidence < 0.90, or (c) the doc is a paper filing (7-digit DocID — scanned,
   ~10% of 2026 P filings). v1 stub behavior: freeze that document + review_task
   `reason = "needs_llm_extraction"`; electronic fixtures must NOT hit the seam.
   Second-model cross-check on impact (≥ `$500,001` bands, watchlist filers) rides
   the same seam per design §5.3 and the automation policy's expected-output machine.
4. **Escalation criteria `pdf-extract` → `pdfium-render`** (record the flip here +
   quirks log if taken): any fixture where (a) a DATA cell glyph is missing or
   transliterated (label small-caps loss of §3.1 does NOT count — anchors avoid it),
   (b) content order breaks row contiguity (cells interleave across rows), or
   (c) the crate errors/panics on a well-formed fixture. `pdftotext -layout` already
   shows band/asset interleaving in LAYOUT mode (E6, E8) — irrelevant to the content-
   order path, listed so nobody "fixes" the parser toward layout mode.
5. **Cache by sha** (design §5.3): re-extraction only on `extractor` version bump.

## 7. Conformance fixtures (test-designer captures; DO NOT commit from this task)

Selection: smallest clean representatives of the three required cases + one
owner/options case. All verified live 2026-07-04; sha256 of fetched bytes pinned.
`capture_fixture` must re-fetch and confirm the sha (drift ⇒ stop + review).

| # | Case | DocID | Filer | Signed | Rows | URL | sha256 |
|---|---|---|---|---|---|---|---|
| 1 | typical single-row purchase (blank owner, `[HN]`, band $250k–$500k) | `20020055` | Hon. Nicholas Begich III (AK00) | 06/12/2026 | 1 | https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20020055.pdf | `4a12b888c2c89ebbfad5c280fa8a6af52489218dbec402ca2abc803436d8fa3f` |
| 2 | multi-row, 2 pages, sales, wraps, vehicle `(Owner: SP)` + blank row owners, 4 distinct bands | `20019182` | Hon. Lloyd K. Smucker (PA11) | 04/30/2026 | 8 | https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20019182.pdf | `5b1b60bea609310f4288adce9557702231cd1f23eb5ceabf1c0babc3fe867b37` |
| 3 | amendment: `F S : Amended`, populated `row_id 2000152831`, `C :` comment, prior-year transaction date | `20033759` | Hon. David Rouzer (NC07) | 01/07/2026 | 1 | https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20033759.pdf | `0a5861a182db417541f62a0179dfbba025d06cf1aa990c4d1931a2076760af1e` |
| 4 | explicit `SP` owner, options `[OP]`, `D :` description, top observed band $1M–$5M | `20034836` | Hon. Nancy Pelosi (CA11) | 06/23/2026 | 2 | https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20034836.pdf | `90bf98e6a2a3685f429964bb0e154ae05cc99423227b94666012332b81dc821e` |

Alternate (not selected; anatomy evidence only): `20034796` Cohen (TN09) — single row
with `S O :` + `L : US` vehicle; covered by #2's vehicle handling.
sha256 `2b78212b71e77830566cb541c2028b6b13ccc9e1f464e565acb1e739b510f1e6`.

Rationale: #1 exercises the happy path and the blank-owner default; #2 exercises
multi-page continuation, cell wrapping, vehicle-owner inheritance, band variety;
#3 exercises amendment detection + unlinked-supersession review_task; #4 exercises
explicit owner code, option asset class, and description sub-line. Together they
cover every §3.2 grammar branch observed in evidence. Expected outputs are produced
per automation policy (high-confidence extraction + second-model cross-check,
published `unverified`, sampling-audit queue) — no human gate.

## 8. Evidence log (retrieved 2026-07-04, UA as above)

Snapshots live in the task scratchpad, NOT committed (task rule: no fixtures/
downloads in repo; test-designer archives fixture bytes under
`crates/adapters/us_house/fixtures/`, and evidence snapshots for THIS doc's claims
should land under `docs/regimes/us-house/evidence/` in the fixture-capture PR —
flagged deviation from evidence-archiving's same-PR rule, accepted by dispatch).
sha256 pins below make every snapshot re-verifiable.

| ID | URL | sha256 / note |
|---|---|---|
| E1 | https://disclosures-clerk.house.gov/public_disc/financial-pdfs/2026FD.zip | `e5419282df7a96daa8aed108b72072a57bf909bb2a6c6333c938144f6898ba0d` (ETag `"3e8597f0ebadd1:0"`, Last-Modified 2026-07-03 13:00:36 GMT, 48,167 B) |
| E2 | https://fd.house.gov/reference/asset-type-codes.aspx | `be94889c6b578bb708274949710617030980c98920e261cbfd8db1310a484990` (46-code legend) |
| E3 | https://disclosures-clerk.house.gov/FinancialDisclosure | `a12698461d52486b706098ba4c1f36acce7a2ad7ce71bb2629257c72e13dc146` (JS app; no FilingType legend — tried-log) |
| E4 | …/ptr-pdfs/2026/20020055.pdf | `4a12b888c2c89ebbfad5c280fa8a6af52489218dbec402ca2abc803436d8fa3f` (fixture #1) |
| E5 | …/ptr-pdfs/2026/20034796.pdf | `2b78212b71e77830566cb541c2028b6b13ccc9e1f464e565acb1e739b510f1e6` (alternate) |
| E6 | …/ptr-pdfs/2026/20019182.pdf | `5b1b60bea609310f4288adce9557702231cd1f23eb5ceabf1c0babc3fe867b37` (fixture #2) |
| E7 | …/ptr-pdfs/2026/20033759.pdf | `0a5861a182db417541f62a0179dfbba025d06cf1aa990c4d1931a2076760af1e` (fixture #3, amendment) |
| E8 | …/ptr-pdfs/2026/20034836.pdf | `90bf98e6a2a3685f429964bb0e154ae05cc99423227b94666012332b81dc821e` (fixture #4) |
| E9 | …/financial-pdfs/2026/10079846.pdf | `b3bcd8067d6be9b17c8941959e937ad49e6108de299f1ef606ad37581da93eac` (FilingType A = FD amendment, "Congressional Candidate" Arias FL27 — NOT a PTR) |
| E10 | https://ethics.house.gov/financial-disclosure/periodic-transaction-reports-ptrs | `dd9d2f653459041568ac11df73074af26f442fabe73e0d8208fb806ffd4adc95` (soft-404 "Page not found" — tried-log) |
| E11 | https://disclosures-clerk.house.gov/robots.txt | HTTP 404 (no robots policy) |
| E12 | …/ptr-pdfs/2026/9115726.pdf | HEAD 200 (paper PTR at ptr-pdfs path); …/financial-pdfs/2026/9115726.pdf HEAD 404 |

## Quirks log (append-only, dated)

- 2026-07-04 · Small-caps labels lose non-initial glyphs in the text layer on ALL
  electronic PTRs; data cells unaffected. Anchor on surviving capitals (§3.1). Two
  extractors agree — this is in the PDFs, not an extractor bug.
- 2026-07-04 · DocID is NOT time-ordered: `20020055` signed 06/12/2026 amid
  20034xxx neighbors, status New (E4). Treat DocID as opaque string.
- 2026-07-04 · PTR amendments: FilingType stays `P`; amendment visible only per-row
  in the PDF; new DocID; no link to original anywhere (E7). Brief's "FilingType A"
  assumption corrected by evidence (E9: A = FD amendment).
- 2026-07-04 · Owner can be expressed ONLY at vehicle level (`(Owner: SP)`) with
  blank row owner column (E6) — vehicle-inheritance rule §3.3.
- 2026-07-04 · Checkbox/radio states (cap-gains, IPO) absent from text layer;
  tri-state modeling (§3.2, §4).
- 2026-07-04 · Index zip XML has UTF-8 BOM; some `W` rows have blank StateDst and
  FilingDate and 4-digit DocIDs (`8068`) — P-filter shields the adapter, but the
  XML reader must tolerate them.
- 2026-07-04 · `Digitally Signed:` line has a stray space before the comma
  (`Hon. Steve Cohen , 06/17/2026`) — trim when parsing `signed_date_raw`.
- 2026-07-04 · `pdf-extract 0.12` renders the lost small-caps glyphs of §3.1 as
  NUL characters (U+0000), e.g. `F\0\0\0\0\0 S\0\0\0\0\0: New` — strip NULs
  first, then anchor on the surviving capitals (`F S:`, `S O:`, `D:`, `C:`,
  `L:`); data cells carry no NULs and pass through verbatim. Page-2 content
  order confirmed on E6: page-1 `Filing ID #` footer + repeated 5-line table
  header block land BETWEEN row 6's sub-lines and row 7's asset cell; rows stay
  contiguous, so the §6.4 escalation criteria were NOT met and `pdf-extract`
  remains the extractor (adapter built against it, conformance ×4 green).

## Operational notes (politeness incidents, outages)

- 2026-07-04 · uscode.house.gov: connect timeout (statute archive pending).
- 2026-07-04 · ethics.house.gov PTR page: soft-404.
- 2026-07-04 · disclosures-clerk.house.gov: all requests 200-served, ETag present on
  the index; no throttling observed at concurrency 1 with ≥2 s spacing; 11 requests
  to the host this task (1 zip, 6 PDFs, 1 search page, 2 HEADs, 1 robots probe).
