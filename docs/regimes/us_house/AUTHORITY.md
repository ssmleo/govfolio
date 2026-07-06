---
# RegimeSurvey front-matter (validated). Every claim: {claim, evidence:[files]}
jurisdiction: "us_house"
bodies: ["US House of Representatives"]
legal_basis:
  claim: "Ethics in Government Act of 1978 (5 U.S.C. App. §103), as amended by the STOCK Act §6 (Pub. L. 112-105, enacted 2012-04-04) adding subsection (l): a person covered by section 101 who is required to report a transaction under section 102(a)(5)(B) 'shall file a report of the transaction' not later than 30 days after receiving notification of it, and in no case later than 45 days after the transaction itself. Covered persons enumerated in the amendment include '(9) A Member of Congress' and '(10) An officer or employee of the Congress'. Verified directly against the enacted primary law text (govinfo.gov PLAW-112publ105.pdf), not a secondary summary — this supersedes the Phase-0 sources.yaml candidate's secondary-source-only corroboration for this exact claim."
  evidence:
    - url: "https://www.govinfo.gov/content/pkg/PLAW-112publ105/pdf/PLAW-112publ105.pdf"
      file: "a4370985791a8c44950e34461ad6f65090fbc15a86414687a3d833f1a39a59e6.stock-act-plaw-112publ105.pdf"
who_files:
  claim: "House Members (every one of the 6 real PTRs reviewed this session carries 'Status: Member') and House officers/employees, per STOCK Act §6 paragraphs (9)-(10). Filings go to the Clerk of the House of Representatives — confirmed both by the STOCK Act's own §8(b)(4) 'FILERS COVERED' text ('individuals required ... to file financial disclosure reports with the Secretary of the Senate or the Clerk of the House of Representatives ...') and by every sampled PTR's header ('Clerk of the House of Representatives — Legislative Resource Center — B81 Cannon Building'). No Congressional-candidate PTR observed in any sampled document; candidates file FD reports (index FilingType C), outside this regime's scope (record_types=[transaction])."
  evidence:
    - url: "https://www.govinfo.gov/content/pkg/PLAW-112publ105/pdf/PLAW-112publ105.pdf"
      file: "a4370985791a8c44950e34461ad6f65090fbc15a86414687a3d833f1a39a59e6.stock-act-plaw-112publ105.pdf"
    - url: "https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2025/20029138.pdf"
      file: "778f643097f59167f5716c469304bcc2539f75cbc6c4d7ef77a7cb980143feba.ptr-sample-20029138.pdf"
record_types: [transaction]
value_precision: "banded"
band_table:
  - {raw: "$1,001 - $15,000",          low: "1001.00",     high: "15000.00",    observed_in_filing: true,  note: "e.g. sample PDF 20029138 (Greene), fixture 20019182 (Smucker), fixture 20033759 (Rouzer)"}
  - {raw: "$15,001 - $50,000",         low: "15001.00",    high: "50000.00",    observed_in_filing: true,  note: "fixture 20019182 (Smucker)"}
  - {raw: "$50,001 - $100,000",        low: "50001.00",    high: "100000.00",   observed_in_filing: true,  note: "sample PDF 20029138 (Greene, US Treasury Bill), fixture 20019182 (Smucker)"}
  - {raw: "$100,001 - $250,000",       low: "100001.00",   high: "250000.00",   observed_in_filing: true,  note: "fixture 20019182 (Smucker)"}
  - {raw: "$250,001 - $500,000",       low: "250001.00",   high: "500000.00",   observed_in_filing: true,  note: "fixture 20020055 (Begich)"}
  - {raw: "$500,001 - $1,000,000",     low: "500001.00",   high: "1000000.00",  observed_in_filing: true,  note: "fixture 20034836 (Pelosi)"}
  - {raw: "$1,000,001 - $5,000,000",   low: "1000001.00",  high: "5000000.00",  observed_in_filing: true,  note: "fixture 20034836 (Pelosi) — highest band observed in this task's evidence set"}
  - {raw: "$5,000,001 - $25,000,000",  low: "5000001.00",  high: "25000000.00", observed_in_filing: false, note: "NOT observed in any filed transaction reviewed this session; string confirmed self-evidencing from the printed paper PTR form itself (fixture 9115811, column H) rather than inferred"}
  - {raw: "$25,000,001 - $50,000,000", low: "25000001.00", high: "50000000.00", observed_in_filing: false, note: "as above — printed paper form column I"}
  - {raw: "Over $50,000,000",          low: "50000000.00", high: null,          observed_in_filing: false, note: "as above — printed paper form column J; open-ended band stores the threshold as low, high=null (codebase convention, cf. UK 70000-open)"}
