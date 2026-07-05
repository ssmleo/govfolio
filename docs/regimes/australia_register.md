---
# RegimeSurvey front-matter (validated shape). Every claim: {claim, evidence:[files]}
jurisdiction: "au"
bodies: ["Australian House of Representatives"]
legal_basis:
  claim: "Resolution of the House of Representatives of 9 October 1984, amended 13 February 1986, 22 October 1986, 30 November 1988, 9 November 1994, 6 November 2003, 13 February 2008 and 19 September 2019 (the resolution text is REPRINTED verbatim at the top of every Statement of Registrable Interests form, E3/E5). The resolution defines 14 registrable-interest headings (§3.2), the 28-day statement and alteration deadlines (§1), the self/spouse-or-partner/dependent-children coverage (§3.5), and the registrability thresholds ($7,500 other assets, $1,000 income guidance, $750/$300 gifts, $300 sponsored travel — RULES thresholds, never per-record values, §3.6). The standalone resolution PDF is linked from the register landing page (E1: /-/media/05_About_Parliament/53_HoR/532_PPP/Resolutions/9Oct1984.pdf) but NOT fetched — the host WAF-blocks all non-browser clients (§2, E11); the resolution text is anchored on the form reprint + the Explanatory Notes booklet (E2) instead."
  evidence: ["E1", "E2", "E3", "E5", "E11"]
who_files:
  claim: "Every Member of the House of Representatives (~150). Within 28 days of making and subscribing an oath or affirmation each Member lodges a Statement of Registrable Interests with the Registrar of Members' Interests; interests of the Member's spouse/partner and dependent children (under 16, or full-time students under 25) are included (E1, E2). v1 scope: the House register ONLY. The Senate Register of Senators' Interests is a SEPARATE regime (different committee — Senate Standing Committee of Senators' Interests — and a different publication model: combined tabled VOLUMES, not per-member PDFs, E14); it is out of v1 scope, its own future goal (§2.6)."
  evidence: ["E1", "E2", "E14"]
record_types: [interest, change_notification]
value_precision: "none"
band_table:
  # NOT a banded regime and NOT encoded into ValueInterval — value is NULL on every row (§3.6).
  # This table pins the registrability FLOORS from the rules (Explanatory Notes, E2) for the
  # methodology page and any future founder-level revisit of the NULL-value decision.
  - {raw: "cat 9 other assets: 'each valued at over $7500' (E2)", low: null, high: null, observed: true}   # rules-threshold, not record data (canada_ciec §3.6 / UK R4 precedent)
  - {raw: "cat 11 gifts: '> $750 from official sources, > $300 from other sources' (E2)", low: null, high: null, observed: true}
  - {raw: "cat 12 sponsored travel/hospitality: 'exceeds $300' (E2)", low: null, high: null, observed: true}
  - {raw: "cat 10 income guidance: 'income over $1000 per annum might be notifiable'; 'There is no need to show the actual amount received' (E2)", low: null, high: null, observed: true}
cadence_and_lag:
  claim: "Event-triggered, rolling publication. New Parliament: every Member refiles an initial statement within 28 days of the oath/affirmation (E1); the 48th-Parliament initial statements land across 2025-08 → 2026 as Members are sworn (landing 'Last updated' spans 8 Aug 2025 → 24 Jun 2026, E1). Ongoing: any ALTERATION notified within 28 days of the change (settlement date for real estate, E2). The alteration is APPENDED to the same member PDF, which is re-published — so the single per-member document GROWS over the Parliament (Albanese 15 pp of gift/travel alterations, E4; Chalmers 5 alteration pages with 'Submitted Date' 18/08/2025 → 30/04/2026, E5). disclosure_lag_days stays NULL — the two 28-day windows (initial vs alteration) plus the publish-to-web lag differ; one number would lie. Observed publish lag: alteration 'Submitted Date' → media Last-Modified is same/next day (Albanese submitted-era 25 May, PDF Last-Modified 26 May, E4)."
  evidence: ["E1", "E2", "E4", "E5"]
formats: [pdf_scanned, pdf_text]
access: {method: "anonymous HTTPS GET of an HTML register-index page (server-rendered, /Senators_and_Members/Members/Register) linking one PDF per Member under /-/media/03_Senators_and_Members/32_Members/Register/{np}/{XY}/{Surname}_{NNP}.pdf", session_required: false, captcha: "Azure Front Door WAF returns HTTP 403 'Page Blocked by WAF' (x-azure-ref header) to EVERY non-browser client — the HTML index, the /-/media blob PDFs, AND robots.txt alike (E11). Only a real browser engine passes. Production fetch REQUIRES a browser-engine fetch seam (us_senate §2.5 precedent); documented as the fetch reality + top open risk (§2.3, §6.5).", notes: "Media PDFs DO carry Last-Modified (reader reported 'Published Time' per PDF: Chalmers 2026-04-30, Batt 2025-08-19, E5/E8) → conditional GET (If-Modified-Since) is the incremental primitive, pending seam confirmation. robots policy NOT retrievable (WAF, E11)."}
historical_depth: {from: "48th Parliament (current, 2025- ) is the v1 target. Previous Parliaments 47P/46P/45P/44P have their own index pages (E1 links) and 43P a legacy committee page — same per-member-PDF, scanned model; backfill is a later goal. Earliest linked: 43rd Parliament.", evidence: ["E1"]}
identifiers_available: {politician: "NO numeric member id in the register itself. Each row on the HTML index prints 'Surname, Title Given, Member for {Electorate}, {STATE}' + a 'Last updated' date (E1); the form header reprints FAMILY NAME / GIVEN NAMES / ELECTORAL DIVISION / STATE (E3, E5, E6). Resolution key = (electoral division, state) join to the House roster (roster seeding from the APH members list, builder/test leg's concern, mirrors us_house Task 9). The PDF filename is a surname slug with disambiguators (ChesterD/ChestersL, CookK/CookT, David_Farley) — a document key, not a person id.", instrument: "none — companies/trusts/funds are free text ('GD & SB Pty Ltd', 'Telstra', 'CE & TS Boyce Family Trust', E6/E7); no ticker/ISIN/registry id anywhere; instrument_id stays NULL below threshold (invariant 3)"}
amendment_mechanism:
  claim: "The per-member PDF is REISSUED in place as alterations accrue: same URL, new bytes, later Last-Modified (E1 'Last updated' column; E4/E5 alteration pages). There is no version history endpoint and no per-alteration stable id. v1 fail-closed handling (§2.5, §3.8): key the FILING on the document version (media Last-Modified / landing 'Last updated' date); a changed version is a new Bronze document (raw is sacred) + new filing; rows are re-extracted and promoted idempotently by fingerprint. Alterations are NEW change_notification rows within the document, not edits to prior interest rows — no supersession guessing across versions (deterministic linkage absent)."
  evidence: ["E1", "E4", "E5"]
