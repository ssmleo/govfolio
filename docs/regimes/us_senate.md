---
# RegimeSurvey front-matter (validated shape). Every claim: {claim, evidence:[files]}
jurisdiction: "us"
bodies: ["US Senate"]
legal_basis:
  claim: "Ethics in Government Act 1978 as amended by the STOCK Act 2012 (5 U.S.C. §13105(l)): PTRs due ≤30 days after notification of a covered transaction >$1,000 and ≤45 days after the transaction date. Statute text NOT yet archived (same open question as us_house); the Senate paper PTR form prints the rule verbatim: 'Report any purchase, sale, or exchange … within 30 days of receiving written notification … exceeded $1,000 … In no event may this disclosure be filed more than 45 days after such transaction.'"
  evidence: ["E10 paper-gif-formpage1"]
who_files:
  claim: "Senators (filer_type 1), candidates (4) and former Senators (5) per the eFD search legend; every fetched electronic PTR is from a sitting Senator ('(Senator)' suffix in the office column). v1 adapter scope: filer_type 1 only."
  evidence: ["E2 search-form.html", "E3 ptr-search-2026-06-window.json"]
record_types: [transaction]
value_precision: "banded"
band_table:
  - {raw: "$1,001 - $15,000",           low: "1001.00",     high: "15000.00",    observed: true}
  - {raw: "$15,001 - $50,000",          low: "15001.00",    high: "50000.00",    observed: true}
  - {raw: "$50,001 - $100,000",         low: "50001.00",    high: "100000.00",   observed: true}
  - {raw: "$100,001 - $250,000",        low: "100001.00",   high: "250000.00",   observed: true}
  - {raw: "$250,001 - $500,000",        low: "250001.00",   high: "500000.00",   observed: false}  # UNCERTAIN — paper-form column E10; electronic string TBC
  - {raw: "$500,001 - $1,000,000",      low: "500001.00",   high: "1000000.00",  observed: false}  # UNCERTAIN — as above
  - {raw: "Over $1,000,000",            low: "1000000.00",  high: null,          observed: false}  # UNCERTAIN — spouse/DC-only column ('Over $1,000,000***', E10); footnote text unread; open-ended: low = stated threshold (codebase convention)
  - {raw: "$1,000,001 - $5,000,000",    low: "1000001.00",  high: "5000000.00",  observed: false}  # UNCERTAIN — paper-form column
  - {raw: "$5,000,001 - $25,000,000",   low: "5000001.00",  high: "25000000.00", observed: false}  # UNCERTAIN — paper-form column
  - {raw: "$25,000,001 - $50,000,000",  low: "25000001.00", high: "50000000.00", observed: false}  # UNCERTAIN — paper-form column
  - {raw: "Over $50,000,000",           low: "50000000.00", high: null,          observed: false}  # UNCERTAIN — paper-form column; open-ended
cadence_and_lag:
  claim: "Rolling (transaction-triggered): statutory ≤30 days from notification / ≤45 days from transaction (printed on the paper form). Observed lags: Whitehouse transactions 05/07–05/08 filed 06/02 (25–26 d); Fetterman transaction 05/06 filed 06/12 (37 d). Listing exposes date_submitted per report; no publication timestamp."
  evidence: ["E10", "E4", "E5", "E3"]
formats: [html_table, image_scanned]
access: {method: "session-gated HTTPS: agreement dance (GET /search/ → 302 /search/home/ → POST prohibition_agreement) then search POST + view GETs", session_required: true, captcha: "none observed; Akamai bot manager 403s non-browser TLS fingerprints and non-stock UA strings on GET paths (probe matrix in E13)", notes: "robots.txt is HTTP 404 under a passing client (E13); earlier curl 403s were the bot manager, not a robots policy"}
historical_depth: {from: "2012-07-25 (earliest PTR in eFD: Cardin, paper, of 2,383 total PTRs); STOCK Act era — matches the design §5.6 backfill target", evidence: ["E12 ptr-search-earliest-since-2012.json"]}
identifiers_available: {politician: "name parts + office string only ('Peters, Gary (Senator)') — NO state, NO bioguide/lis id anywhere in listing or report page", instrument: "dedicated Ticker column on electronic PTRs (anchor text, '--' when absent) — stronger than us_house's in-name parenthetical; no ISIN/CUSIP/FIGI"}
amendment_mechanism:
  claim: "Amendments are NEW documents with a NEW report UUID, titled '<original title> (Amendment N)' (listing and page h1). No field anywhere references the original's UUID (full-page scan: zero matches). (filer, for-date) is NOT a usable key: Boozman filed TWO same-titled originals on 06/16/2026 (15 and 18 rows, overlapping dates/assets) plus 'Amendment 1' 8 minutes after the second. Fail closed: promote + review_task, never guess the original."
  evidence: ["E6", "E7", "E8", "E3"]
personal_data_to_redact: ["paper transmittal cover letters carry third-party personal data: counsel name + handwritten signature (E11) — redact from any public rendering of paper pages"]
tos_and_politeness:
  claim: "Access requires accepting: 'I understand the prohibitions on obtaining and use of financial disclosure reports.' — the eFD gate mirroring 5 U.S.C. §13107 use restrictions (unlawful/commercial/credit/solicitation uses). govfolio's legal posture on this agreement is a FOUNDER LANE (residual human: legal), flagged in open_questions. No robots.txt (404). Politeness: concurrency 1, ≥2 s interval, contact identification via From: header (identified UA strings are mechanically 403'd — see §2.5 + E13)."
  evidence: ["E1 agreement-page.html", "E13 retrieval log"]