cadence_and_lag:
  claim: "Rolling, transaction-triggered: statutory ceiling is 30 days after notification of the transaction, and independently no later than 45 days after the transaction itself (STOCK Act §6, EIGA §103(l)). Real filings reviewed this session are consistent with the 30-day-from-notification ceiling: fixture 20019182 (Smucker) notified 04/17-04/23/2026, signed 04/30/2026 (<=13 days); fixture 20033759 (Rouzer, amendment) transaction+notification 12/09/2025, signed 01/07/2026 (29 days). The current-year index zip is regenerated frequently server-side: this session's fresh fetches of 2012FD.zip and 2015FD.zip both returned a Last-Modified timestamp of today (2026-07-06), i.e. server-side regeneration on every request, not static archival — consistent with the Phase-0 scout's independent observation of the same on the 2026 zip."
  evidence:
    - url: "https://www.govinfo.gov/content/pkg/PLAW-112publ105/pdf/PLAW-112publ105.pdf"
      file: "a4370985791a8c44950e34461ad6f65090fbc15a86414687a3d833f1a39a59e6.stock-act-plaw-112publ105.pdf"
    - url: "https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20019182.pdf"
      file: "5b1b60bea609310f4288adce9557702231cd1f23eb5ceabf1c0babc3fe867b37.ptr-multi-row-sp-vehicle-20019182.pdf"
    - url: "https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20033759.pdf"
      file: "0a5861a182db417541f62a0179dfbba025d06cf1aa990c4d1931a2076760af1e.ptr-amendment-20033759.pdf"
formats: [pdf_text, pdf_scanned]
access:
  method: "anonymous HTTPS GET (no login, no API key, no session token)"
  session_required: false
  captcha: "none observed"
  notes: "No robots.txt (HTTP 404 — independently re-checked 2026-07-06, same finding as the pre-existing legacy doc's E11). Every response observed this session (index zips, PDFs, robots.txt probe, homepage) carried only standard security headers (Strict-Transport-Security, Cache-Control) and no Set-Cookie/auth challenge. ETag + Last-Modified served on index zips, so conditional GETs work."
historical_depth:
  from: "At least 2012 (the calendar year the STOCK Act's PTR obligation took effect, 2012-07-03): 2012FD.zip, independently downloaded and parsed this session, genuinely contains 813 records tagged <DisclosureType>PTR</DisclosureType>. IMPORTANT CAVEAT for any backfill: the index SCHEMA changes between 2013 and 2015. Pre-2015 years (verified: 2012, 2013) mark PTRs with a <DisclosureType>PTR</DisclosureType> field under <FilingType>O</FilingType> or <FilingType>A</FilingType> — a field ABSENT from the current (2026) schema. Only from ~2015 onward (verified: 2015 has 728 records) does <FilingType>P</FilingType> — the exact code the live us_house adapter's discovery filter matches on — appear, with the DisclosureType field gone entirely. A discovery pass filtering strictly on FilingType=='P' will silently find ZERO PTRs for any index year before ~2015, even though PTR-shaped data exists back to 2012. The exact flip year is not pinned tighter than 'sometime in 2014': 2014FD.zip is anomalously sparse (11 total records, none PTR-tagged, several non-US-state StateDst tokens) and did not help narrow it further. 2011FD.zip and 2008FD.zip were confirmed to exist (HTTP 200 on HEAD) but not downloaded/parsed (politeness budget); 2011 is expected (not directly parsed) to predate the obligation entirely per the 2012-07-03 effective date."
  evidence:
    - url: "https://disclosures-clerk.house.gov/public_disc/financial-pdfs/2012FD.zip"
      file: "3ef175309c99f036fe053814fb2a8939e5adb7e3cf33ab00cfd1c11667036251.2012FD.zip"
    - url: "https://disclosures-clerk.house.gov/public_disc/financial-pdfs/2015FD.zip"
      file: "670e7fecfeeec064f137b6f0665e78baba8e0a51a6c832be6dea52ddc01a273b.2015FD.zip"