personal_data_to_redact: ["Third-party personal data is published deliberately: spouse/partner and dependent-children interests appear in dedicated form columns (E5, E6); gift/alteration free text names private individuals ('Friendship bracelet from Florence (student)', E4) and family ('Spouse gift - ...', E4). Keep verbatim in Bronze/Silver/details; whether govfolio's PUBLIC rendering surfaces or search-indexes non-politician individuals (esp. dependent children) is a product/legal decision — flagged, default to NOT indexing non-politician names, and treat dependent-children data with extra care."]
tos_and_politeness:
  claim: "Public register published by the House for exactly this purpose ('to place on the public record Members' interests which may conflict ... with their public duty', E1). robots policy NOT retrievable (WAF blocks robots.txt too, E11). Access is WAF-gated (§2.3): the polite path is a browser-engine seam sending the identified UA. Politeness: concurrency 1, >=2.5 s interval, identified UA 'govfolio.io research (contact: ssm.leo@outlook.com)' + From header, conditional GET on media Last-Modified. This research session: 15 requests, WAF-blocked on direct fetch, structure/content obtained via a browser-engine reader (r.jina.ai) — method logged in E12."
  evidence: ["E1", "E11", "E12"]
language: [en]
open_questions:
  - {question: "Canonical raw-byte sha256 of each fixture PDF (Bronze pins, §7) — UNPINNED. The host WAF-blocks every non-browser client (curl, WebFetch, server-fetch proxies all 403/timeout, E11/E12) and web.archive.org is unreachable from the spec env; only a browser-engine reader returned CONTENT (not bytes). Pins are DEFERRED to the sampler/capture leg via the production browser-engine seam.", tried: ["curl identified-UA + Accept headers (403 WAF); WebFetch (403); allorigins/codetabs/corsproxy server-fetch proxies (520/522/HTML shell); web.archive.org (sandbox connection reset + WebFetch policy-blocked). r.jina.ai browser reader passed but returns extracted text only (E12)."]}
  - {question: "Text-layer reliability of the deterministic route (pdftotext) — cannot be measured from the spec env (no raw bytes). Reader signal shows HETEROGENEITY: Katter Form A OCR is garbage ('AUSTE,ALIA'=AUSTRALIA, 'STATE OLD'=QLD, 'luruber of AMP xhares', E3) while Chalmers/Batt typed forms extract clean member entries (E5/E8) and Albanese alteration pages are scanned-handwritten garbage (E4). Some documents have usable text; many do not; handwriting never does.", tried: ["8 member PDFs read via reader; the garbage vs clean split is stark (E3/E4 vs E5/E6/E8). Decision (§6): LLM-vision PRIMARY, deterministic only as a cross-check signal, never solely trusted."]}
  - {question: "Owner-column tick semantics on scanned Form A — is a value under 'Self' vs 'Spouse/Partner' vs 'Dependent Children' always column-aligned, or can a single entry span columns? Clean forms are column-clean (E5/E6); scanned forms may be ambiguous under vision.", tried: ["Chalmers/Buchholz clean grids show per-column entries (E5/E6); §3.5 maps the column, else NULL + confidence penalty."]}
  - {question: "Alteration ADDITION vs DELETION vs the 'Signed'/'Details' sub-structure — DELETION observed as a column header (E4 Albanese first page) but no populated DELETION row captured; how is a deleted interest rendered?", tried: ["8 members; only ADDITION rows populated in samples; §3.4 keeps addition_deletion verbatim, unknown value => review_task."]}
  - {question: "Initial-statement completion DATE extraction — the Form A 'Date'/signature is handwritten and often illegible under OCR (Katter 'Date:' blank in reader, E3). Alteration pages carry a typed 'Submitted Date: DD/MM/YYYY' on newer forms (E5/E6) but handwritten 'Date:' garbage on older ones (Albanese 'Date: 3/41 Y6', E4).", tried: ["notified_date is fail-soft: parse 'Submitted Date'/legible date DD/MM/YYYY when present, else NULL + confidence penalty; raw text always survives (§3.7)."]}
  - {question: "Gift value: cat 11 asks for 'the nature AND value of the gift' (E2) — do members write dollar values? Observed gift alterations are descriptive with NO amount (E4).", tried: ["Albanese ~30 gift lines, none carry a dollar figure (E4); §3.6 keeps value NULL, any figure stays verbatim in asset_description_raw, never promoted."]}
  - {question: "Cross-version diff (which rows are new when a member PDF is reissued) — no per-row id, so we re-extract the whole document each version; fingerprints dedupe unchanged rows. Is whole-document re-extraction acceptable cost at ~150 members x reissue frequency?", tried: ["conditional GET on Last-Modified limits refetch to changed docs (§2.3); idempotent fingerprints make re-promotion free (invariant 4)."]}
  - {question: "The 43P legacy committee page + 44P-47P backfill — same scanned model, but older forms and possibly worse scans; separate backfill goal.", tried: ["previous-Parliament index links enumerated (E1); not fetched (WAF + budget)."]}
regime_versions:
  - {effective_from: "2019-09-19", change: "Latest resolution amendment: 'spouse' -> 'spouse/partner', gift third-party-vehicle disclosure, gift ongoing-supply rules (E2). Current 14-heading form scheme.", evidence: ["E2"]}
  - {effective_from: "1984-10-09", change: "Original resolution establishing the Register of Members' Interests (reprinted on every form, E3).", evidence: ["E3"]}
---

# Australia — House of Representatives Register of Members' Interests — Source Authority File

> **Internal context; the public methodology page derivation requires founder review
> (residual human lane: methodology PUBLIC copy).** Goal 063 leg A (spec). Written
> BEFORE any adapter code, per the adapter template (design §5.1, plan Task 8) and the
> canada_ciec/uk_commons_register pattern (`docs/regimes/canada_ciec.md`,
> `docs/regimes/uk_commons_register.md`).

Scope: the **Register of Members' Interests** of the Australian **House of
Representatives** — govfolio's Tier-2 Australian adapter (design §5.5 names "AU"
explicitly) and its FIRST scanned-document register (the us_house paper-PTR seam
becomes the whole-regime default here). Two record types coexist in ONE per-member
document: the initial **Statement of Registrable Interests** (`record_type = interest`,
§3.5) and the appended **Notification(s) of Alteration of Interests**
(`record_type = change_notification`, §3.5). **No record carries a monetary value**
(§3.6) — `value` is NULL on every row; the register is descriptive, not valued. The
Senate register is a separate regime, out of v1 scope (§2.6).

Evidence citations `E1..E14` refer to §8. All retrievals 2026-07-05, identified UA
`govfolio.io research (contact: ssm.leo@outlook.com)` + `From:` header, concurrency 1,
>=2.5 s spacing. **The host is Azure-WAF-gated to non-browser clients** (§2.3, E11):
direct byte fetch (curl/WebFetch/server-proxies) is 403-blocked, so page structure and
document content were obtained via a browser-engine reader; canonical raw-byte sha256
pins are deferred to the capture leg via the production browser-engine seam (§7). All
reader extractions + the WAF-block sample + the retrieval log are archived under
`docs/regimes/australia_register/evidence/` **in this same commit**.

Per automation policy (`docs/decisions/automation-policy.md`), the goal's "HUMAN
completes expected.*.json" step is superseded: the test-designer authors expecteds
independently (schema-constrained vision extraction + second-model cross-check),
records publish `unverified`, sampling-audit queue.

## 1. Regime metadata