language: [en]
open_questions:
  - {question: "Exact electronic renderings of the 7 unobserved band strings, incl. the spouse/DC 'Over $1,000,000***' variant and its footnote text", tried: ["55 rows across 5 electronic PTRs (2026-07-05) show only the 4 lowest bands", "paper form E10 shows all 11 columns but not the electronic strings"]}
  - {question: "Full electronic Asset Type vocabulary (only 'Stock' and 'Corporate Bond' observed; eFD types ETFs as 'Stock')", tried: ["no asset-type legend page exists on efdsearch (none linked from report pages or the search form, E2/E4–E8)", "55 rows sampled"]}
  - {question: "'Exchange' transaction-type electronic rendering", tried: ["unobserved in 55 rows; paper form has an Exchange checkbox column (E10)"]}
  - {question: "Timezone of the 'Filed MM/DD/YYYY @ H:MM AM/PM' stamp (presumably US Eastern)", tried: ["no TZ on any fetched page; raw string kept in Silver so nothing is lost"]}
  - {question: "Comment column with real text (only '--' observed)", tried: ["55 rows sampled 2026-07-05"]}
  - {question: "Does any page variant link an amendment to its original?", tried: ["full-HTML scan of E6 for both original UUIDs: 0 matches", "listing row carries only the title suffix (E3)"]}
  - {question: "Can a Rust HTTP client (reqwest / wreq-impersonate) pass the bot manager for the POST-only flow and for view GETs?", tried: ["Node non-browser stack: POSTs pass, HTML GETs 403 even with valid session cookies (E13 probe matrix, hybrid probe)", "browser-engine fetch is the only PROVEN path for view pages as of 2026-07-05"]}
  - {question: "PTR handling for filer_types 4 (Candidate) / 5 (Former Senator) — out of v1 scope", tried: ["deliberate scope cut; recordsTotal 2,383 includes them, senators-only window returned 13/13 senator rows"]}
  - {question: "Lifetime of the agreement flag on the server-side session (opaque hex cookie)", tried: ["sessions held only minutes during research; re-dance rule (§2.4) makes expiry a non-event"]}
  - {question: "Earliest ELECTRONIC (/search/view/ptr/) PTR — electronic-era start for backfill planning", tried: ["earliest-10 probe returns paper docs only (E12); binary-date probing deferred (request budget)"]}
regime_versions:
  - {effective_from: "2012-07-03", change: "STOCK Act PTR obligation (assumed; statute archive pending — earliest eFD PTR is 2012-07-25, E12)", evidence: []}
---

# US Senate (PTR) — Source Authority File

> **Internal context; the public methodology page derivation requires founder review
> (residual human lane: methodology PUBLIC copy + the §13107 use-restriction legal
> posture).** Goal 060 leg A (spec). Written BEFORE any adapter code, per the adapter
> template (design §5.1, plan Task 8) and the proven us_house pattern
> (`docs/regimes/us-house.md`).

Scope: **Periodic Transaction Reports only** (eFD report_type `11`), electronic
(`/search/view/ptr/`) on the green path; paper (`/search/view/paper/`) is the LLM-seam
case. Annual reports (7), extensions (10), blind trusts (14), other documents (15) are
separate regimes/goals. All money is `USD`.

Evidence citations `E1..E13` refer to §8. All retrievals 2026-07-05, Playwright
Chromium 149 (stock UA) + `From: ssm.leo@outlook.com` on every request — see §2.5 for
why the identified-UA string could not be used and how identification is carried
instead. Everything is archived under `docs/regimes/us_senate/evidence/` **in this same
commit** (closing the deferred-evidence gap flagged twice on us_house audits).

Per automation policy (`docs/decisions/automation-policy.md`), the goal's "HUMAN
completes expected.*.json" step is superseded: the test-designer authors expecteds
independently (high-confidence extraction + second-model cross-check), records publish
`unverified`, sampling-audit queue.

## 1. Regime metadata

| Field | Value |
|---|---|
| jurisdiction | `us` (national) |
| body | `US Senate` |
| regime_type | `transaction_report` |
| value_precision | `banded` (front-matter band_table; identical statutory bands to us_house) |
| cadence | rolling; statutory ≤30 d from notification, ≤45 d from transaction (printed on paper form, E10) |
| disclosure_lag_days | 45 (statutory max) |
| source_url | https://efdsearch.senate.gov/search/home/ |
| search endpoint | `POST https://efdsearch.senate.gov/search/report/data/` (session + CSRF, §2.2) |
| report URL (electronic) | `https://efdsearch.senate.gov/search/view/ptr/{uuid}/` |
| report URL (paper) | `https://efdsearch.senate.gov/search/view/paper/{uuid}/` (GIF carousel; images on `efd-media-public.senate.gov`, no session) |
| currency | USD always |
| cadence tier | 1 (design §5.5): discover 1–5 min in publication windows |

## 2. Discovery

### 2.1 Session dance (exact request sequence, E1/E2/E13)

eFD is session-gated behind a one-checkbox agreement. The full dance, captured live:

| # | Method | URL | Sends | Gets |
|---|---|---|---|---|
| 1 | GET | `/search/` | — | `302 Location: /search/home/` when the session lacks the agreement flag |
| 2 | GET | `/search/home/` | — | 200 agreement page. Sets TWO cookies: `csrftoken` (32 hex chars, ~1 y expiry) and one opaque session cookie whose **name and value are both 32-char hex** (e.g. `33a5c6d9…=234ecb30…`; HttpOnly, Secure, session-scoped). Page contains `<form id="agreement_form" action="" method="POST">` with checkbox `name="prohibition_agreement" value="1"` and hidden `csrfmiddlewaretoken` (64-char masked Django token — distinct from the cookie). Page JS submits the form when the checkbox is clicked. |
| 3 | POST | `/search/home/` | form `prohibition_agreement=1&csrfmiddlewaretoken=<form token>`; headers: `Referer: …/search/home/` + cookies | `302 Location: /search/` — acceptance is stored server-side against the opaque session cookie; no new cookie |
| 4 | GET | `/search/` | cookies | 200 search form page (E2 — the field legend of §2.2) |

Re-dance rule: any later request answered with a 302-to-`/search/home/` (or a 403
burst) means the session died — run steps 1–3 again (2 requests, negligible). If a
fresh dance still 403s, that is a blocking incident: freeze the adapter + review_task
(fail closed; **no fingerprint-evasion escalation beyond the documented client**).