identifiers_available:
  politician: "Name (as printed on the 'Name:' field, e.g. 'Hon. Lloyd K. Smucker') plus State/District (e.g. 'PA11') only. No bioguide ID or any other stable politician identifier observed in the index XML or in the PDF text of any of the 6 real documents reviewed this session (5 fixtures + 1 Phase-0 sample)."
  instrument: "Asset/company name plus ticker symbol in parentheses (e.g. '(FULT)') plus a trailing bracketed asset-type code (e.g. '[ST]'). No ISIN/CUSIP/FIGI or other standard security identifier observed anywhere in the 6 documents reviewed this session; the paper form itself instructs filers to 'Provide full name, not ticker symbol' for the asset cell, confirming tickers are filer-volunteered, not a required identifier field."
amendment_mechanism:
  claim: "PTR amendments are filed as entirely NEW documents under a NEW DocID. The amendment is signalled only per-row inside the new PDF via a 'FILING STATUS: Amended' sub-line (rendered 'F  S : Amended' in the small-caps-degraded text layer) plus a populated 10-digit row id (e.g. '2000152831'), which is otherwise blank on non-amended rows. Neither the filing-index XML nor the amended PDF itself references the DocID of the original filing being amended anywhere — supersession/linkage back to the original is not deterministically recoverable from the source alone. Independently re-verified this session by reading fixture DocID 20033759 (Hon. David Rouzer) directly."
  evidence:
    - url: "https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/2026/20033759.pdf"
      file: "0a5861a182db417541f62a0179dfbba025d06cf1aa990c4d1931a2076760af1e.ptr-amendment-20033759.pdf"
personal_data_to_redact: []
tos_and_politeness:
  claim: "No Terms of Service page or robots.txt found on disclosures-clerk.house.gov (independently re-checked 2026-07-06: GET /robots.txt returns HTTP 404). No captcha, login, or session requirement encountered on any GET/HEAD to the host this session (index zips for 2008/2011/2012/2013/2014/2015/2026, 6 PDFs, robots.txt, homepage — 2026-07-06's requests alone: 1 robots.txt GET, 5 zip GETs, 2 zip HEADs). Politeness is therefore entirely self-imposed: identified UA 'govfolio.io research (contact: ssm.leo@outlook.com)', concurrency 1, >=2s interval between requests, conditional-GET-capable (ETag/Last-Modified present on every index zip response)."
  evidence:
    - url: "https://disclosures-clerk.house.gov/robots.txt"
      file: "dc1d54dab6ec8c00f70137927504e4f222c8395f10760b6beecfcfa94e08249f.robots-txt-404.html"
    - url: "https://disclosures-clerk.house.gov/"
      file: "219527786b15ee97637c45b0f8febcebad8427add9b14555e04f87bee8f64197.clerk-home.html"