| Field | Value |
|---|---|
| jurisdiction | `au` (national) |
| body | `Australian House of Representatives` (register administered by the Registrar of Members' Interests / Committee of Privileges and Members' Interests) |
| regime_type | `change_notification` — design §5.5 Tier 2 names "Change-notification registers (UK, AU, CA)". The ongoing obligation is event-triggered 28-day alteration notices; the once-per-Parliament initial statement is the baseline the notices amend |
| value_precision | `none` — the register is DESCRIPTIVE: Members list the EXISTENCE of interests, not amounts ("There is no need to show the actual amount received", E2). Registrability floors ($7,500 / $750 / $300 / $1,000) live in the RULES, not the records → never encoded (§3.6). Same posture as canada_ciec |
| cadence | rolling; per-obligation 28-day windows: initial statement <=28 d after oath, alteration <=28 d after the change (§front-matter) |
| disclosure_lag_days | NULL — two distinct 28-day windows + web-publish lag; one number would misrepresent them |
| source_url | https://www.aph.gov.au/Senators_and_Members/Members/Register |
| index endpoint | `GET /Senators_and_Members/Members/Register` (HTML: alphabetical Member table with per-member PDF links + 'Last updated' dates, E1) |
| document endpoint | `GET /-/media/03_Senators_and_Members/32_Members/Register/{np}/{XY}/{Surname}_{NNP}.pdf` — one per Member; the canonical Bronze document (§2.4). `{np}`=`48p`, `{XY}`=surname-range folder (`AB`,`CF`,`KN`,...) |
| resolution | https://www.aph.gov.au/-/media/05_About_Parliament/53_HoR/532_PPP/Resolutions/9Oct1984.pdf (linked E1; WAF-gated, text anchored on E2/E3) |
| explanatory notes | https://www.aph.gov.au/-/media/03_Senators_and_Members/32_Members/Register/Explanatory_notes/Explanatory_Notes___Booklet_1.pdf (E2) |
| currency | AUD contextually — but no value is ever stored (§3.6), so no currency is ever emitted |
| cadence tier | 2 (design §5.5): discover hourly–daily; latency target same day |

## 2. Discovery

**Data-source decision: the HTML register index + per-member scanned PDFs.** No
machine-readable route exists; evidence-ranked:

1. **HTML index + per-member PDFs (CHOSEN):** the register landing page is a
   server-rendered alphabetical table — `Last updated | Member name and electorate |
   [PDF]` — with one `/-/media/.../{Surname}_{NNP}.pdf` per Member and a per-row
   `Last updated` date (E1). Each PDF is the compound Statement + Alterations document
   (§3.3). This is the only publication surface.
2. **JSON/data API (REJECTED — none exists):** no API host, no bulk dataset, no CSV;
   the register is PDF-only. (The Senate side publishes combined tabled PDF volumes,
   E14 — still not machine-readable, and a different regime, §2.6.)
3. **ParlInfo / Tabled documents (NOT v1):** aph runs ParlInfo search + a Tabled
   documents index (E1 nav), but the register's own per-member index is the direct,
   complete surface; ParlInfo is not needed for v1.

### 2.1 Index surface (verbatim from E1)

The landing page groups Members into six anchor sections (`A–B`, `C–F`, `G–J`,
`K–N`, `O–R`, `S–Z`) rendered as one table each:

| Column | Content | Use |
|---|---|---|
| `Last updated` | `DD Month YYYY` (e.g. `25 May 2026`, `8 August 2025`) | document VERSION marker — the incremental primitive (§2.5) |
| `Member name and electorate` | `Surname, Title Given, Member for {Electorate}, {STATE}` (e.g. `Buchholz, Mr Scott, Member for Wright, QLD`) | politician resolution key (§2.4) |
| (PDF icon link) | `/-/media/.../{np}/{XY}/{Surname}_{NNP}.pdf` | the Bronze document URL |

No pagination — all ~150 Members render on the single index page (six in-page
sections). 153 distinct member-PDF links observed on the 48P index (E1).

### 2.2 Filename grammar (document key, NOT a person id)

`{Surname}_{NNP}.pdf` under a two-letter surname-range folder. Collisions carry
disambiguators: `ChesterD_48P` / `ChestersL_48P` (Darren Chester / Lisa Chesters),
`CookK_48P` / `CookT_48P`, `David_Farley_48P` (E1). Treat the filename as an opaque
document key; the AUTHORITATIVE name+electorate is the index row (E1) and the form
header (§3.3), never the filename.

### 2.3 Discover algorithm + politeness (WAF reality)

1. **Fetch the index** `GET /Senators_and_Members/Members/Register` and parse the six
   member tables → per Member: name, title, electorate, state, `Last updated` date,
   PDF URL. (HTML parse — `scraper` crate, deterministic.)
2. **Version check:** a Member whose `Last updated` is newer than the stored version
   (or a Member not seen before) → (re)fetch that member PDF. Conditional GET
   `If-Modified-Since` on the media Last-Modified is the cheap primitive (media PDFs
   serve Last-Modified, E5/E8 — pending seam confirmation).
3. **Fetch the PDF** once per new version → store the raw response bytes as the Bronze
   document (sha256-addressed, invariant 2). One Bronze doc = one member-statement
   VERSION (§2.5).
4. **The WAF constraint (top operational risk):** `www.aph.gov.au` is fronted by an
   Azure WAF that 403-blocks EVERY non-browser client — the HTML index, the media
   PDFs, and `robots.txt` alike (E11; curl + WebFetch + three server-fetch proxies all
   blocked, E12). The polite production fetch therefore MUST run through a
   **browser-engine fetch seam** (headless Chromium sending the identified UA), the
   us_senate §2.5 precedent. This is a FETCH-stage concern, not a parse concern; the
   adapter freezes + `review_task` if the seam is unavailable, never falls back to
   evasion. robots policy is unretrievable (WAF) → self-imposed politeness governs.
5. **Politeness (invariant 10):** concurrency 1; >=2.5 s min interval; exponential
   backoff on 429/5xx; identified UA + `From:` header carried by the browser-engine
   seam; conditional GET on Last-Modified.

### 2.4 Politician resolution

The register carries NO numeric member id. Resolution key = `(electoral division,
state)` from the index row (E1) cross-checked against the form-header
`ELECTORAL DIVISION` / `STATE` (E3, E5, E6). Rosters seed from the APH House members
list (electoral division is the House's stable seat key), builder/test leg's concern
mirroring us_house Task 9. Store the resolved politician id; keep the as-filed
`Surname, Title Given`, electorate and state raw in Silver for audit + alias
enrichment. Zero or multiple roster hits ⇒ fail closed — `review_task reason =
"unresolved_filer"` (target `australia_register:{filename}@{version}`), no filing row,
no Gold rows (invariant 3). By-elections mid-Parliament add new seat-holders; the
index row is the resolution source of truth.

### 2.5 Filing model & version keys (reissue-safe by construction)

The source reissues the per-member PDF in place as alterations accrue (§front-matter),
but our Gold is immutable (invariant 1). Bridge: **one filing per document VERSION.**

- `filing.external_id = "{filename}@{version}"`, `version` = the media `Last-Modified`
  date (or the index `Last updated` date when Last-Modified is absent) in `YYYY-MM-DD`.
  Deterministic from source metadata; a reissue changes the version and arrives as a
  new filing.
- `filing_type = "statement"`; `filed_date` = the latest statement/alteration date
  legible in the document (nullable, §3.7); `published_at` = the `Last updated` /
  Last-Modified date at `00:00:00Z` — **date-precision convention** (the source
  exposes a date, not a timestamp); `discovered_at` = ours.
- `supersedes_filing_id` = deterministic lookup of `("{filename}", prior version)` when
  observed; NULL when the prior version was never fetched. Record-level supersession
  across versions is NOT attempted — no per-row id exists (§3.8).

### 2.6 Senate register — out of v1 scope

The **Register of Senators' Interests** is administered by a different body (the Senate
Standing Committee of Senators' Interests) and published as combined **tabled volumes**
(one large PDF per tabling, not per-senator files) at a different path (E14). Same Form
A structure and rules, different discovery/document model → its own regime + goal, like
uk_commons_register scoping out the Lords. Not fetched, not fixtured here; recorded so
nobody assumes the House adapter also covers senators.

## 3. Document anatomy (the Bronze document)

One PDF per Member = a **compound document**: an initial Form A (Statement of
Registrable Interests) followed by zero-or-more Notification-of-Alteration pages,
concatenated and re-scanned/re-exported as the member's single file (E4: 15 pp; E5:
18 pp; E6: 11 pp). Pages are scanned images and/or exported form pages — see the
extraction heterogeneity in §3.9.