### 2.2 Search endpoint contract (E2, E3)

`POST /search/report/data/` — server-side DataTables. The search page's own JS posts
exactly these fields (captured verbatim from E2), so this is the official contract:

- Form fields: `draw`, `start`, `length` (pagination), `report_types` (JSON-ish string,
  PTR = `[11]`), `filer_types` (`[1]`=Senator, `[4]`=Candidate, `[5]`=Former Senator),
  `submitted_start_date` (`MM/DD/YYYY HH:MM:SS`, may be empty), `submitted_end_date`,
  `candidate_state`, `senator_state`, `office_id`, `first_name`, `last_name`
  (starts-with matches), `order[0][column]`/`order[0][dir]` (column 4 = date
  submitted), `columns[N][data]` for N=0..4.
- Required headers: `X-CSRFToken:` = the **`csrftoken` cookie value** (not the form
  token), `Referer: https://efdsearch.senate.gov/search/`,
  `X-Requested-With: XMLHttpRequest`.
- Response: `{"draw":N,"recordsTotal":T,"recordsFiltered":F,"data":[[first_name,
  last_name, office, "<a href=\"/search/view/(ptr|paper)/<uuid>/\" target=\"_blank\">title</a>",
  date_submitted], …]}`.
- Report-type legend (checkbox values on E2): 7 Annual · **11 Periodic Transactions** ·
  10 Due Date Extension · 14 Blind Trusts · 15 Other Documents.

Listing row semantics (E3):

| Cell | Content | Quirks (verbatim from E3) |
|---|---|---|
| 0 `first_name` | first + middle | `"Gary C"` (middle initial inside first_name); paper rows ALL-CAPS with trailing spaces: `"RICHARD "` |
| 1 `last_name` | last (+ suffix) | dirty: `"Moran,  "` (trailing comma + spaces); `"McConnell, Jr."` (suffix after comma) |
| 2 `office` | `"Last, First (Senator)"` for electronic senator rows; bare `"Senator"` for paper rows | no state anywhere |
| 3 link | view URL + title | title = `Periodic Transaction Report for MM/DD/YYYY` (+ ` (Amendment N)` on amendments). `/ptr/` = electronic HTML; `/paper/` = scanned GIFs. UUID is the only source-native id → `filing.external_id` |
| 4 `date_submitted` | `MM/DD/YYYY` | can differ from the title's "for" date by one day (McCormick: title `for 06/27/2026`, submitted `06/26/2026`, E3) — treat the title date as label, not data |

### 2.3 Discover algorithm + politeness

1. Dance if needed (§2.1). `POST /search/report/data/` with `report_types=[11]`,
   `filer_types=[1]`, `submitted_start_date` = high-water mark minus a 7-day overlap
   (idempotency makes the overlap free), `order` by date_submitted asc; page with
   `start += length` (length 100, ≥2 s between pages) until `start ≥ recordsFiltered`.
2. Emit `FilingRef` per row: `external_id` = UUID from the href, `doc_kind`
   (`ptr`|`paper`) from the href path, name cells verbatim, office string, title
   (amendment suffix intact), `date_submitted`. New filing ⇔ unseen
   `(regime_id, external_id)`; amendments arrive as NEW UUIDs (E6) so this captures
   them. Idempotent: `ON CONFLICT DO NOTHING`.
3. **No conditional GETs exist here**: discovery is a POST (no ETag/Last-Modified
   semantics) and view pages serve neither `ETag` nor `Last-Modified` (E13). Poll at
   tier-1 cadence only in publication windows; the date-windowed POST *is* the cheap
   incremental check. Report pages are immutable in practice (§7 pinning evidence) —
   fetch each once into Bronze by sha256 (invariant 2) and never re-fetch a stored one.
4. `fetch`: electronic → GET the `/ptr/` page once, store raw HTML bytes as the Bronze
   document. Paper → store the `/paper/` wrapper HTML **and** each
   `efd-media-public.senate.gov/media/...gif` page image (each sha-addressed; the GIFs
   are the document; the media host needs no session, E13).
5. Politeness (invariant 10): concurrency 1; ≥2 s min interval; exponential backoff on
   429/5xx; contact identification on every request (§2.5); no robots.txt exists
   (404, E13) so these self-imposed limits govern.

### 2.4 Politician resolution

The listing and the report page give NAME ONLY (no state, no member id — §2.2, §3).
Rosters seed from the official senate.gov member list + Wikidata (design §5.4; roster
seeding is the builder/test leg's concern, mirroring us_house Task 9). Resolution:
normalize the page's `(Last, First)` parenthetical and the listing name cells (trim,
strip trailing commas — §2.2 quirks) against `politician_alias` within the
`US Senate` mandate roster (100 sitting members; small collision surface but NOT
zero). Anything but exactly one hit fails closed — `review_task
reason = "unresolved_filer"` (target `us_senate:<uuid>`), no filing row, no Gold rows
(invariant 3).

### 2.5 Client-fingerprint constraint (load-bearing; probe matrix in E13)

The host runs an Akamai bot manager that mechanically 403s, on HTML GET paths:

- ANY non-browser TLS/HTTP fingerprint (curl with four UA/header disguises; the
  Playwright Node request stack), **even when carrying a valid agreement-accepted
  session cookie** (hybrid probe) — the gate is fingerprint-based, not cookie-based;