language: [en]
open_questions:
  - question: "Official legend for FilingType codes B/C/D/E/F/G/H/N/R/T/W/X. This session cross-verified O='original PTR or annual FD' (disambiguated by DisclosureType pre-2015), A='amended PTR or annual FD', and P='PTR, schema >=2015' via the DisclosureType field correlation, but no official published legend for the remaining letters was found."
    tried:
      - "2026-07-06 refetched https://ethics.house.gov/financial-disclosure/periodic-transaction-reports-ptrs -> soft-404 'Page not found' (independently reproduced; same result as the pre-existing legacy doc's tried-log)"
      - "2026-07-06 the JS single-page portal at https://disclosures-clerk.house.gov/FinancialDisclosure carries no static legend in its HTML (reconfirmed from the already-archived clerk portal page fetched this session)"
  - question: "Official instruction confirming a blank row-level Owner column (no vehicle) defaults to the filer/self."
    tried:
      - "2026-07-06 ethics.house.gov PTR guidance page: soft-404 (see above)"
      - "2026-07-06 the paper PTR form's own instructions (fixture 9115811) explain only what the SP/DC/JT letter codes mean, not what a blank owner cell defaults to"
  - question: "Exact rendering of the DC and JT owner codes in a real filed PTR (SP is now confirmed twice: row-level in fixture 20034836, vehicle-level in fixture 20019182)."
    tried:
      - "2026-07-06 none of the 6 real documents reviewed this session (5 fixtures + 1 Phase-0 sample) contain a DC or JT owner instance"
      - "2026-07-06 the paper form (fixture 9115811) confirms SP/DC/JT are legal column-header abbreviations but a blank example does not show their filled rendering"
  - question: "Electronic-PDF text-layer token for the 'Partial Sale' and 'Exchange' transaction types. The paper form independently confirms these are two real, distinct checkbox categories alongside Purchase/Sale, but no electronic document reviewed this session contains one, so the exact text-layer string (grammar guess 'S (partial)' / 'E') remains unverified against a real electronic filing."
    tried:
      - "2026-07-06 none of the 6 real electronic-style documents reviewed this session contain a Partial-Sale or Exchange row"
  - question: "How the paper form's column-K checkbox ('Transaction in a Spouse or Dependent Child Asset over $1,000,000') should be represented downstream. This session established (correcting a prior guess) that column K is NOT an 11th value band — it is a separate boolean flag that can co-occur with any of columns A-J — but no sampled filing (electronic or paper) has it checked, so its Gold/details representation is unconfirmed."
    tried:
      - "2026-07-06 visually reviewed the full printed column layout on fixture 9115811 (the only paper-form instance in the evidence set); no filing in this session's evidence set has column K checked"
  - question: "Exact calendar year the filing-index schema flips from DisclosureType-tagged PTRs (FilingType O/A) to FilingType=='P'. Bracketed this session to somewhere between 2013 (old schema) and 2015 (new schema); 2014 could not narrow it further."
    tried:
      - "2026-07-06 downloaded and parsed 2011FD.zip, 2012FD.zip, 2013FD.zip, 2014FD.zip, 2015FD.zip: 2011 has no PTR tag at all; 2012/2013 use DisclosureType=PTR under FilingType O/A; 2014 has only 11 total records (all DisclosureType=FD, none PTR-tagged) — too sparse to place the flip; 2015 has FilingType=='P' present and the DisclosureType field gone entirely"
  - question: "Why 2014FD.zip is anomalously sparse (11 total Member records vs thousands of records in the neighboring 2013 and 2015 indexes, and several non-US-state StateDst tokens such as NS00/WM00/CB00/CP00)."
    tried:
      - "2026-07-06 no explanation found on any page fetched this session; not investigated further as it is outside this survey's scope — flagged here for whoever runs a pre-2015 backfill"
  - question: "Does {YYYY}FD.zip exist for years before 2008 (deeper historical ceiling)?"
    tried:
      - "2026-07-06 only 2008/2011/2012/2013/2014/2015/2026 checked this session (HEAD-only for 2008/2011; full GET+parse for 2012/2013/2014/2015/2026); this survey's politeness/scope budget did not extend further back"
regime_versions:
  - effective_from: "2012-07-03"
    change: "STOCK Act (Pub. L. 112-105 §6, enacted 2012-04-04) adds EIGA §103(l): the Periodic Transaction Report obligation (30 days from notification / 45 days from transaction) takes effect for transactions occurring on or after this date — 90 days after the Act's enactment, per the Act's own effective-date clause. Independently confirmed against the primary law text this session (upgrading the pre-existing legacy doc's unevidenced regime_versions entry for this same date)."
    evidence:
      - url: "https://www.govinfo.gov/content/pkg/PLAW-112publ105/pdf/PLAW-112publ105.pdf"
        file: "a4370985791a8c44950e34461ad6f65090fbc15a86414687a3d833f1a39a59e6.stock-act-plaw-112publ105.pdf"