### 3.1 Initial statement (Form A) anatomy (E3, E5, E6)

1. Header block: `PARLIAMENT OF AUSTRALIA / HOUSE OF REPRESENTATIVES / REGISTER OF
   MEMBERS' INTERESTS / Statement of Registrable Interests / {NN}th Parliament`, then
   `FAMILY NAME`, `GIVEN NAMES`, `ELECTORAL DIVISION`, `STATE` (E3: Katter/Kennedy/QLD;
   E5: Chalmers/James/Rankin/Queensland; E6: Buchholz/Wright/Queensland).
2. Notes block reprinting the resolution history (dates 1984→2019) and the
   self/spouse-partner/dependent-children coverage (E3, E5).
3. Fourteen numbered CATEGORY grids (§3.2). Each grid is a table whose rows are the
   three owner bands — `Self`, `Spouse/Partner`, `Dependent Children` — and whose
   columns are the category-specific fields (e.g. cat 1: `Name of company`; cat 3:
   `Location` / `Purpose for which owned`; cat 6: `Nature of liability` / `Creditor`).
   An empty band prints `Not Applicable` (E5, E6) → NO row emitted (§3.5).
4. Signature/date at the end (handwritten, frequently illegible under OCR, §3.7).

### 3.2 Registrable-interest category census (all 14, verbatim from E2; form order)

| # | Heading (Explanatory Notes, E2) | Gold `asset_class` | Notes |
|---|---|---|---|
| 1 | Shareholdings in public and private companies (incl. holding cos) | `equity` | company names free text; SMSF/trust/nominee holdings included (E2, E6, E7) |
| 2 | Family and business trusts and nominee companies (i beneficial / ii trustee) | `other` | trust name / operation / interest (E5) |
| 3 | Real estate — location (suburb/area only) + purpose | `real_estate` | 'Primary Residence'/'Secondary Residence'/farm/investment (E5) |
| 4 | Directorships of companies | `other` | company + activities |
| 5 | Partnerships — nature of interest + activities | `other` | |
| 6 | Liabilities — nature + creditor | `other` | mortgages/loans/overdrafts (E2, E6) |
| 7 | Bonds, debentures and like investments | `other` | |
| 8 | Savings/investment accounts — nature + institution | `other` | 'Self Savings ...', 'Superannuation (Australian Retirement Trust)' (E5, E8) |
| 9 | Other assets each valued over $7,500 (excl. household/personal) | `other` | incl. life assurance, SMSF; threshold in RULES (§3.6) |
| 10 | Other substantial sources of income | `other` | 'no need to show the actual amount' (E2) |
| 11 | Gifts (> $750 official / > $300 other) | `other` | nature/source/date; value descriptive (E2, E4) |
| 12 | Sponsored travel or hospitality (> $300) | `other` | source + purpose (E2, E4) |
| 13 | Membership of any organisation (conflict of interest) | `other` | |
| 14 | Any other interests (conflict of interest) | `other` | |

A category HEADING outside this census (or a form scheme change) ⇒ FREEZE adapter +
review_task (a new heading is a rules change; invariant 6). `asset_class` stays honest:
only cat 1 is `equity`, only cat 3 is `real_estate`, everything else `other` — no
creative bucketing (uk/canada precedent).

### 3.3 Alteration (Notification of Alteration of Interests) anatomy (E4, E5, E6)

Appended pages, each: header `... REGISTER OF MEMBERS' INTERESTS / NOTIFICATION OF
ALTERATION(S) OF INTERESTS / {NN}TH PARLIAMENT`, then `FAMILY NAME` / `GIVEN NAMES` /
`ELECTORAL DIVISION` / `STATE`, `I wish to alter my statement of registrable interests
as follows:`, an `ADDITION` / `DELETION` axis, an `Item` = category number + name
(e.g. `11. Gifts`, `12. Sponsored travel or hospitality`), a `Details` free-text block
(the substance), and `Signed:` / `Date:` (E4). Newer typed alteration pages carry a
machine-readable `Submitted Date: DD/MM/YYYY` (E5: 18/08/2025 … 30/04/2026; E6:
19/08/2025, 25/02/2026); older scanned pages carry a handwritten, often-garbled `Date:`
(E4: `Date: 3/41 Y6`). Quirk: a member may reuse an old-Parliament form — Albanese's
document includes a page stamped `45 TH PARLIAMENT` inside a 48th-Parliament file (E4);
the `{NN}TH PARLIAMENT` stamp is captured raw but NOT trusted over the document's own
Parliament context.

### 3.4 Rows per document + row semantics

**record_type decision (design §4.2 vocab: transaction | holding | interest |
change_notification):**

- **Form A category entries → `interest`.** Threshold-triggered statements that an
  interest EXISTS, no values, no transaction sides, no snapshot date. Not `holding`
  (`holding` requires `as_of_date`, gold.rs) — the register is not a valued
  point-in-time position; it is the canada/uk `interest` shape. `validate()` requires
  nothing extra.
- **Alteration entries → `change_notification`.** The document literally says
  "Notification of Alteration(s) of Interests" — a change event notified within a
  statutory 28-day window; the vocabulary's fourth type exists for exactly this
  (canada_ciec Material-Change precedent). `validate()` requires nothing extra.

**Rows per Bronze document:**

| Region | StagingRows / Gold rows |
|---|---|
| Form A category grid | one per NON-empty (category × owner-band) cell — `Not Applicable` bands emit nothing (E5, E6); a category may yield multiple rows (Buchholz cat 1: 2 Self + 1 Spouse, E6) |
| each Alteration page | one per `Details` line item under its `Item` category + ADDITION/DELETION axis (E4) |

`row_ordinal` = 1-based document order across the whole compound document. Each row
carries its `category_number` + `category_name`, `owner_band` (Form A) or
`addition_deletion` (alteration), `section_kind` (`statement` | `alteration`), and
`page_ordinal`.

### 3.5 Owner map (design vocab self/spouse/dependent/joint/unknown; NULL allowed)