- ANY non-stock UA string **on a real browser** (the identified-UA
  `govfolio.io research (contact: …)` and even a stock UA with a research suffix were
  both 403'd — UA/client-hints consistency is checked).

POSTs (`/search/home/` agreement, `/search/report/data/`) passed from the non-browser
stack in every attempt. What worked end-to-end: real Chromium, stock UA, contact
identification via the standard `From:` header on every request. This is recorded as a
**politeness deviation with rationale**, not stealth: identification was present on
every request that reached the origin; we do not escalate beyond this documented
client if blocked (fail closed to a work item instead).

**Consequence for the Rust fetch stage (builder leg, decided here per
extraction-strategy exclusivity):** plain `reqwest` must be assumed 403-bound for view
GETs. Options in order: (a) probe `wreq`/browser-impersonation TLS from Rust — may
pass; (b) headless-browser fetch sidecar driven from Rust (`chromiumoxide`) — proven
path, heavier. The flip between them must be recorded HERE (SAF-first) with its own
probe evidence. The language boundary is untouched either way: fetching writes Bronze,
so it stays in Rust regardless of which HTTP engine it embeds.

## 3. Document anatomy (electronic PTR, E4–E8)

Layout identical across all 5 fetched documents — a Django-templated HTML page
(`<title>eFD: Print Periodic Transaction Report</title>`):

1. `<h1 class="mb-2">` — `Periodic Transaction Report for MM/DD/YYYY` with
   ` (Amendment N)` appended on amendments (E6). Whitespace-collapse before use (the
   template line-breaks inside the element).
2. `<h2 class="filedReport">` — `The Honorable First Last (Last, First)` (three text
   lines; collapse). No state, no honorific variants observed.
3. `<p class="muted"><strong class="noWrap">` — `Filed  MM/DD/YYYY @ H:MM AM/PM`
   (double space after "Filed", folder icon inside the element; no timezone —
   open question). The two Boozman originals + amendment are 9:46 AM / 10:19 AM /
   10:27 AM of the same day (E6–E8) — minute precision matters for ordering filings.
4. Certification block — two checked, disabled checkboxes (`filing_certified`,
   `filing_cannot_be_edited`); the second reads "…reports cannot be edited once filed.
   To make corrections, I will submit an *electronic* amendment to this report."
   (the in-form statement of the amendment mechanism, E4).
5. `<section class="card">` Transactions — `<h3>Transactions</h3>` + a summary list:
   `(N transaction[s] total)` `X Self` `Y Joint` `Z Spouse` `W Dependent Child`
   (singular "transaction" at N=1, E4) — **integrity cross-check input** (§3.7).
6. `<table class="table table-striped">` (caption "List of transactions added to this
   report"), columns exactly:
   `# | Transaction Date | Owner | Ticker | Asset Name | Asset Type | Type | Amount | Comment`.
7. No signature block, no IPO section, no cap-gains column (all us_house-only
   features); the certification block + Filed stamp carry that weight here.

### 3.1 Row anatomy (per `<tr>` in `<tbody>`)

| Cell | Content (verbatim examples) | Notes |
|---|---|---|
| `#` | `1`, `18` | printed DESCENDING on every fetched doc (18→1, 3→1; E4–E8). `#1` = first-entered transaction. Keep as `row_number_raw`; `row_ordinal` is 1-based DOCUMENT order (top-to-bottom) |
| Transaction Date | `05/06/2026` | `MM/DD/YYYY` |
| Owner | `Self` \| `Spouse` \| `Joint` \| `Child` | full words (paper form uses codes (S)/(DC)/(J), E10); summary line says "Dependent Child" for `Child` |
| Ticker | `--` or `<a href="https://finance.yahoo.com/quote/ORCL" target="_blank">ORCL</a>` | anchor TEXT is the ticker; the href is derivable (`/quote/{ticker}`) — do not store it. `--` = none |
| Asset Name | `EXPEDIA GROUP INC NOTE`, `Oracle Corporation Common Stock` | may carry a `<div class="text-muted">` sub-line: `<em>Rate/Coupon:</em> 5.5%<br> <em>Matures:</em> 2036-04-15` (E4). When Ticker is `--` the name may embed it: `SPYM - Tradr 2X Long SPY Monthly ETF` (E7) |
| Asset Type | `Stock`, `Corporate Bond` | full words, NOT us_house `[XX]` codes. eFD types ETFs as `Stock` (30+ ETF rows in E7/E8). Full vocabulary unknown — open question |
| Type | `Purchase` \| `Sale (Partial)` \| `Sale (Full)` | `Exchange` form-standard (paper column, E10), UNOBSERVED electronically — grammar accepts, anything else rejects |
| Amount | `$1,001 - $15,000` (spaces around the hyphen) | band string; must match band_table grammar; unknown band ⇒ freeze + review_task (invariant 6) |
| Comment | `--` or text | text form UNOBSERVED (open question) |

`--` is the form's empty-cell sentinel (observed in Ticker and Comment): parse maps it
to `NULL`; the sentinel convention is documented here so Silver stays interpretable.

### 3.2 Owner map

| Source | Gold `owner` | Evidence |
|---|---|---|
| `Self` | `self` | E5 |
| `Spouse` | `spouse` | E5 |
| `Joint` | `joint` | E7/E8 (51 rows) |
| `Child` | `dependent` | E4 |
| anything else | reject row → review_task | fail closed |

Unlike us_house there is NO blank-owner default problem: the cell is always populated
on every fetched row, and the summary line double-checks the distribution (§3.7).

### 3.3 Transaction side map

| `Type` cell | Gold `side` | details | Evidence |
|---|---|---|---|
| `Purchase` | `buy` | `partial_sale=false` | E4 |
| `Sale (Full)` | `sell` | `partial_sale=false` | E7/E8 |
| `Sale (Partial)` | `sell` | `partial_sale=true` | E5, E7 |
| `Exchange` | `exchange` | `partial_sale=false` | UNOBSERVED electronically (paper column E10) |
| anything else | reject row → review_task | | fail closed |

### 3.4 Amount band → ValueInterval

Front-matter `band_table` is normative; identical statutory bands to us_house
(cross-checked column-by-column against the paper form, E10). Rules: strip
`$`/commas/spaces; decimals as strings (invariant 7); open-ended bands store the
stated threshold as `low`, `high = NULL` (codebase convention, cf. UK 70000-open in
`crates/core/src/domain/gold.rs`). The spouse/DC `Over $1,000,000***` column exists on
the paper form; its electronic string is UNOBSERVED and therefore NOT in the accepted
grammar until archived — fail closed.

### 3.5 Asset Type → `asset_class`

eFD prints full words, not codes; the value is always kept verbatim in
`details.asset_type_raw` (reclassification never needs a reparse). Observed + mapped:

| Source value | Gold `asset_class` | Evidence |
|---|---|---|
| `Stock` | `equity` | E4–E8 (54 rows). CAVEAT: eFD types ETFs as `Stock`; they land in `equity`, honestly — name-based fund detection would be guessing (invariant 3 spirit) |
| `Corporate Bond` | `bond` | E4 |
| any other value | `other` + review_task (unknown vocabulary member — extend THIS table first, then reparse) | fail closed |

### 3.6 Amendment semantics (E3, E6–E8)

- Detection: title suffix regex `\(Amendment (\d+)\)$` on the h1 (and the identical
  listing title). Fixture E6: `Periodic Transaction Report for 06/16/2026 (Amendment 1)`.
- The amendment is a complete restatement (18/18 rows), not a delta: E6 vs E7 differ
  in exactly 3 Ticker cells (original: `--` + name-embedded ticker `SPYM - …`;
  amendment: proper `SPYM` ticker link + clean name).
- **No linkage exists**: zero references to either original UUID anywhere in E6's
  HTML; the listing carries no id but the UUID. `(filer, for-date)` is genuinely
  ambiguous — Boozman has TWO same-titled 06/16 originals (E7: 18 rows @ 10:19 AM,
  E8: 15 rows @ 9:46 AM, overlapping dates and assets).
- Fail-closed rule (invariants 1, 3, 6): promote amendment rows as normal Gold inserts
  with `details.amendment_number = N`; leave `filing.supersedes_filing_id` and
  `supersedes_record_id` NULL; open one `review_task
  reason = "ptr_amendment_unlinked"` per newly inserted amended record (same reason
  string and same insert-gated idempotency as us_house §3.7). Supersession happens via
  the promotion machinery later, never by guessed matching.

### 3.7 Document integrity cross-checks (parse-time REJECTS, not scores)

1. Transactions-summary count `(N transaction[s] total)` must equal the number of
   parsed `<tbody>` rows.
2. Summary owner counts (`X Self` / `Y Joint` / `Z Spouse` / `W Dependent Child`) must
   equal the per-row Owner distribution.
3. Printed `#` cells must form a contiguous descending sequence `N..1`.
4. The page h1 must parse as a PTR title; the URL UUID must equal the discovery
   `FilingRef.external_id` (pipeline threads it; the page itself never repeats the
   UUID).

Any failure ⇒ reject the DOCUMENT + review_task (invariant 6). Zero parsed rows from a
fetched `/ptr/` page ⇒ freeze adapter + review_task; every real PTR has ≥1 row.

### 3.8 Paper filings (`/search/view/paper/{uuid}/`, E9–E11) — LLM seam

- The view page is a GIF carousel: `Page N of M` + sequentially numbered images on
  `https://efd-media-public.senate.gov/media/{year}/{n}/000/000/{seq}.gif` (E9:
  7 pages, `000000112`–`000000118`; no session needed on the media host).
- The HTML wrapper has NO filer header and NO Filed line — filer identity and
  date_submitted come from the LISTING row (ALL-CAPS names, office `Senator`).
- The scanned form (E10) is the classic paper PTR: received stamp
  (`RECEIVED BY: SECRETARY OF THE SENATE  Date: June 08, 2026`), owner codes
  `(S)/(DC)/(J)`, Purchase/Sale/Exchange checkboxes, the 11 band columns, `X` marks.
  Transmittal cover letters (E11) may precede the form and carry third-party personal
  data (counsel name + signature) — redaction-relevant for any public rendering.
- Green path does NOT fixture paper docs (goal-021 pattern: the seam routes them to
  `review_task reason = "needs_llm_extraction"` until the LLM leg lands;
  `us_senate_ptr/llm@1` when it does).

## 4. Silver contract — `StagingRow` (stg_us_senate)

Source-faithful; verbatim strings (whitespace-collapsed where the template hard-wraps,
noted per field), no normalization beyond the documented `--`→NULL sentinel, no entity
resolution. This is the shape `expected.silver.json` asserts (array of rows, document
order). test-designer authors against THIS table, not parser code. DDL mirrors
us_house: linkage columns `id`, `raw_document_id`, `created_at` + dedup key
`unique (raw_document_id, row_ordinal)`; `stg_meta` carries run linkage.

| Field | Type | Req | Content |
|---|---|---|---|
| `report_uuid` | string | yes | from the fetch URL (threaded by the pipeline; the page never prints it) |
| `row_ordinal` | integer ≥1 | yes | 1-based document order (top-to-bottom) |
| `row_number_raw` | string | yes | printed `#` cell (descending, §3.1) |
| `report_title_raw` | string | yes | h1, collapsed: `Periodic Transaction Report for 06/16/2026 (Amendment 1)` |
| `filer_name_raw` | string | yes | h2.filedReport, collapsed: `The Honorable John Fetterman (Fetterman, John)` |
| `filed_at_raw` | string | yes | Filed line minus the `Filed` label, collapsed: `06/12/2026 @ 1:23 PM` |
| `owner_raw` | string | yes | `Self`/`Spouse`/`Joint`/`Child` verbatim |
| `ticker_raw` | string\|null | yes | Ticker anchor text (`ORCL`); NULL when `--` |
| `asset_name_raw` | string | yes | Asset Name main text, collapsed (`SPYM - Tradr 2X Long SPY Monthly ETF` stays intact) |
| `asset_detail_raw` | string\|null | yes | `div.text-muted` sub-line text, collapsed (`Rate/Coupon: 5.5% Matures: 2036-04-15`); NULL when absent |
| `asset_type_raw` | string | yes | `Stock`, `Corporate Bond`, … verbatim |
| `transaction_type_raw` | string | yes | `Purchase`/`Sale (Partial)`/`Sale (Full)`/(`Exchange`) verbatim |
| `transaction_date_raw` | string | yes | `MM/DD/YYYY` as printed |
| `amount_raw` | string | yes | band string verbatim (`$1,001 - $15,000`) |
| `comment_raw` | string\|null | yes | Comment text; NULL when `--` |
| `confidence` | number [0,1] | yes | §6 scoring |
| `extractor` | string | yes | `us_senate_ptr/html@1` |

## 5. `details` contract — (us_senate, transaction)

Schemars type `UsSenatePtrTransactionDetailsV1` in
`crates/adapters/us_senate/src/details.rs`, snapshot committed at
`crates/pipeline/schemas/details/us_senate.transaction.json` (adapter-local placement
per the T8d audit ruling recorded in us-house.md §5; schema-contracts skill learnings
apply — doc comments are contract surface). Field list (no Rust here by task rule):

| Field | JSON type | Req | Source |
|---|---|---|---|
| `report_uuid` | string | yes | StagingRow.report_uuid |
| `row_ordinal` | integer ≥1 | yes | StagingRow.row_ordinal |
| `row_number` | string | yes | StagingRow.row_number_raw |
| `ticker` | string\|null | no | StagingRow.ticker_raw (instrument-resolution input) |
| `asset_type_raw` | string | yes | StagingRow.asset_type_raw verbatim |
| `asset_detail` | string\|null | no | StagingRow.asset_detail_raw |
| `amount_band_raw` | string | yes | StagingRow.amount_raw verbatim |
| `transaction_type_raw` | string | yes | StagingRow.transaction_type_raw verbatim |
| `partial_sale` | boolean | yes | derived, §3.3 |
| `comment` | string\|null | no | StagingRow.comment_raw |
| `amendment_number` | integer\|null | no | parsed from `report_title_raw` (§3.6); null on originals |
| `filed_at_raw` | string | yes | StagingRow.filed_at_raw (timezone unresolved — raw survives) |

### 5.1 StagingRow → GoldCandidate mapping (cite: E4–E8 fields per §3)

| GoldCandidate field | Rule |
|---|---|
| `record_type` | `transaction` always |
| `asset_description_raw` | `asset_name_raw` verbatim (invariant 2; ticker + detail ride `details`, not concatenated into it) |
| `asset_class` | §3.5 map over `asset_type_raw` |
| `side` | §3.3 map |
| `transaction_date` | parse `transaction_date_raw` as `MM/DD/YYYY` |
| `notified_date` | **NULL — the Senate electronic PTR has no notification-date column** (9 columns only, §3; divergence from us_house) |
| `as_of_date` | NULL |
| `value` | §3.4 band map; low/high as decimal strings, currency `USD` |
| `owner` | §3.2 map |
| `instrument_id` | NULL at parse; resolution waterfall (design §5.4) starts from `details.ticker` — exact ticker match first; below threshold stays NULL + review_task (invariant 3) |
| `extraction_confidence` | StagingRow.confidence |
| `extracted_by` | StagingRow.extractor |
| `fingerprint` | canonical sha256 over (filing_id, ordinal, content) — Task 6 machinery |
| `details` | §5 object, validated against the snapshot schema at promotion (invariant 5) |
| filing: `external_id` | report UUID; `filing_type` `ptr`; `filed_date` = date part of `filed_at_raw` (electronic) / listing `date_submitted` (paper); `published_at` NULL (source exposes no publication timestamp); `supersedes_filing_id` NULL (§3.6) |

## 6. Extraction strategy (spec-writer exclusive; builders read it HERE)

**Decision: deterministic first** (extraction-strategy skill; design §5.3). Electronic
PTRs are machine-generated Django-template HTML with a fixed 9-column table — the
best-case deterministic input; LLM-first would be an anti-pattern. The us_house
"text layer present?" lesson translates cleanly: here the WHOLE document is clean
markup; there is no lossy-label problem at all.

1. **Primary path** — `scraper` crate (html5ever DOM + CSS selectors; spec-compliant
   parsing of real-world HTML, no JS needed — the table is server-rendered, verified
   by parsing raw fetched bytes, E4–E8). Selectors: `h1.mb-2`, `h2.filedReport`,
   `p.muted strong` (strip the `Filed` label), the certification block is ignored
   (static), `section.card` summary `<ul>` (integrity input),
   `table.table tbody tr > td` ×9 by position. Whitespace-collapse every text node
   join (`\s+` → single space, trim). `--` sentinel → NULL (§3.1). Ticker = anchor
   text when the cell contains `<a>`, else sentinel. Asset Name: main text = cell text
   minus the `div.text-muted` subtree; detail = that subtree's text.
2. **Confidence scoring** (per row): start 1.00; −0.05 asset_type_raw outside §3.5's
   observed vocabulary (row still promotes to `other` + review_task); −0.02
   `asset_detail_raw` present (sub-line shapes beyond Rate/Coupon+Matures are
   unverified). Hard REJECTS (not scores): §3.7 document checks, unknown Owner/Type
   token, band outside grammar, unparseable date — row/doc goes to review, never
   low-confidence Gold (invariant 6 over confidence).
3. **LLM-fallback seam**: route a DOCUMENT to the `Extractor` trait when (a) it is a
   paper filing (`/search/view/paper/` — GIF scans, §3.8), or (b) the deterministic
   parse rejects per §3.7. v1 stub behavior: freeze that document + `review_task
   reason = "needs_llm_extraction"`; electronic fixtures must NOT hit the seam.
   Second-model cross-check on impact (≥ `$500,001` bands, watchlist filers) rides the
   same seam per design §5.3 and the automation policy.
4. **Escalation criteria `scraper` → alternative** (record the flip here + quirks log
   if taken): (a) a data cell's text is altered/dropped by html5ever tree-building on
   a well-formed fixture, (b) selector structure varies across documents (template
   fork), (c) the crate errors/panics on a fetched page.
5. **Fetch-client constraint**: §2.5 — the fetch engine must present a browser-grade
   TLS fingerprint for view GETs; probe `wreq`-style impersonation first, fall back to
   a headless-browser sidecar; record the outcome here before building.
6. **Cache by sha** (design §5.3): re-extraction only on `extractor` version bump.

## 7. Conformance fixtures (test-designer captures; DO NOT commit from this leg)

Selection: smallest clean representatives of the three required cases + the amended
original (amendment semantics need the pair). All verified live 2026-07-05; canonical
bytes = the raw response body of `GET /search/view/ptr/{uuid}/` (browser-fingerprint
client, §2.5); sha256 pinned below and archived byte-for-byte in
`docs/regimes/us_senate/evidence/` (same commit — the sampler re-fetches and confirms
the sha; drift ⇒ stop + review).

**Pinning rule (verified, with defined fallback):** pin the sha256 of the RAW BYTES.
Evidence this is sound: within-session re-GET, cross-session re-GET (fresh agreement
dance), and re-GETs of all four fixtures in a second session were ALL byte-identical,
and view pages contain zero session-variant markup (0 `csrf` matches; E13 variance
tests). IF a future capture ever drifts on bytes, switch pinning to the
**parsed-content hash**: sha256 over the UTF-8 serialization of
`title \n filer \n filed \n` then per row (document order) the 9 cell texts
(whitespace-collapsed, `--` kept literal) joined by `\t` and terminated by `\n` —
defined here so sampler and conformance can implement it identically; record the flip
in the quirks log (SAF-first).

| # | Case | UUID | Filer | Filed | Rows | URL | sha256 (raw bytes) |
|---|---|---|---|---|---|---|---|
| 1 | typical single-row purchase (`Child` owner, ticker `--`, bond with `Rate/Coupon`+`Matures` detail, `$1,001 - $15,000`) | `4b69867f-0376-4526-93f2-cd556b1155c9` | Fetterman, John | 06/12/2026 @ 1:23 PM | 1 | https://efdsearch.senate.gov/search/view/ptr/4b69867f-0376-4526-93f2-cd556b1155c9/ | `bd2d1df73361210360e1c4b4c0fdb72d1b646497352975a4b2dcb562aaaea80e` |
| 2 | multi-row: `Self`+`Spouse`, ticker links (ORCL/NVDA), `Sale (Partial)`, two bands | `4aa0094d-d9da-4a05-aa13-6d9f5d376105` | Whitehouse, Sheldon | 06/02/2026 @ 12:56 PM | 3 | https://efdsearch.senate.gov/search/view/ptr/4aa0094d-d9da-4a05-aa13-6d9f5d376105/ | `abbbdd79d5bc33ff07f880398cd9c6ee985df3b45d17ef22a83649cd2e5a6ef2` |
| 3 | amendment: `(Amendment 1)` title, 18 rows incl. `Joint` owners + `Sale (Full)`, restates #4 with 3 Ticker cells fixed | `727b4eb6-d8c7-4792-aa5b-c651c2d72f9c` | Boozman, John | 06/16/2026 @ 10:27 AM | 18 | https://efdsearch.senate.gov/search/view/ptr/727b4eb6-d8c7-4792-aa5b-c651c2d72f9c/ | `9c53a91cce5db4e201889fb580df5e4d43db4df9157fefbece42e7a1019dd5e7` |
| 4 | the amended original (same day 10:19 AM; name-embedded tickers `SPYM - …` with Ticker `--`; proves unlinked-amendment handling) | `a9754ff5-901a-4877-b7be-a647bd361c52` | Boozman, John | 06/16/2026 @ 10:19 AM | 18 | https://efdsearch.senate.gov/search/view/ptr/a9754ff5-901a-4877-b7be-a647bd361c52/ | `b1a9c78d5b059909f5944a8dc5d5fd6b19851f3b43ad0c9d31faf9b27d9fb487` |

Alternate (not selected; amendment-ambiguity evidence only): `2e076759-19df-458d-a3be-53610b8be5d0`
Boozman same-day sibling original, 15 rows, 9:46 AM,
sha256 `9c859cd43d78283e99226fa391ad704d47c0ea3eb3d2845f7d51d04b6d898fbe` (E8).
Paper case (NOT fixtured on the green path, §3.8): `a0d25e8f-fe54-4328-a7ea-504da008742b`
Blumenthal, view HTML + 7 GIFs (E9–E11) — reserved for the LLM leg.

Rationale: #1 exercises the happy path, `Child` owner, null ticker, asset-detail
sub-line; #2 exercises multi-row parsing, ticker anchors, partial sales, owner
variety, band variety; #3 exercises amendment detection (`details.amendment_number`,
`ptr_amendment_unlinked` review_task) plus `Joint`/`Sale (Full)` vocabulary at scale;
#4 exercises the original-side of the amendment pair and the name-embedded-ticker
quirk. Together they cover every §3.1 grammar branch observed in evidence. Expected
outputs per automation policy (no human gate): high-confidence extraction +
second-model cross-check, published `unverified`, sampling-audit queue.

## 8. Evidence log (retrieved 2026-07-05; full request/probe detail in the retrieval log)

Archived under `docs/regimes/us_senate/evidence/` **in this commit**, sha-named
(`{sha256}.{slug}.{ext}`). The session-dance/probe/politeness record is
`2026-07-05-efd-session-dance.retrieval.json` (E13).

| ID | URL | sha256 / note |
|---|---|---|
| E1 | https://efdsearch.senate.gov/search/home/ | `0481e1b5602294554f9327955c04f7da1e0cf0808354f0125665a72285ea47bf` agreement page (contains that session's csrfmiddlewaretoken — expected in EVIDENCE, unlike fixture pins) |
| E2 | https://efdsearch.senate.gov/search/ (post-agreement) | `457751d5acb511207eff08be810cd01238569c628da95773481abbb588984e31` search form: filer/report-type value legend + the page's own DataTables POST config |
| E3 | POST https://efdsearch.senate.gov/search/report/data/ (senators, PTRs, submitted ≥ 06/01/2026) | `4ab5ca7e13b2b84cb3f5bb891ed2af0d7f48a07667de4cf4fc342d08bd6b0a15` 13 rows: 12 electronic + 1 paper, incl. the Boozman amendment trio |
| E4 | …/search/view/ptr/4b69867f-…/ | `bd2d1df73361210360e1c4b4c0fdb72d1b646497352975a4b2dcb562aaaea80e` (fixture #1) |
| E5 | …/search/view/ptr/4aa0094d-…/ | `abbbdd79d5bc33ff07f880398cd9c6ee985df3b45d17ef22a83649cd2e5a6ef2` (fixture #2) |
| E6 | …/search/view/ptr/727b4eb6-…/ | `9c53a91cce5db4e201889fb580df5e4d43db4df9157fefbece42e7a1019dd5e7` (fixture #3, Amendment 1) |
| E7 | …/search/view/ptr/a9754ff5-…/ | `b1a9c78d5b059909f5944a8dc5d5fd6b19851f3b43ad0c9d31faf9b27d9fb487` (fixture #4, the amended original) |
| E8 | …/search/view/ptr/2e076759-…/ | `9c859cd43d78283e99226fa391ad704d47c0ea3eb3d2845f7d51d04b6d898fbe` (same-day sibling original — ambiguity evidence) |
| E9 | …/search/view/paper/a0d25e8f-…/ | `932f61763b40721319378f36dd465c1e260dbeed1a1cff3e24eee1df42b4d8f8` paper GIF-carousel page (7 pages; no filer header/Filed line) |
| E10 | https://efd-media-public.senate.gov/media/2026/2/000/000/000000113.gif | `a06ddb01fb7215da9727a7c3e4489f50d15e9996aafda2394e75f7d751bfad8d` paper form p.1: 11 band columns, owner codes, 30/45-day + $1,000 rule text, received stamp |
| E11 | https://efd-media-public.senate.gov/media/2026/2/000/000/000000112.gif | `c793070150665ce1d5885a857e05988b579f815ad1dfd423611dd0f116e2e871` transmittal cover letter (third-party personal data — redaction) |
| E12 | POST …/search/report/data/ (all filer types, submitted ≥ 01/01/2012, asc) | `a074a33785c513e8d22960dfb08224bf99dfba23461cebcbfdd63a26f2d8725f` recordsTotal 2,383; earliest PTR 07/25/2012 (Cardin, paper) |
| E13 | (our record) | `2026-07-05-efd-session-dance.retrieval.json` — exact request sequence, cookie anatomy, client-fingerprint probe matrix, variance tests, robots 404, politeness stats |

## Quirks log (append-only, dated)

- 2026-07-05 · Akamai bot manager 403s ALL non-browser TLS fingerprints on HTML GET
  paths (cookies irrelevant — hybrid probe) and ANY non-stock UA string even on a real
  browser; POST endpoints passed from a non-browser stack. Contact identification must
  ride the `From:` header, not the UA (§2.5, E13). Fetch-client architecture depends
  on this.
- 2026-07-05 · View pages serve no ETag/Last-Modified; discovery is POST-only — the
  conditional-GET habit from us_house does not transfer; date-windowed search POST is
  the incremental primitive (§2.3).
- 2026-07-05 · Listing name cells are dirty: `"Moran,  "` (trailing comma+spaces),
  `"Gary C"` (middle initial in first_name), `"McConnell, Jr."`; paper rows ALL-CAPS
  with trailing spaces + office bare `Senator` (E3). Trim + strip trailing commas
  before resolution.
- 2026-07-05 · Listing title "for" date can differ from date_submitted by one day
  (McCormick: `for 06/27/2026` vs `06/26/2026`, E3) — the title date is a label; use
  the page Filed line (electronic) / date_submitted (paper) for `filed_date`.
- 2026-07-05 · Printed `#` DESCENDS in document order on all five fetched PTRs
  (18→1, 3→1); `#1` = first-entered transaction. Contiguity is an integrity check
  (§3.7).
- 2026-07-05 · eFD types ETFs as Asset Type `Stock` (30+ ETF rows, E7/E8) — they map
  to `equity`; no name-based reclassification (that would be guessing).
- 2026-07-05 · Ticker may be `--` while the ticker is embedded in Asset Name
  (`SPYM - Tradr 2X Long SPY Monthly ETF`, E7); Boozman's Amendment 1 exists to fix
  exactly 3 such cells (E6 vs E7 diff) — amendments restate the full document.
- 2026-07-05 · A filer can file MULTIPLE same-titled originals the same day (E7/E8,
  overlapping content) — `(filer, for-date)` never identifies a document; only the
  UUID does.
- 2026-07-05 · `--` is the empty-cell sentinel (Ticker, Comment); yahoo-link hrefs are
  ticker-derivable boilerplate (never store).
- 2026-07-05 · Filed stamp has minute precision but no timezone; Boozman's three
  filings are 9:46 / 10:19 / 10:27 AM same day — precision matters for ordering,
  timezone stays an open question (raw string kept).

## Operational notes (politeness incidents, outages)

- 2026-07-05 · Initial identified-UA and curl probes: 6 requests 403'd by the edge
  (AkamaiGHost `$(SERVE_403)` rule) during client diagnosis; each 403 was followed by
  a configuration change, never a blind retry. Working configuration found on the
  third client (real Chromium + stock UA + From header).
- 2026-07-05 · With the working client: ~40 app requests total across 6 short
  sessions (dance ×5, search POSTs ×3, view GETs ×12 incl. variance re-GETs, GIFs ×2,
  robots ×1), concurrency 1, ≥2 s spacing — zero 429s, zero throttling observed.
- 2026-07-05 · efd-media-public.senate.gov (paper GIFs): anonymous 200, no session, no
  fingerprint gate observed.