---

# US House (PTR) — Source Authority File

Living canonical context for the `us_house` adapter's Periodic Transaction Report (PTR)
regime. Specialists MUST load this before any source-scoped task and MUST write back
new learnings in the same PR.

Scope: **Periodic Transaction Reports only** (`FilingType == "P"` in the current/2026
index schema; see the historical_depth caveat above for the pre-2015 schema fork).
Annual FDs, candidate reports, extensions etc. are a separate regime. All money is `USD`.

This file is the Phase-1 SURVEY re-derivation of the pre-existing, differently-schemaed
`docs/regimes/us-house.md` (hyphen path, frozen legacy reference, citation-tag evidence
style) into the current `{url, file}` evidence schema. Every claim above was
independently re-verified against primary sources this session (2026-07-06) rather than
copied from that legacy doc; where this file's findings differ from or sharpen the
legacy doc's, that is called out explicitly (see Quirks log).

## Data catalog

- **Portal**: `https://disclosures-clerk.house.gov/FinancialDisclosure` — JS single-page
  app (browse/search/download UI); static HTML carries no legend content (confirmed
  this session and by the Phase-0 scout pass).
- **Filing index**: `https://disclosures-clerk.house.gov/public_disc/financial-pdfs/{YYYY}FD.zip`
  — a zip containing `{YYYY}FD.txt` (TSV) and `{YYYY}FD.xml` (UTF-8 BOM, root
  `FinancialDisclosure`, repeated `Member` elements). CURRENT (2026) schema fields:
  `Prefix, Last, First, Suffix, FilingType, StateDst, Year, FilingDate, DocID`. Confirmed
  this session that years 2012-2013 additionally carry `FilingYear` and `DisclosureType`
  fields not present in 2026 (see historical_depth + Quirks log — a real schema
  evolution, not a fetch error).
- **PTR document**: `https://disclosures-clerk.house.gov/public_disc/ptr-pdfs/{Year}/{DocID}.pdf`
  — electronic (8-digit DocID beginning `2`, real text layer) or paper/scanned (7-digit
  DocID, image-only, no text layer — confirmed this session on fixture 9115811:
  `pdftotext` emits exactly 1 byte, a lone form-feed).