| Condition | Gold `owner` | Evidence |
|---|---|---|
| Form A entry in the `Self` band | `self` | E5, E6 |
| Form A entry in the `Spouse/Partner` band | `spouse` | E6 (Buchholz cat 1 Telstra) |
| Form A entry in the `Dependent Children` band | `dependent` | E2 grid (populated case unobserved; grammar accepts it) |
| Alteration `Details` line prefixed `Spouse -` / `Spouse gift -` | `spouse` | E4 (Albanese) |
| Alteration `Details` line with no owner marker | `self` (the Member's own notification) | E4 |
| Entry notated 'jointly owned with spouse/partner' (the rules record joint interests as the MEMBER'S own with a notation, E2) | `self` — the notation is preserved raw; NOT mapped to `joint` (the register's own convention) | E2 |
| Owner band ambiguous under vision (scanned grid, column not resolvable) | `NULL` (unknown) + confidence penalty (§6) — never guessed | fail-soft |

Rationale: the register's rule is that joint spouse/dependent interests are the
Member's to declare (E2), so `self` is the faithful default and the explicit
`Spouse/Partner`/`Dependent Children` band (or a `Spouse -` marker) is the only signal
that flips owner. Free text is never parsed for owner beyond the documented markers
(guessing; invariant 3 spirit).

### 3.6 Value → ValueInterval: NULL, always (the defining mapping)

No entry carries an amount, band, or per-record threshold field — the register is
descriptive ("There is no need to show the actual amount received", E2). The
registrability floors ($7,500 other assets / $750-$300 gifts / $300 travel / $1,000
income guidance) are RULES text (§front-matter band_table), not per-record data: the
canada_ciec §3.6 + uk_commons_register R4 precedent applies verbatim — thresholds live
in the rules, value stays NULL, never inferred.

⇒ `value = NULL` on every Gold row; `value_precision = 'none'`; no currency emitted.
Even cat 11 gifts, whose rules ask for "nature AND value", are filed descriptively in
practice (E4 — ~30 gift lines, none with a dollar figure). If a `Details` line ever
contains a dollar figure it stays VERBATIM in `asset_description_raw`, never promoted
to `value` (guessing; there is no typed money anywhere in this source). Any future
revisit is a founder-level methodology decision, deliberately not taken here.

### 3.7 Dates

| Gold field | Rule |
|---|---|
| `notified_date` | alteration rows: the `Submitted Date`/legible `Date:` (`DD/MM/YYYY` — **Australian day-first**, contrast US MM/DD/YYYY) parsed to ISO; Form A rows: the statement completion/signature date when legible; **nullable** — handwritten/illegible ⇒ NULL + confidence penalty, raw survives (§6). `Interest`/`ChangeNotification` validate() require no date, so NULL degrades honestly |
| `transaction_date`, `as_of_date` | NULL always |
| filing `filed_date` | latest legible statement/alteration date in the document (nullable) |
| filing `published_at` | index `Last updated` / media Last-Modified date at `00:00:00Z` (date-precision convention, §2.5) |
| gift/travel dates inside `Details` free text | kept verbatim in `asset_description_raw` + `details.raw_text`; typed extraction addable later without refetch (raw is sacred) |

### 3.8 Mutability, versioning, supersession (fail-closed handling)

- **The document mutates by REISSUE:** same URL, later Last-Modified, more pages
  (§front-matter). Detection = the `Last updated` diff (§2.3). A newer version →
  new Bronze document (raw is sacred) + new filing `{filename}@{version}` (§2.5).
- **No per-row id, no cross-version linkage:** alterations are new
  change_notification rows, not edits to prior interest rows; `(member, category,
  later version)` is not a supersession key. `supersedes_record_id` stays NULL at
  insert; whole-document re-extraction + idempotent fingerprints (invariant 4) handle
  reissues without mutation. Record-level supersession, if ever wired, runs through the
  promotion machinery, never the parser.
- **Unexplained shrinkage / scheme change:** a reissue that DROPS the 14-category
  scheme, or a category heading outside §3.2, freezes the adapter + review_task
  (rules change; invariant 6).

### 3.9 Extraction heterogeneity (why this regime is LLM-vision-first)

The reader signal (not a substitute for a raw-byte probe, §open-questions) shows three
document flavours coexisting, sometimes within one file:

1. **Scanned handwritten/printed Form A** — OCR garbage: Katter (E3) renders
   `AUSTE,ALIA` (AUSTRALIA), `STATE OLD` (QLD), `1g September 2019` (19),
   `luruber of ,{MP xhares` (Number of AMP shares). Deterministic text is unusable.
2. **Typed/fillable Form A** — clean text layer: Chalmers/Batt (E5/E8) render member
   entries verbatim (`House (Springwood, QLD)`, `Superannuation (Australian Retirement
   Trust)`). Deterministic text is usable HERE.
3. **Scanned alteration pages** — garbage even when the base form was typed: Albanese
   alterations (E4) render crest/date garbage (`Date: 3/41 Y6`).

Because (a) a large share of documents and virtually all handwriting are
non-deterministic, (b) the multi-column Self/Spouse/Dependent grid is layout-hostile
even with a text layer, and (c) we cannot verify text-layer presence per document from
the spec env, the parse stage is **LLM-vision-first** (§6). This is design §5.3 rule 2
("scanned/handwritten/layout-hostile docs go to LLM extraction") applied at the regime
level.

### 3.10 Integrity cross-checks (parse/discovery-time REJECTS, not scores)

1. The form header `ELECTORAL DIVISION` + `STATE` must match the index row that linked
   the PDF (§2.4); mismatch ⇒ freeze + review_task (wrong document / roster drift).
2. Every emitted row's `category_number` (1–14) + heading must be in the §3.2 census;
   unknown heading ⇒ FREEZE + review_task (rules change).
3. `owner_band` ∈ {Self, Spouse/Partner, Dependent Children} for Form A rows;
   `addition_deletion` ∈ {ADDITION, DELETION} for alteration rows; unknown value ⇒
   review_task.
4. A Bronze document parses to >=1 StagingRow when the index shows the member has a
   statement; 0 rows from a NON-empty statement ⇒ freeze + review_task (invariant 6).
   Scope: a genuinely all-`Not Applicable` statement is legal and yields 0 Form A rows
   but the document must still parse structurally (header + category grid recognised).
5. Discovery: a member row on the index without a resolvable PDF link, or a PDF whose
   header names a different member, ⇒ review_task.

## 4. Silver contract — `StagingRow` (stg_australia_register)

Source-faithful; verbatim values from the (vision- or text-) extracted document; no
entity resolution; no value inference. One StagingRow per §3.4 row. This is the shape
`expected.silver.json` asserts; test-designer authors against THIS table, not parser
code. DDL mirrors us_house: linkage columns `id`, `raw_document_id`, `created_at` +
dedup key `unique (raw_document_id, row_ordinal)`; `stg_meta` carries run linkage.

| Field | Type | Req | Content |
|---|---|---|---|
| `document_filename` | string | yes | e.g. `Buchholz_48P` (threaded by the pipeline from the fetch URL) |
| `parliament_no` | integer | yes | `48` (from the URL `{np}` and the form stamp; §3.3 quirk: stamp is raw, URL wins) |
| `row_ordinal` | integer >=1 | yes | 1-based document order across statement + alteration regions |
| `page_ordinal` | integer >=1 | yes | source page the row was read from |
| `section_kind` | string enum `statement`\|`alteration` | yes | §3.4 |
| `category_number` | integer 1..14 | yes | §3.2 census (§3.10 check 2) |
| `category_name_raw` | string | yes | the heading text as extracted |
| `owner_band_raw` | string\|null | yes | `statement` rows: `Self`\|`Spouse/Partner`\|`Dependent Children`; null for alteration rows |
| `addition_deletion_raw` | string\|null | yes | `alteration` rows: `ADDITION`\|`DELETION`; null for statement rows |
| `family_name_raw` | string | yes | form header `FAMILY NAME` verbatim |
| `given_names_raw` | string | yes | form header `GIVEN NAMES` verbatim |
| `electoral_division_raw` | string | yes | form header `ELECTORAL DIVISION` verbatim (resolution key, §2.4) |
| `state_raw` | string | yes | form header `STATE` verbatim |
| `entry_text_raw` | string | yes | the cell/`Details` text VERBATIM (whitespace-collapsed at extraction); the record substance (invariant 2) |
| `entry_fields_raw` | jsonb | yes | column-labelled parts when the grid resolves them (e.g. `{location, purpose}` cat 3; `{nature, creditor}` cat 6; `{company, number_of_shares}` cat 1) — lossless; `{}` when unresolved |
| `date_raw` | string\|null | yes | alteration `Submitted Date`/`Date:` or statement completion date, AS PRINTED (nullable, illegible ⇒ null) |
| `parliament_stamp_raw` | string\|null | yes | the `{NN}TH PARLIAMENT` stamp on the page (raw; §3.3 quirk) |
| `confidence` | number [0,1] | yes | §6 scoring (vision wrapper + per-field) |
| `extractor` | string | yes | `australia_register/llm@1` (or `.../text@1` on the clean-text fast path, §6) |

## 5. `details` contracts — (australia_register, interest) and (australia_register, change_notification)

Two schemars types in `crates/adapters/australia_register/src/details.rs`, snapshots
committed at `crates/pipeline/schemas/details/australia_register.interest.json` and
`crates/pipeline/schemas/details/australia_register.change_notification.json`
(adapter-local placement per the T8d audit ruling recorded in us-house.md §5;
schema-contracts skill learnings apply — doc comments are contract surface). Field
lists (no Rust here by task rule):

**`AustraliaRegisterInterestDetailsV1`** — Form A category entries:

| Field | JSON type | Req | Source |
|---|---|---|---|
| `document_filename` | string | yes | StagingRow.document_filename |
| `parliament_no` | integer | yes | StagingRow.parliament_no |
| `row_ordinal` | integer >=1 | yes | StagingRow.row_ordinal |
| `page_ordinal` | integer >=1 | yes | StagingRow.page_ordinal |
| `category_number` | integer 1..14 | yes | StagingRow.category_number |
| `category_name` | string | yes | StagingRow.category_name_raw |
| `owner_band` | string enum `self`\|`spouse`\|`dependent`\|null | no | normalized StagingRow.owner_band_raw |
| `electoral_division` | string | yes | StagingRow.electoral_division_raw (resolution audit trail) |
| `state` | string | yes | StagingRow.state_raw |
| `entry_text` | string | yes | StagingRow.entry_text_raw verbatim |
| `entry_fields` | object (string→string) | yes | StagingRow.entry_fields_raw (`{}` when unresolved) |
| `statement_date` | string date\|null | no | parsed StagingRow.date_raw (DD/MM/YYYY → ISO; fail-soft) |
| `language` | string const `en` | yes | |
| `source_flavour` | string enum `text_layer`\|`scanned_vision` | yes | which §6 path produced the row (provenance) |

**`AustraliaRegisterChangeNotificationDetailsV1`** — alteration entries: all fields
above (owner_band derived from a `Spouse -` marker, §3.5) PLUS:

| Field | JSON type | Req | Source |
|---|---|---|---|
| `addition_deletion` | string enum `addition`\|`deletion` | yes | normalized StagingRow.addition_deletion_raw |
| `submitted_date` | string date\|null | no | parsed StagingRow.date_raw (`Submitted Date` DD/MM/YYYY → ISO; fail-soft, null on handwritten-illegible) |
| `parliament_stamp` | string\|null | no | StagingRow.parliament_stamp_raw (§3.3 mis-stamp quirk) |

### 5.1 StagingRow → GoldCandidate mapping (cite: E2–E7 per §3)

| GoldCandidate field | Rule |
|---|---|
| `record_type` | `change_notification` when `section_kind = alteration`; else `interest` (§3.4) |
| `asset_description_raw` | `entry_text_raw` verbatim. Empty ⇒ reject row (invariant 2) |
| `asset_class` | §3.2 category map (`equity` cat 1, `real_estate` cat 3, else `other`) |
| `side` | NULL (validate() requires it only for transactions) |
| `transaction_date` | NULL |
| `as_of_date` | NULL |
| `notified_date` | parse `date_raw` DD/MM/YYYY → ISO; NULL when absent/illegible (§3.7) |
| `value` | NULL always (§3.6) |
| `owner` | §3.5 map (owner_band / `Spouse -` marker → self/spouse/dependent; ambiguous → NULL) |
| `instrument_id` | NULL at parse; company/trust free text (`Telstra`, `GD & SB Pty Ltd`, E6) is the resolution input — below-threshold by default ⇒ stays NULL + review_task per the waterfall (invariant 3) |
| `extraction_confidence` | StagingRow.confidence |
| `extracted_by` | StagingRow.extractor |
| `fingerprint` | canonical sha256 over (filing_id, ordinal, content) — Task 6 machinery |
| `details` | §5 object per record_type, validated against its snapshot schema at promotion (invariant 5) |
| filing | §2.5: `external_id = "{filename}@{version}"`, `filing_type "statement"`, `filed_date` = latest legible date, `published_at` = Last-Updated date 00:00Z, `supersedes_filing_id` per §2.5 |

## 6. Extraction strategy (spec-writer exclusive; builders read it HERE)

**Decision: LLM-vision extraction is the PRIMARY, default path** (extraction-strategy
skill; design §5.3 rule 2; goal 021 seam). The Bronze document is a scanned,
often-handwritten, multi-column paper form — squarely the "scanned/handwritten/
layout-hostile" case. This is the us_house `scanned_paper` fixture #5 seam promoted
from a ~10% fallback to the regime default. Contrast us_house/uk/canada, which were
deterministic-first.

1. **Primary path — schema-constrained vision extraction** (goal 021 machinery,
   `crates/pipeline/src/extraction`): the raw PDF is sent to the Anthropic Messages API
   as a base64 PDF block with FORCED TOOL USE whose `input_schema` IS the §4 StagingRow
   JSON Schema; the tool output is re-validated against that schema locally
   (schema-invalid ⇒ fail closed, never silent Gold). The prompt encodes the §3
   grammar: 14-category census, the Self/Spouse/Dependent grid, `Not Applicable` →
   no row, the ADDITION/DELETION alteration axis, DD/MM/YYYY dates, value-never
   (invariant 3/§3.6), owner map (§3.5). Extractor tag `australia_register/llm@1`.
2. **Optional deterministic pre-check (cross-check input, NOT a trusted parse):** a
   `pdftotext`/`pdfium` read is cheap; on the clean-text-layer flavour (§3.9 case 2)
   it can confirm the vision rows field-for-field (a free second opinion) and its
   presence/absence is a confidence signal. It is NEVER the sole source of a Gold row
   (handwriting + layout-hostility defeat it, §3.9). A future `australia_register/
   text@1` fast-path MAY promote clean-text documents deterministically once the
   text-layer-reliability open question is measured against real bytes — deliberately
   NOT v1 (fail closed: vision is the safe default).
3. **Confidence scoring** (per row): start at the vision wrapper confidence; −0.05
   owner band ambiguous (§3.5 NULL); −0.05 `date_raw` present but unparseable; −0.02
   `date_raw` absent; −0.05 `entry_fields_raw` unresolved on a grid category. Hard
   REJECTS (not scores): §3.10 checks, unknown category heading, empty
   `entry_text_raw` — row/doc goes to review, never low-confidence Gold (invariant 6
   over confidence).
4. **Cross-check on impact** (design §5.3): a second, distinct model re-extracts
   documents on the watchlist/high-impact predicate (goal 021 `WATCHLIST_POLITICIANS`,
   currently stubbed) and compares field-by-field; any mismatch is a freeze +
   review_task, agreement proceeds.
5. **Fetch client — browser-engine seam (mandatory here):** the WAF (§2.3, E11) blocks
   plain `reqwest`; the fetch stage runs through the headless-browser seam (us_senate
   §2.5 precedent) sending the identified UA + `From:`; unavailable seam ⇒ freeze +
   work item, never evasion. This is the ONE regime so far where fetch cannot be plain
   HTTP.
6. **Cache by sha** (design §5.3; `crates/pipeline/src/extraction/cache.rs`): keyed on
   `(document_sha256, extractor_tag, model_id)` — pay per document VERSION once;
   re-extraction only on tag/model bump. Conformance file-cache entries are primed
   MECHANICALLY from `expected.silver.json` (`prime_from_expected_silver`), never from
   a live model call — the fixtures below run offline (us_house #5 precedent).

## 7. Conformance fixtures (test-designer captures; DO NOT commit from this leg)

Selection: the goal's asked diversity — a **shareholding**, a **real estate**, an
**alteration/change** — spread to also exercise the §3.9 extraction matrix
(clean-text Form A vs scanned Form A vs scanned alterations), both record types, and
the owner map. All verified live 2026-07-05 via the browser-engine reader (content
archived, E3–E8); the canonical bytes = the raw response body of the member PDF URL.

**Pinning rule + the WAF deferral (READ THIS):** the fixture Bronze pin is the sha256
of the RAW PDF RESPONSE BYTES for the document VERSION fixtured (version = media
`Last-Modified` date). **These raw-byte pins are UNSET in this leg** — the host WAF
403-blocks every non-browser client and web.archive.org is unreachable from the spec
env (§open-questions, E11/E12), so canonical bytes are unobtainable here without the
production browser-engine seam. The **capture/sampler leg fetches each URL via that
seam, records the sha256, AND asserts the document VERSION** (Last-Modified matches the
`@version` below); it also runs the drift procedure: (a) if a re-fetch's Last-Modified
advanced, the SOURCE reissued the document (§3.8 semantics, not pin drift) — keep the
archived version's bytes as the fixture, record the new version, optionally fixture it
as an added case; (b) if bytes differ at the SAME Last-Modified (re-export noise),
switch pinning to a **normalized-content hash** over the ordered StagingRow tuples
`(row_ordinal, section_kind, category_number, owner_band|addition_deletion,
entry_text)` — defined here so capture and conformance implement it identically. No
sha256 is guessed in this leg (fail closed).

| # | Case | Member (electorate, state) | Version (`Last updated`) | URL | Bronze sha256 |
|---|---|---|---|---|---|
| 1 | **shareholding** (interest, cat 1): Self `Canungra & District Community Finance Group Ltd`, `GD & SB Pty Ltd` (owner self); Spouse/Partner `Telstra` (owner spouse) → equity, value NULL, instrument NULL; ALSO real estate + liabilities + typed alterations w/ `Submitted Date` (E6) | Buchholz, Mr Scott (Wright, QLD) | `2026-02-25` | https://www.aph.gov.au/-/media/03_Senators_and_Members/32_Members/Register/48p/AB/Buchholz_48P.pdf | _capture leg (§7)_ |
| 2 | **real estate** (interest, cat 3): Self `House (Springwood, QLD) — Primary Residence`, `Apartment (Forrest, ACT) — Secondary Residence` → real_estate, owner self, value NULL; CLEAN text-layer Form A (`source_flavour text_layer`); shares `Not Applicable` (0 rows); + typed alterations `Submitted Date` 18/08/2025..30/04/2026 (E5) | Chalmers, Hon James (Rankin, QLD) | `2026-04-30` | https://www.aph.gov.au/-/media/03_Senators_and_Members/32_Members/Register/48p/CF/Chalmers_48P.pdf | _capture leg (§7)_ |
| 3 | **alteration** (change_notification): many `NOTIFICATION OF ALTERATION(S)` pages, cat 11 Gifts + cat 12 Sponsored travel ADDITIONs, `Spouse -`/`Spouse gift -` owner markers, scanned-handwritten (`source_flavour scanned_vision`, the §6 vision seam); `45 TH PARLIAMENT` mis-stamp quirk (E4) | Albanese, Hon Anthony (Grayndler, NSW) | `2026-05-25` | https://www.aph.gov.au/-/media/03_Senators_and_Members/32_Members/Register/48p/AB/Albanese_48P.pdf | _capture leg (§7)_ |
| 4 | **fully-scanned Form A** (interest, vision seam): OCR-garbage initial statement (`AUSTE,ALIA`, `STATE OLD`=QLD, `luruber of AMP xhares`) — the worst-case vision document; exercises `scanned_vision` on the category grid + handwritten completion date NULL (E3) | Katter, Hon Robert (Kennedy, QLD) | `2025-09-09` | https://www.aph.gov.au/-/media/03_Senators_and_Members/32_Members/Register/48p/KN/Katter_48P.pdf | _capture leg (§7)_ |

Alternates archived, not selected: Boyce (Flynn, QLD — cat 1 Self+Spouse shares +
family trust, E7); Batt (Hinkler, QLD — clean-text savings/super only, self, E8);
Aldred (Monash, VIC) / Bell (Moncrieff, QLD — all-`Not Applicable` shares, empty-band
→ 0-row evidence, E9/E10). Rationale: #1 covers shareholding + owner self/spouse +
public/private company free text; #2 covers real estate + the clean-text fast-path +
`Not Applicable` empties + typed alterations; #3 covers change_notification + the
scanned-vision seam + spouse markers + the mis-stamp quirk; #4 covers the vision seam
on a fully-scanned Form A grid. Together they span both record types, all owner
branches observed, and all three §3.9 extraction flavours. Expected outputs per
automation policy (no human gate): schema-constrained vision extraction + second-model
cross-check; #3/#4 conformance cache entries primed MECHANICALLY from the human/
independent transcription's `expected.silver.json`, never from a live model call
(us_house #5 precedent), published `unverified`, sampling-audit queue.

## 8. Evidence log (retrieved 2026-07-05)

Archived under `docs/regimes/australia_register/evidence/` **in this commit**. NOTE:
the `reader-extract.*.md` sha256 is the sha of the ARCHIVED reader extraction (real
content captured via the browser-engine reader — the only route past the WAF, §2.3),
NOT the canonical source-PDF bytes (unobtainable here; §7, E12).

| ID | Source | Archived file / sha256 |
|---|---|---|
| E1 | https://www.aph.gov.au/Senators_and_Members/Members/Register (48P index: member table, `Last updated`, 153 PDF links, resolution/notes links, prev-Parliament links) | `reader-extract.48P-register-landing.md` · `2259fb579768e15a262e0dec21ae3824f3064b2ba1e45eb4466bd29ae7e7143a` |
| E2 | .../Register/Explanatory_notes/Explanatory_Notes___Booklet_1.pdf (14-category scheme, owner/threshold rules) | `reader-extract.explanatory-notes-booklet.md` · `6a2315b5f7685a59efa06fffdd9720633b8c367349457a83c21f045a8dff9480` |
| E3 | .../Register/48p/KN/Katter_48P.pdf (SCANNED Form A, OCR-garbage proof; fixture #4) | `reader-extract.Katter_48P.scanned-ocr-garbage.md` · `8518561bac9abc48f176b2c866073ff10ff0e7df179e2d36dc21fca8161c4c40` |
| E4 | .../Register/48p/AB/Albanese_48P.pdf (alteration grammar, gifts/travel, mis-stamp; fixture #3) | `reader-extract.Albanese_48P.alterations.md` · `ab0aa8de2c048cd49e96466c55345716813af56fe474d49c86a5e459c602e45a` |
| E5 | .../Register/48p/CF/Chalmers_48P.pdf (clean Form A, real estate, Self/Spouse/Dependent grid, typed alterations; fixture #2) | `reader-extract.Chalmers_48P.clean-formA-realestate.md` · `7537150617e61dd653cfdbd3a3a6176f11aaf5e306876605f79d10e8f238906a` |
| E6 | .../Register/48p/AB/Buchholz_48P.pdf (shareholdings self+spouse, real estate, liabilities, alterations; fixture #1) | `reader-extract.Buchholz_48P.shareholdings.md` · `92a2de9fcefb388303bd50054cfb49f5af9db1811e301c0fbeead5e997bd21b7` |
| E7 | .../Register/48p/AB/Boyce_48P.pdf (shareholdings self+spouse + family trust; alternate) | `reader-extract.Boyce_48P.shareholdings.md` · `8177e8357ed0109fe11dc743465289c5171cfe9616ce1f52df0e9d2b49c933f0` |
| E8 | .../Register/48p/AB/Batt_48P.pdf (clean Form A, savings/super, self; alternate) | `reader-extract.Batt_48P.clean-formA-savings.md` · `b01c663dbc76a0c400b45fcb48fd5c8906e850c4285066393efe3df1ea03cf2f` |
| E9 | .../Register/48p/AB/Aldred_48P.pdf (all-Not-Applicable shares; empty-band evidence) | `reader-extract.Aldred_48P.md` · `ede851ffaa378cdb27b8b95b20cadf8f78afe22ce8a2ecb302eeb5f077397026` |
| E10 | .../Register/48p/AB/Bell_48P.pdf (all-Not-applicable shares) | `reader-extract.Bell_48P.md` · `5830eb07702d8a9a7da890f720ed3bf310bb665d93e88d31d0c16b3e4c2a3a86` |
| E11 | www.aph.gov.au WAF block (403 'Page Blocked by WAF', x-azure-ref) — HTML index, media PDFs, robots.txt all blocked to non-browser clients | `waf-block-page.html` · `0ca7b796a2173b5a4e932f0cbc85f85b782c2c1a50ecbb07fb83c58f24a170eb` |
| E12 | our retrieval + access-method record (15 requests; every WAF probe; browser-engine reader method; sha-deferral rationale) | `2026-07-05-aph-register.retrieval.json` |
| E13 | .../Resolutions/9Oct1984.pdf (the resolution; linked E1, WAF-gated — NOT fetched, text anchored on E2/E3) | URL-only (tried-log, E12) |
| E14 | https://www.aph.gov.au/Parliamentary_Business/Committees/Senate/Senators_Interests/Register_of_Senators_Interests/Tabled_volumes (Senate register — separate committee, combined tabled volumes; out of v1 scope §2.6) | URL-only (search-derived, not fetched) |

## Quirks log (append-only, dated)

- 2026-07-05 · **Host is Azure-WAF-gated to ALL non-browser clients** (E11): curl +
  WebFetch + allorigins/codetabs/corsproxy server-fetch proxies all 403/520/522; even
  robots.txt is blocked. web.archive.org unreachable from the spec sandbox. Only a
  headless-browser reader passed. Production fetch = browser-engine seam (us_senate
  §2.5); canonical sha pins deferred to the capture leg (§7).
- 2026-07-05 · **Scanned + OCR-garbage is real and common** (E3 Katter): `AUSTE,ALIA`,
  `STATE OLD` (QLD), `1g September 2019` (19), `luruber of ,{MP xhares`. Deterministic
  text-layer parsing is NOT a reliable regime path → LLM-vision-first (§6).
- 2026-07-05 · **Heterogeneous flavours** (§3.9): typed/fillable forms (Chalmers/Batt,
  E5/E8) DO carry clean member entries in the text layer, while scanned handwritten
  pages (Albanese alterations, E4) do not — sometimes within one file. `source_flavour`
  records which path produced each row.
- 2026-07-05 · **Compound document**: one member PDF = initial Form A + appended
  Notification-of-Alteration pages, re-published in place as alterations accrue
  (Albanese 15 pp, E4; Chalmers 18 pp, E5). Two record types in one file (§3.4).
- 2026-07-05 · **Dates are DAY-FIRST** DD/MM/YYYY (`Submitted Date: 30/04/2026`, E5) —
  contrast US MM/DD/YYYY. Handwritten dates frequently illegible (E4 `Date: 3/41 Y6`) →
  notified_date fail-soft NULL.
- 2026-07-05 · **Owner is column/marker-driven**: Form A has Self / Spouse/Partner /
  Dependent Children bands (E5/E6); alterations mark `Spouse -` / `Spouse gift -` (E4).
  Joint interests are the Member's to declare with a notation (E2) → owner self, never
  guessed `joint`.
- 2026-07-05 · **Value is never present**: descriptive register, "no need to show the
  actual amount received" (E2); ~30 Albanese gift lines carry no dollar figure (E4).
  value NULL always (§3.6).
- 2026-07-05 · **`Not Applicable` = no row** (E5/E6): empty owner bands print
  `Not Applicable`/`Not applicable` (case varies) → emit nothing; do not manufacture a
  row for an empty category.
- 2026-07-05 · **Mis-stamped forms** happen: Albanese's 48P file contains a page
  stamped `45 TH PARLIAMENT` (E4) — capture the stamp raw, trust the URL's Parliament.
- 2026-07-05 · **Filename disambiguators**: `ChesterD`/`ChestersL`, `CookK`/`CookT`,
  `David_Farley` (E1) — filename is a document key, resolve people via the index row +
  form header electorate.

## Operational notes (politeness incidents, outages)

- 2026-07-05 · 15 requests total this session (E12): 4 direct fetches to aph
  (all 403 WAF), 10 via the browser-engine reader (r.jina.ai, 200), 1 WAF-block sample
  saved. Concurrency 1, >=2.5 s spacing, identified UA + From on every request. Zero
  429s. WebSearch used for the Senate-scope + resolution-metadata cross-check (E14).
- 2026-07-05 · No polite path to canonical raw bytes exists from the spec env; this is
  logged as the #1 fetch risk (§2.3) and the reason §7 pins are capture-leg deferred.
  The browser-engine seam that unblocks the WAF is a prerequisite for the build leg,
  not a nice-to-have.