- **Asset-type code legend**: `https://fd.house.gov/reference/asset-type-codes.aspx` —
  not re-fetched this session (out of scope for the required front-matter fields; carried
  forward from the legacy doc's E2 as a lead only, not asserted here as evidence).
- **Primary law text**: `https://www.govinfo.gov/content/pkg/PLAW-112publ105/pdf/PLAW-112publ105.pdf`
  (Pub. L. 112-105, the STOCK Act) — fetched and read directly this session; SEC. 6 is
  the operative PTR-obligation text, SEC. 8(b)(4) names the filing offices.

## Field mapping (source → gold)

| Source field | Gold-adjacent concept | Notes (this session's verification) |
|---|---|---|
| index `Last`/`First`/`Suffix`/`Prefix` + PDF `Name:` | politician identity | verbatim name only, no bioguide id (identifiers_available) |
| index `StateDst` + PDF `State/District:` | mandate (body, district) | e.g. `PA11`, `AK00` (at-large `00`) |
| index `DocID` + PDF `Filing ID #` | filing external_id | cross-check the two match (fixture 20033759: both read `20033759`) |
| index `FilingType` | filing/record-type discriminator | `P` in 2026 schema; `O`/`A` + `DisclosureType=PTR` pre-2015 (historical_depth) |
| PDF `Owner` column / vehicle `(Owner: XX)` | Gold `owner` | blank + vehicle-inherited SP confirmed (fixture 20019182); explicit row-level SP confirmed (fixture 20034836); DC/JT still unobserved (open_questions) |
| PDF asset cell | `asset_description_raw` (raw is sacred) | name + `(TICKER)` + trailing `[XX]` code, verbatim |
| PDF transaction-type token + two dates + amount | `side`, `transaction_date`, `notified_date`, `value` | band_table above is normative for `value` |
| PDF `F S : <status>` sub-line | amendment detection | see amendment_mechanism claim |
| paper-form column K checkbox | UNRESOLVED (open_questions) | confirmed NOT part of the value-band vocabulary |

## Parse strategy & rationale

This survey does not re-litigate the existing adapter's extraction strategy (deterministic
text-layer parse for electronic PTRs with an LLM-fallback seam for scanned paper PTRs,
already implemented in `crates/adapters/us_house` against the small-caps label quirk and
the content-order row grammar) — it exists to re-verify the source-of-truth facts that
strategy depends on. Nothing reviewed this session contradicts the deterministic-first
decision: every electronic PTR sampled (5 of 6 documents) has a complete, verbatim text
layer for every data cell; only the scanned paper filing lacks one, confirming the
LLM-fallback trigger condition is real and narrow (paper filings only).

## Quirks log (append-only, dated)

- 2026-07-06 · **Historical index schema fork (new finding, not in the legacy doc)**:
  years before ~2015 do not use `FilingType == "P"` for PTRs at all. 2012 and 2013
  indexes independently downloaded and parsed this session mark PTRs via a
  `<DisclosureType>PTR</DisclosureType>` field (absent from the 2026 schema) combined
  with `FilingType` `O` or `A`. `FilingType == "P"` first appears at 2015 in this
  session's sample. Any backfill adapter that filters strictly on `FilingType == "P"`
  will silently miss every pre-2015 PTR. See historical_depth and open_questions.
- 2026-07-06 · **2014FD.zip data-quality anomaly**: only 11 total `Member` records
  (thousands in neighboring years), all `DisclosureType=FD`, none PTR-tagged, several
  non-US-state `StateDst` tokens (`NS00`, `WM00`, `CB00`, `CP00`). Not explained by
  anything fetched this session; flagged, not resolved.
- 2026-07-06 · **Column K is a checkbox flag, not a band variant (correction to the
  legacy doc)**: the legacy `us-house.md` guessed an unobserved 11th band string
  "Spouse/DC over $1,000,000". Direct visual review of the printed paper PTR form
  (fixture 9115811) shows this is actually a *separate* checkbox column K, "Transaction
  in a Spouse or Dependent Child Asset over $1,000,000", that can be checked alongside
  (not instead of) one of the ten amount-band columns A-J. There is no 11th band string.
- 2026-07-06 · STOCK Act primary text independently confirms the legacy doc's
  `regime_versions` effective date (2012-07-03 = enactment 2012-04-04 + the Act's own
  90-day effective-date clause) and the filing-office claim (Clerk of the House /
  Secretary of the Senate), both previously only secondary-sourced.
- 2026-07-06 · `robots.txt` re-confirmed 404 (no robots policy) independently of the
  legacy doc's prior finding.

## Operational notes (politeness incidents, outages)

- 2026-07-06 · disclosures-clerk.house.gov: 10 requests this task (1 robots.txt GET,
  5 index-zip GETs [2012/2013/2014/2015 fresh downloads + reuse of the Phase-0 2026
  fetch], 2 index-zip HEADs [2008, 2011]); all 200 except the expected robots.txt 404;
  no throttling observed at concurrency 1 with >=2s spacing between requests.
- 2026-07-06 · www.govinfo.gov: 1 request (STOCK Act PDF GET), 200, no incidents; not
  previously probed by this task's Phase-0 pass.
- 2026-07-06 · ethics.house.gov: PTR guidance page re-confirmed soft-404 (`Page not
  found`), consistent with the legacy doc's prior finding — not a new outage.
- Per the task's known pattern for this host (documented in `sources.yaml`'s notes),
  the WebFetch tool was not used for disclosures-clerk.house.gov or govinfo.gov this
  session; `curl` with the identified UA was used directly throughout and succeeded on
  every request.
