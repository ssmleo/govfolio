---
# RegimeSurvey front-matter (validated shape). Every claim: {claim, evidence:[files]}
jurisdiction: "gb"
bodies: ["UK House of Commons"]
legal_basis:
  claim: "House of Commons Code of Conduct + Guide to the Rules relating to the Conduct of Members (registration categories 1-10 with 1.1/1.2 sub-categories). Rules text NOT yet archived (publications.parliament.uk is Cloudflare-gated, E23 — open question); the current category scheme and thresholds are anchored on the official API's own data: category names/numbers (E2) and the printed threshold strings, e.g. '(ii) Other shareholdings, valued at more than £70,000' (E5)."
  evidence: ["E2 categories.json", "E5 interests-cat7-shareholdings.json", "E23 publications-cmregmem-cloudflare-403.html"]
who_files:
  claim: "Members of the House of Commons (member.house == 'Commons' on every sampled interest; RegisterType enum has the single value 'Commons', E1). Each interest carries the registering member's numeric Members-API id + name/constituency/party."
  evidence: ["E1 swagger-v1.json", "E5", "E6", "E7", "E12", "E14"]
record_types: [interest]
value_precision: "categorical"
band_table:
  # Not a banded regime — this table pins the two observed categorical threshold strings
  # (category 7 Shareholdings). Exact GBP amounts elsewhere ride ValueInterval low==high.
  - {raw: "(ii) Other shareholdings, valued at more than £70,000", low: "70000.00", high: null, observed: true}   # open-ended: low = stated threshold (codebase convention, gold.rs UK example)
  - {raw: "(i) Shareholdings: over 15% of issued share capital",   low: null,       high: null, observed: true}   # percentage threshold, NO monetary value → value NULL
cadence_and_lag:
  claim: "Rolling registration, rolling API publication: observed publishedDates fall BETWEEN fortnightly register dates (E16 published 2026-06-17 vs neighbouring registers 2026-06-15/2026-06-29, E3). The formal register is published fortnightly during sitting periods (observed: 06-29, 06-15, 06-01, 05-18, 04-27, 04-13, 03-23, 03-09 of 2026 — E3). Observed registration→publication lag 0-1 days on current rows (E16 0d, E17 1d, E19 3d over a weekend+register day); migrated legacy rows show years (E14: registered 2016-04-20, published 2024-07-26). The rule-book registration deadline (28 days per the Guide) is NOT archived — open question; disclosure_lag_days stays NULL until it is."
  evidence: ["E3", "E14", "E16", "E17", "E19"]
formats: [open_data_json, pdf_text]
access: {method: "anonymous HTTPS GET against a documented public API (OpenAPI 3.0.1 contract published at /swagger/v1/swagger.json; API contact softwareengineering@parliament.uk)", session_required: false, captcha: "none on interests-api; legacy publications.parliament.uk IS Cloudflare-challenged (E23)", notes: "identified UA + From header served 200 on all 24 interests-api requests (contrast us_senate). NO ETag/Last-Modified on any interests-api response → conditional GETs impossible; windowed date queries are the incremental primitive. Take capped at 20 by contract (E1)."}
historical_depth: {from: "2024-03-18 (earliest register version in the API, id 511, E4). Pre-2024 registers live on legacy publications.parliament.uk (Cloudflare-gated, E23) — backfill route is an open question", evidence: ["E4", "E23"]}
identifiers_available: {politician: "numeric Members-API (MNIS) member id on EVERY interest, plus nameDisplayAs/nameListAs/constituency/party — politician resolution is deterministic by id, no name matching (E16: member.links → members-api.parliament.uk/api/Members/4051)", instrument: "none — shareholdings name the company in free text (OrganisationName) with no ticker/ISIN/registry id; instrument_id stays NULL below threshold (invariant 3)"}
amendment_mechanism:
  claim: "Interests are MUTABLE in place: /Interests/{id} returns 'the latest version of an interest' (E1); updatedDates[] lists update dates (non-empty observed on migrated rows: E14 id 2696 ['2024-07-26','2026-06-18']); rectified/rectifiedDetails flag rectifications (e.g. late registration; true-case unobserved). No version-history endpoint found. Unlike US PTR amendments the source id is STABLE across updates, so supersession linkage is deterministic (same interest id). v1 fail-closed handling in §3.7: version-qualified filings ({id}@{len(updatedDates)}), new Gold insert + review_task per updated version; deterministic supersession wiring is the promotion machinery's job."
  evidence: ["E1", "E14"]
personal_data_to_redact: ["Third-party personal data is published deliberately in the official register and API: family-member names + relationship (categories 9/10, E14/E15), private-individual donor names (DonorStatus 'Individual', E6), employee names. Keep verbatim in Bronze/Silver/details; whether govfolio's PUBLIC rendering surfaces or search-indexes non-politician individuals is a product/legal decision — flagged, default to not indexing non-politician names."]
tos_and_politeness:
  claim: "Public API published by UK Parliament for exactly this purpose ('API to allow users to query and download the register of Members interests', E1). No robots.txt on interests-api or members-api (404, E24). No ToS gate encountered. Politeness: concurrency 1, ≥2s interval, identified UA 'govfolio.io research (contact: ssm.leo@outlook.com)' + From header — 27 requests this task, zero 429s, zero challenges on interests-api."
  evidence: ["E1", "E24 retrieval log"]
language: [en]
open_questions:
  - {question: "Archive the Guide to the Rules (28-day registration deadline, category thresholds, loan rules) — legal_basis + disclosure_lag_days anchor", tried: ["publications.parliament.uk Cloudflare-challenged 403 'Just a moment...' under the identified non-browser client (E23); one probe, no evasion per polite-fetching skill"]}
  - {question: "ShareholdingThreshold full vocabulary — only variants (i) and (ii) observed", tried: ["201 category-7 interests exist; 5 sampled (E5) + fixture E16 show only the two strings; unknown string ⇒ freeze + review_task (§3.4)"]}
  - {question: "HeldOnBehalfOf / ManagedBy populated variants (category 7) — all null in samples; owner grammar accepts nothing yet", tried: ["6 category-7 rows sampled (E5, E16), all null"]}
  - {question: "Cross-session byte stability of /Interests/{id} responses (single research session so far)", tried: ["within-session re-GETs of 15475 and 15854 byte-identical 2/2 (E24 variance tests); normalized-hash fallback defined in §7"]}
  - {question: "Deletion semantics: members-api exposes deletedWhen (E21); interests-api has no deleted flag — does a deleted interest 404 or vanish from sweeps?", tried: ["no known-deleted interest id to probe; not guessable politely"]}
  - {question: "Non-GBP currencyCode existence (only GBP observed across every Decimal money field)", tried: ["all sampled categories with money fields show currencyCode GBP (E6-E11, E13, E17, E20)"]}
  - {question: "rectified=true rendering of rectifiedDetails", tried: ["0 rectified rows in all samples; grammar keeps both fields verbatim so nothing is lost"]}
  - {question: "Historical reconstruction via RegisterId param (interests as published in older registers) — backfill within the API era", tried: ["deliberately not probed (request budget); RegisterId documented in E1"]}
  - {question: "Pre-2024 backfill route (legacy HTML/PDF registers on publications.parliament.uk)", tried: ["Cloudflare 403 under this client (E23); browser-engine fetch seam or another bulk source — future goal"]}
  - {question: "Category 2 loan-specific fields ('including loans' in the category name) — loan rows unobserved", tried: ["5 donation rows sampled (E6), all plain donations; /Interests/csv?IncludeFieldDescriptions=true would enumerate per-category field lists (E1) — not fetched, request budget"]}
  - {question: "PaymentDescription empty-string vs null (both observed, E6 ids 15928 vs 15899) — one sentinel or two?", tried: ["kept verbatim in Silver fields_raw; details normalizes '' → null with the raw untouched (§4)"]}
regime_versions:
  - {effective_from: "2024-03-18", change: "New register system + category scheme (1-10 with 1.1/1.2 sub-categories) — earliest register version served by the API (id 511, E4); rules-text archive pending", evidence: ["E4"]}
---

# UK House of Commons — Register of Members' Financial Interests — Source Authority File

> **Internal context; the public methodology page derivation requires founder review
> (residual human lane: methodology PUBLIC copy).** Goal 061 leg A (spec). Written
> BEFORE any adapter code, per the adapter template (design §5.1, plan Task 8) and the
> us_house/us_senate pattern (`docs/regimes/us-house.md`, `docs/regimes/us_senate.md`).

Scope: the **Register of Members' Financial Interests** (Commons) via the official
**Register of Interests API** — govfolio's first non-transaction adapter: every row is
`record_type = interest` (design §4.2), `side` NULL, `notified_date` carries the
registration date, values are exact-or-open-ended-or-absent (§3.4). Lords' register,
APPG registers, and ministerial transparency returns are separate regimes/goals.
All money observed is `GBP` (per-field `currencyCode`, §3.2).

Evidence citations `E1..E24` refer to §8. All retrievals 2026-07-05, identified UA
`govfolio.io research (contact: ssm.leo@outlook.com)` + `From:` header, concurrency 1,
≥2 s spacing (E24). Everything is archived under
`docs/regimes/uk_commons_register/evidence/` **in this same commit**.

Per automation policy (`docs/decisions/automation-policy.md`), the goal's "HUMAN
completes expected.*.json" step is superseded: the test-designer authors expecteds
independently (high-confidence extraction + second-model cross-check), records publish
`unverified`, sampling-audit queue.

## 1. Regime metadata

| Field | Value |
|---|---|
| jurisdiction | `gb` (national) |
| body | `UK House of Commons` |
| regime_type | `change_notification` — the obligation is event-triggered (register a new/changed interest as it arises; rolling API publication), not a periodic snapshot; design §5.5 Tier 2 names "Change-notification registers (UK, …)" explicitly |
| value_precision | `categorical` — registrability is category/threshold-driven; per-record value semantics are mixed (exact GBP decimals for payments/donations/gifts/visits; open-ended £70,000 threshold for shareholdings (ii); NO value for land/misc/family categories). Exact amounts still ride `ValueInterval` `low == high`; the regime-level label describes the register's value discipline, which is categorical. §3.4 is normative per-category |
| cadence | rolling registration + rolling API publication; formal register republished fortnightly in sitting periods (E3) |
| disclosure_lag_days | NULL — the 28-day rule is not yet archived (open question); never encode an unverified constant |
| source_url | https://interests-api.parliament.uk/ |
| API contract | https://interests-api.parliament.uk/swagger/v1/swagger.json (OpenAPI 3.0.1, E1) |
| interest URL | `https://interests-api.parliament.uk/api/v1/Interests/{id}` (canonical Bronze document, §2.3) |
| register PDF | `https://interests-api.parliament.uk/api/v1/Registers/{id}/document?type=Full|Updated` (E22) |
| currency | GBP (only value observed; per-field `currencyCode`, §3.4 R1) |
| cadence tier | 2 (design §5.5): discover hourly–daily; latency target same day |

## 2. Discovery

**Data-source decision: the Register of Interests API** (interests-api.parliament.uk).
Three routes exist; evidence-ranked:

1. **interests-api** (CHOSEN): documented OpenAPI contract (E1), fully structured
   per-category `fields` with typed money (`Decimal` + `currencyCode`, §3.2), stable
   numeric interest ids, member ids on every row, register-version history to
   2024-03-18 (E4). Deterministic parse, no scraping.
2. **members-api** `/api/Members/{id}/RegisteredInterests` (REJECTED for parsing;
   E21): same underlying data rendered as flat prose blobs (`interest` = multi-line
   free text) — would require text parsing the chosen route makes unnecessary. Serves
   `Last-Modified` (the only route that does). Useful cross-check only.
3. **publications.parliament.uk** legacy HTML register (REJECTED; E23):
   Cloudflare-challenged for non-browser clients; pre-2024 backfill seam only.

### 2.1 API surface (verbatim from E1)

| Endpoint | Purpose | Key params |
|---|---|---|
| `GET /api/v1/Categories` | category scheme, sorted by category number | `Skip`, `Take` |
| `GET /api/v1/Categories/{id}` | one category | |
| `GET /api/v1/Interests` | search published interests | `MemberId`, `CategoryId`, `PublishedFrom/To`, `RegisteredFrom/To`, `UpdatedFrom/To`, `RegisterId` (**default = latest register**), `ExpandChildInterests` (default false → children appear as individual items), `SortOrder` (`PublishingDateDescending` \| `CategoryAscending`), `Skip`, `Take` (**"default is 20. Maximum is 20"**) |
| `GET /api/v1/Interests/{id}` | **"the latest version of an interest which has been published"** — the canonical per-interest document | |
| `GET /api/v1/Interests/csv` | ZIP of CSVs (bulk); `IncludeFieldDescriptions` emits per-category field metadata | same filters |
| `GET /api/v1/Registers` | published register versions (47 total; earliest 2024-03-18, E4) | `SessionId`, `Skip`, `Take` |
| `GET /api/v1/Registers/{id}/document` | official register PDF, `type=Full\|Updated` (E22: Updated for register 803 = 244 KB `%PDF-1.6`) | |

List responses are `{links[], skip, take, totalResults, items[]}` with HATEOAS
`nextPage` links (E5). No response carries `ETag` or `Last-Modified` (E24).

### 2.2 Pagination contract

`Take` ≤ 20 (server contract, E1). Walk `Skip += 20` until `Skip ≥ totalResults`,
≥2 s between pages. `totalResults` is recomputed per page — a changed value mid-walk
means the window moved under us; restart that sweep (idempotent writes make the
re-walk free). Register ids are NOT contiguous (…799, 800, 801, **803** — no 802;
E3): treat register id as opaque, order by `publishedDate`.

### 2.3 Discover algorithm + politeness

1. Two windowed sweeps of `GET /api/v1/Interests`, both `SortOrder=PublishingDateDescending`:
   (a) `PublishedFrom = hwm_published − 30d` (new interests), (b) `UpdatedFrom =
   hwm_updated − 30d` (in-place updates, §3.7). The 30-day overlap is free
   (idempotency); interests publish ROLLING, between fortnightly register dates
   (front-matter cadence claim), so sweeps poll at tier-2 cadence (hourly–daily), not
   on register days.
2. Emit `FilingRef` per item: `external_id = "{id}@{version}"` where
   `version = updatedDates.length` (§2.5), interest id, member id, category id,
   `registrationDate`, `publishedDate`, `updatedDates`. New filing ⇔ unseen
   `(regime_id, external_id)` — an in-place update changes `version` and therefore
   arrives as a new filing. Idempotent: `ON CONFLICT DO NOTHING`.
3. `fetch`: `GET /api/v1/Interests/{id}` once per new `external_id` → store the raw
   response bytes as the Bronze document (sha256-addressed, invariant 2). One Bronze
   doc = one interest version. Child interests (`parentInterestId` set) are their own
   documents — the list sweep returns them as individual items (E8).
4. Optionally daily: `GET /api/v1/Registers?Take=1`; on a new register id, archive the
   official PDF (`type=Updated`) to Bronze as the formal snapshot document (audit
   trail, not a parse input).
5. **No conditional GETs exist** (no validators served, E24; contrast us_house). The
   date-windowed sweep is the cheap incremental check.
6. Politeness (invariant 10): concurrency 1; ≥2 s min interval; exponential backoff on
   429/5xx; identified UA + `From:` header (served without challenge here — the
   us_senate §2.5 fingerprint problem does NOT exist on this host, E24); no robots.txt
   (404, E24) so these self-imposed limits govern.

### 2.4 Politician resolution

Every interest carries `member.id` — the numeric MNIS id shared with
members-api.parliament.uk (E16: `member.links[0].href =
https://members-api.parliament.uk/api/Members/4051`). Resolution is therefore an
**exact id join**, not name matching: rosters seed from members-api with the MNIS id
stored as a politician external id (roster seeding is the builder/test leg's concern,
mirroring us_house Task 9). `nameDisplayAs`/`nameListAs`/`memberFrom`/`party` are
stored raw in Silver for audit and alias enrichment. Zero or multiple roster hits for
a member id ⇒ fail closed — `review_task reason = "unresolved_filer"` (target
`uk_commons_register:{id}@{version}`), no filing row, no Gold rows (invariant 3).

### 2.5 Filing model & version keys (update-safe by construction)

The source mutates interests in place (§3.7) but our Gold is immutable (invariant 1).
Bridge: **one filing per interest VERSION.**

- `filing.external_id = "{interest_id}@{version}"`, `version = updatedDates.length`
  at fetch time (0 = original). Deterministic from source data; multiple source
  updates between polls collapse into the version we actually fetched (we can only
  ever archive observed states).
- `filing_type = "interest"`; `filed_date = registrationDate` (nullable, §3.6);
  `published_at = publishedDate at 00:00:00Z` — **date-precision convention** (the
  source exposes a date, not a timestamp; midnight-UTC is a documented convention, not
  fabricated precision); `discovered_at` = ours.
- `supersedes_filing_id`: deterministic lookup of `(regime_id, "{id}@{version-1}")`;
  NULL when that version was never observed (first sight at v≥1). Record-level
  supersession stays with the promotion machinery (§3.7).

## 3. Record anatomy (PublishedInterest, E1 schema + E5–E20 observations)

One JSON object per interest:

| Key | Content | Notes |
|---|---|---|
| `id` | int | source-native id, stable across updates |
| `summary` | string | display title, e.g. `Shares in Lockhouse Systems Limited` (E16), `Arab Investments Limited - £3,400.00` (E17) — **may embed a formatted value; display artifact, never parse money from it** (§3.4) |
| `parentInterestId` | int\|null | payment (child) → payer (parent) link, category 1.x (E8, E20) |
| `registrationDate` | date\|null | **null observed on migrated legacy rows** (E14 id 2704) |
| `publishedDate` | date | first publication (schema nullable; always present in samples) |
| `updatedDates` | date[] | update history; non-empty observed (E14: `["2015-06-05","2017-06-30","2024-07-23"]` — entries may PREDATE publishedDate on migrated rows) |
| `category` | object | `{id, number, name, parentCategoryIds, type:"Commons"}` — `number` is a STRING (`"1.1"`) |
| `member` | object | `{id, nameDisplayAs, nameListAs, house:"Commons", memberFrom, party}` (§2.4) |
| `fields` | Field[] | category-specific payload (§3.2) — the actual substance |
| `childInterests` | PublishedInterest[]\|absent | only when `ExpandChildInterests=true` on list queries; ABSENT on `/Interests/{id}` (E16–E20) — children are fetched as their own documents |
| `links` | Link[] | HATEOAS boilerplate (self href) — never parsed, stripped by the normalized hash (§7) |
| `rectified` | bool | rectification flag (e.g. late registration); `true` unobserved |
| `rectifiedDetails` | string\|null | reason; null unless rectified |

### 3.1 The category scheme (verified live, E2; sorted by category number)

| API id | number | name | totalResults at retrieval | money in record | Gold `asset_class` |
|---|---|---|---|---|---|
| 12 | 1 | Employment and earnings (payer entry) | 262 (E7) | none on parent | `other` |
| 1 | 1.1 | Employment and earnings - Ad hoc payments | 604 (E8) | exact `Value` GBP | `other` |
| 2 | 1.2 | Employment and earnings - Ongoing paid employment | not queried directly; child observed (E7 id 15223) | exact `Value` GBP | `other` |
| 3 | 2 | Donations and other support (including loans) for activities as an MP | 631 (E6) | exact `Value` GBP | `other` |
| 4 | 3 | Gifts, benefits and hospitality from UK sources | 668 (E9) | exact `Value` GBP | `other` |
| 5 | 4 | Visits outside the UK | 445 (E10) | per-donor `Value` GBP in `Donors[]` | `other` |
| 6 | 5 | Gifts and benefits from sources outside the UK | 19 (E11) | exact `Value` GBP | `other` |
| 7 | 6 | Land and property (within or outside the UK) | 214 (E12) | **none** (threshold booleans only) | `real_estate` |
| 8 | 7 | Shareholdings | 201 (E5) | `ShareholdingThreshold` string (§3.4 R2) | `equity` |
| 9 | 8 | Miscellaneous | 938 (E13) | none | `other` |
| 10 | 9 | Family members employed | 33 (E14) | none | `other` |
| 11 | 10 | Family members engaged in third-party lobbying | 23 (E15) | none | `other` |

Category ids 1/2 are children of 12 (`parentCategoryIds: [12]`, E2). An interest in an
UNKNOWN category id ⇒ freeze adapter + review_task (a new category is a rules change;
invariant 6). `asset_class` stays honest: only shareholdings are equity and only
land/property is real_estate; everything else is `other` — no creative bucketing.

### 3.2 Field grammar (`fields[]` entries, E1 `Field` schema)

`{name, description, type, typeInfo, value, values}` — a self-describing typed record:

| `type` | `value` JSON rendering | Observed examples |
|---|---|---|
| `String` | string \| null \| `""` (both empties observed, E6) | `OrganisationName`, `DonorName`, `ShareholdingThreshold` |
| `Decimal` **with** `typeInfo.currencyCode` | **string** decimal, e.g. `"3400.00"` (E17) | `Value` — the only money shape |
| `Decimal` **without** `currencyCode` | string decimal | `HoursWorked "5.00"` (E20) — **NOT money**; the grammar keys on `currencyCode` presence, never on the field name alone |
| `Boolean` | JSON bool | `IsSoleOwner`, `IsSoleBeneficiary`, `PaymentReceived` |
| `Int` | JSON number | `NumberOfProperties` (E12) |
| `DateOnly` | `"YYYY-MM-DD"` \| null | `ReceivedDate`, `RegistrableDate`, `StartDate` |
| `VisitLocation[]`, `Donor[]` | data rides `values`: array of array-of-Field (rows of sub-fields) | E10: `Donors[0] = [Name, IsPrivateIndividual, PublicAddress, PaymentType, PaymentDescription, Value(GBP), IsSoleBeneficiary]` |

Field NAMES vary per category and can grow over time. The whole `fields` array is
stored VERBATIM in Silver (`fields_raw`) — unknown field names are lossless and never
an error; only the load-bearing extractions below have grammars.

### 3.3 Category → Gold semantics

Everything maps to `record_type = interest` (one Gold row per interest version).
`asset_description_raw = summary` verbatim (invariant 2; empty summary ⇒ reject row).
Per-category `asset_class` per §3.1. `side`/`transaction_date`/`as_of_date` are NULL —
`GoldCandidate::validate()` requires nothing extra for `Interest`
(`crates/core/src/domain/gold.rs`, the T4 `uk_interest_fixture` is exactly this
regime's shape: notified 2026-04-10, 70000–open GBP, owner None).

### 3.4 Value → `ValueInterval` (the heart of the mapping)

Rules applied in order; first match wins; provenance recorded in
`details.value_source`. Decimals stay strings end-to-end (invariant 7).

| # | Rule | ValueInterval | `value_source` | Evidence |
|---|---|---|---|---|
| R1 | top-level `Field name="Value" type="Decimal"` with `currencyCode` (categories 1.1, 1.2, 2, 3, 5) | `low = high = value`, currency = `currencyCode` | `value_field` | E6 (`"3400.00"` GBP), E9, E11, E20 |
| R2a | category 7, `ShareholdingThreshold` = `(ii) Other shareholdings, valued at more than £70,000` | `low = 70000.00, high = NULL, GBP` — the design's UK open-ended example, verbatim | `shareholding_threshold` | E5, E16 |
| R2b | category 7, `ShareholdingThreshold` = `(i) Shareholdings: over 15% of issued share capital` | `NULL` (percentage of capital, no monetary value — inventing one would be guessing) | `none` | E5 |
| R2c | category 7, any OTHER `ShareholdingThreshold` string (or the field missing) | reject row → review_task (vocabulary outside archived grammar) | — | fail closed |
| R3 | category 4 (Visits): `Donors[]` rows each carry `Value` + `currencyCode` | single donor → that exact value; multiple donors → **sum** (deterministic arithmetic over declared exact amounts) — all donor currencies must be identical, else reject row → review_task | `sum_of_donors` | E10 (single donor `1588.83` GBP; multi-donor unobserved — flagged) |
| R4 | no money field at all (categories 1 parent, 6, 8, 9, 10) | `NULL` — the categorical/no-value case. Category 6 registrability thresholds (>£100,000 value / >£10,000 rent) are in the RULES, not the record; `RegistrableRentalIncome` etc. stay booleans in `fields_raw`, and we cannot know which threshold triggered registration — value stays NULL, never inferred | `none` | E12, E13, E14, E15 |

Never parse `£` amounts out of `summary` or `PaymentDescription` free text — typed
fields carry the money; the summary is a display artifact (E17's
`- £3,400.00` suffix duplicates the `Value` field). `currencyCode` must map into the
core `Currency` enum; an unmapped code ⇒ reject row → review_task (only GBP observed).

### 3.5 Owner

The register is the Member's own register; family/joint information is category-local:

| Condition | Gold `owner` | Evidence |
|---|---|---|
| category 7, `HeldOnBehalfOf` null | `self` | E5, E16 (all observed) |
| category 7, `HeldOnBehalfOf` non-null (`"Held jointly or on behalf of a spouse, partner, or dependent child"` per field description; value vocabulary UNOBSERVED) | `unknown` + review_task — grammar accepts nothing until archived | fail closed |
| category 6, `IsSoleOwner = true` | `self` | E12 |
| category 6, `IsSoleOwner = false` | `joint` (`PropertyOwnerDetails` raw in `fields_raw`, e.g. `Co-owned with husband`) | E12, E18 |
| categories 9, 10 | `NULL` — no asset of the Member; the subject is a third party (`PersonName`, `FamilyRelationType` ride `fields_raw`) | E14, E15 |
| all other categories | `self` (benefit/earning received by the Member; `IsSoleBeneficiary=false` means a shared benefit and stays in `fields_raw` — the registered interest is still the Member's) | E6, E9–E11 |

### 3.6 Dates

| Gold field | Rule |
|---|---|
| `notified_date` | `registrationDate` — the date the Member registered (notified) the interest. **Nullable**: legacy migrated rows carry null (E14 id 2704); `Interest` validate() requires no date, and `event_date` degrades to NULL honestly. NEVER substitute `publishedDate` (publication ≠ notification) |
| `transaction_date`, `as_of_date` | NULL always |
| filing `filed_date` | = `registrationDate` (same nullability) |
| filing `published_at` | `publishedDate` at 00:00:00Z (date-precision convention, §2.5) |
| everything else (`ReceivedDate`, `AcceptedDate`, `StartDate`, `RegistrableDate`, `AroseOn`, …) | category-specific → stays in `fields_raw`; typed extraction can be added to the details contract later WITHOUT refetching (raw is sacred) |

### 3.7 Update & rectification semantics (the amendment mechanism)

- The source updates interests IN PLACE: `/Interests/{id}` serves the latest version;
  `updatedDates[]` grows. Old versions are not addressable (no history endpoint found).
- Detection: the `UpdatedFrom` sweep (§2.3). A hit re-fetches the document; new bytes →
  new Bronze doc; `version = updatedDates.length` → new filing `{id}@{n}` (§2.5).
- Promotion (invariants 1, 3, 6): every version's row is a normal Gold insert. For
  `version ≥ 1`, `filing.supersedes_filing_id` links deterministically to
  `{id}@{n-1}` when observed (§2.5); `supersedes_record_id` stays NULL at insert and
  one `review_task reason = "uk_interest_update_unlinked"` opens per newly inserted
  updated-version record (insert-gated, idempotent — same pattern as us_house/us_senate
  `ptr_amendment_unlinked`). Record-level supersession runs through the promotion
  machinery later; the linkage is deterministic here (same interest id), so this
  review lane should burn down mechanically once that machinery lands.
- `rectified`/`rectifiedDetails` ride the details contract verbatim (true-case
  unobserved — nothing to normalize yet).

### 3.8 Integrity cross-checks (parse/discovery-time REJECTS, not scores)

1. `/Interests/{id}` response `id` must equal the requested id (pipeline threads it).
2. `category.id` must be in §3.1's table; unknown ⇒ FREEZE adapter + review_task
   (rules change).
3. `member.house` must be `Commons`; `category.type` must be `Commons`.
4. `updatedDates.length` must equal the `version` in the FilingRef being fetched; a
   mismatch means the interest changed between discover and fetch — re-emit the
   FilingRef with the observed version (not an error, but never store under a stale
   version key).
5. A Bronze interest document parses to EXACTLY 1 StagingRow; 0 rows ⇒ freeze +
   review_task (invariant 6). NOTE the scope: zero items from a windowed DISCOVERY
   sweep is a normal quiet period (recess), not a freeze — the zero-row rule applies
   to parsing fetched documents.
6. Pagination sanity per §2.2 (`totalResults` stability across a walk).

### 3.9 Legacy pre-2024 registers (out of green-path scope)

Pre-2024-03 registers exist only as legacy HTML/PDF on publications.parliament.uk,
which Cloudflare-challenges non-browser clients (E23). Backfill is a separate goal:
browser-engine fetch seam (us_senate precedent) or an alternative bulk source; rows
from that era would come with the prose-parsing problem the API route avoids. Not
fixtured, not built, recorded here so nobody "quickly scrapes" it.

## 4. Silver contract — `StagingRow` (stg_uk_commons_register)

Source-faithful; verbatim values from the fetched JSON; no normalization beyond JSON
parsing itself; no entity resolution. One row per Bronze document (interest version).
This is the shape `expected.silver.json` asserts. test-designer authors against THIS
table, not parser code. DDL mirrors us_house: linkage columns `id`,
`raw_document_id`, `created_at` + dedup key `unique (raw_document_id, row_ordinal)`;
`stg_meta` carries run linkage.

| Field | Type | Req | Content |
|---|---|---|---|
| `interest_id` | integer ≥1 | yes | top-level `id` |
| `row_ordinal` | integer | yes | always 1 (one interest per document; kept for the shared dedup key shape) |
| `version` | integer ≥0 | yes | `updatedDates.length` (must equal the FilingRef version, §3.8) |
| `parent_interest_id` | integer\|null | yes | `parentInterestId` |
| `category_id` | integer | yes | `category.id` |
| `category_number_raw` | string | yes | `category.number` verbatim (`"7"`, `"1.1"`) |
| `category_name_raw` | string | yes | `category.name` verbatim |
| `member_id` | integer | yes | `member.id` (MNIS) |
| `member_name_raw` | string | yes | `member.nameDisplayAs` |
| `member_list_name_raw` | string | yes | `member.nameListAs` |
| `member_from_raw` | string | yes | `member.memberFrom` (constituency) |
| `party_raw` | string\|null | yes | `member.party` |
| `house_raw` | string | yes | `member.house` (must be `Commons`, §3.8) |
| `summary_raw` | string | yes | `summary` verbatim |
| `registration_date_raw` | string\|null | yes | `registrationDate` as printed (`YYYY-MM-DD`); null on legacy rows |
| `published_date_raw` | string\|null | yes | `publishedDate` as printed |
| `updated_dates_raw` | json array | yes | `updatedDates` verbatim (may be `[]`) |
| `rectified` | boolean | yes | `rectified` |
| `rectified_details_raw` | string\|null | yes | `rectifiedDetails` |
| `fields_raw` | jsonb | yes | the `fields` array VERBATIM (incl. `""` vs null empties, typos like `resigtered` (E20), non-ASCII `Gŵyl` — raw is sacred) |
| `confidence` | number [0,1] | yes | §6 scoring |
| `extractor` | string | yes | `uk_commons_register/api@1` |

## 5. `details` contract — (uk_commons_register, interest)

Schemars type `UkCommonsRegisterInterestDetailsV1` in
`crates/adapters/uk_commons_register/src/details.rs`, snapshot committed at
`crates/pipeline/schemas/details/uk_commons_register.interest.json` (adapter-local
placement per the T8d audit ruling recorded in us-house.md §5; schema-contracts skill
learnings apply — doc comments are contract surface). Field list (no Rust here by
task rule):

| Field | JSON type | Req | Source |
|---|---|---|---|
| `interest_id` | integer ≥1 | yes | StagingRow.interest_id |
| `version` | integer ≥0 | yes | StagingRow.version |
| `parent_interest_id` | integer\|null | no | StagingRow.parent_interest_id (payment→payer join, category 1.x) |
| `category_id` | integer | yes | StagingRow.category_id |
| `category_number` | string | yes | StagingRow.category_number_raw |
| `category_name` | string | yes | StagingRow.category_name_raw |
| `member_id` | integer | yes | StagingRow.member_id (resolution audit trail) |
| `registration_date` | string date\|null | no | StagingRow.registration_date_raw |
| `published_date` | string date | yes | StagingRow.published_date_raw (reject if null at promotion — unobserved) |
| `updated_dates` | array[string date] | yes | StagingRow.updated_dates_raw |
| `rectified` | boolean | yes | StagingRow.rectified |
| `rectified_details` | string\|null | no | StagingRow.rectified_details_raw |
| `shareholding_threshold_raw` | string\|null | no | category 7 only: the `ShareholdingThreshold` value verbatim (query-hot copy; also inside `fields`) |
| `value_source` | string enum `value_field`\|`shareholding_threshold`\|`sum_of_donors`\|`none` | yes | §3.4 provenance |
| `fields` | array[Field] | yes | StagingRow.fields_raw; `Field = {name: string, description: string\|null, type: string, currency_code: string\|null, value: any, values: array[array[Field]]\|null}` (flattens `typeInfo.currencyCode`; `value` is deliberately schema-`any` — the payload is source-shaped and category-specific) |

### 5.1 StagingRow → GoldCandidate mapping (cite: E5–E20 fields per §3)

| GoldCandidate field | Rule |
|---|---|
| `record_type` | `interest` always |
| `asset_description_raw` | `summary_raw` verbatim (invariant 2) |
| `asset_class` | §3.1 category map (`equity` for cat 7, `real_estate` for cat 6, else `other`) |
| `side` | NULL (validate() requires it only for transactions) |
| `transaction_date` | NULL |
| `as_of_date` | NULL |
| `notified_date` | parse `registration_date_raw` (`YYYY-MM-DD`); NULL when source null (§3.6) |
| `value` | §3.4 rules R1–R4; decimal strings; open-ended = `low` set, `high` NULL, GBP |
| `owner` | §3.5 map |
| `instrument_id` | NULL at parse; category 7 `OrganisationName` (in `fields`) is the resolution input — company-name fuzzy resolution is below-threshold by default ⇒ stays NULL + review_task (invariant 3) |
| `extraction_confidence` | StagingRow.confidence |
| `extracted_by` | StagingRow.extractor |
| `fingerprint` | canonical sha256 over (filing_id, ordinal, content) — Task 6 machinery |
| `details` | §5 object, validated against the snapshot schema at promotion (invariant 5) |
| filing | §2.5: `external_id "{id}@{version}"`, `filing_type "interest"`, `filed_date` = registration date, `published_at` = published date 00:00Z, `supersedes_filing_id` per §2.5/§3.7 |

## 6. Extraction strategy (spec-writer exclusive; builders read it HERE)

**Decision: deterministic, no LLM seam on the green path** (extraction-strategy skill;
design §5.3 "open-data feeds get coded parsers"). The Bronze document is JSON from a
documented public API with a published OpenAPI schema — the best possible extraction
input; any LLM involvement would be an anti-pattern. This is the first adapter whose
parse stage is pure `serde_json`.

1. **Primary path** — `serde_json` deserialize into source-shaped structs mirroring
   E1's `PublishedInterest`/`Field` (tolerant of unknown keys at the JSON level is NOT
   allowed: `deny_unknown_fields` on the top-level object so schema drift surfaces as
   a freeze, while `fields[].name` VOCABULARY stays open by design, §3.2). Read bytes
   as UTF-8 (non-ASCII observed: `Gŵyl`, `£`).
2. **Confidence scoring** (per row): start 1.00; −0.02 `registration_date_raw` null
   (legacy migrated row; `notified_date` lost); −0.05 `value_source = sum_of_donors`
   with >1 donor (aggregation convention on an unobserved multi-donor shape). Hard
   REJECTS (not scores): §3.8 checks, §3.4 R2c unknown threshold string, unmapped
   `currencyCode`, mixed-currency donor sum, unparseable date string, empty `summary`
   — row/doc goes to review, never low-confidence Gold (invariant 6 over confidence).
3. **LLM seam**: none wired for this adapter's green path. The only future LLM surface
   is the pre-2024 legacy backfill (§3.9), a separate goal with its own SAF section.
4. **Escalation criteria** (record the flip here + quirks log if taken): the API
   response fails `deny_unknown_fields` (contract drift — freeze first, then extend
   structs against re-archived evidence), or a `Field.value` arrives in a JSON type
   outside §3.2's table for its declared `type`.
5. **Fetch client**: plain `reqwest` with the identified UA + `From:` header — proven
   by 24 served requests (E24). No bot-manager problem on this host (contrast
   us_senate §2.5); if that ever changes: freeze + work item, no evasion.
6. **Cache by sha** (design §5.3): re-extraction only on `extractor` version bump.

## 7. Conformance fixtures (test-designer captures; DO NOT commit from this leg)

Selection: the three required value cases (open-ended threshold / exact amount /
no-value categorical) + the parent/child employment pair. All verified live
2026-07-05; canonical bytes = the raw response body of
`GET /api/v1/Interests/{id}`; sha256 pinned below and archived byte-for-byte in
`docs/regimes/uk_commons_register/evidence/` (same commit — the sampler re-fetches
and confirms the sha; drift procedure below).

**Pinning rule (version-scoped raw bytes, with a defined drift procedure):** pin the
sha256 of the RAW RESPONSE BYTES, valid for the interest VERSION fixtured (all pins
below are `version 0`, `updatedDates: []`). Within-session re-GETs were byte-identical
2/2 (15475, 15854 — E24 variance tests); responses contain no session-variant markup
(no cookies, no CSRF, no timestamps beyond the data itself). Drift procedure on
re-capture: (a) if the re-fetched document's `updatedDates` grew or fields changed,
the SOURCE updated the interest — that is §3.7 semantics, not pin drift: keep the
archived v0 bytes as the fixture (Bronze is immutable), record the new version in the
quirks log, and optionally fixture the new version as an additional update-case;
(b) if bytes differ but the parsed content is logically identical (serialization
drift), switch pinning to the **normalized content hash**: sha256 over the UTF-8
serialization of the response JSON with every `links` key removed recursively
(HATEOAS boilerplate at all levels), keys sorted recursively, compact separators
(no insignificant whitespace) — defined here so sampler and conformance implement it
identically; record the flip in the quirks log (SAF-first). Cross-session byte
stability is an open question until the capture leg re-verifies (us_senate precedent:
its leg-B capture became the third confirmation).

| # | Case | Interest | Member (MNIS id) | Category | Registered | Published | URL | sha256 (raw bytes) |
|---|---|---|---|---|---|---|---|---|
| 1 | shareholding, threshold (ii) → **open-ended 70000.00–NULL GBP** (`value_source shareholding_threshold`, owner self, `OrganisationName` = instrument-resolution input) | 15475 v0 | John Glen (4051), Salisbury, Con | 7 | 2026-06-17 | 2026-06-17 | https://interests-api.parliament.uk/api/v1/Interests/15475 | `8b8613ade949e0b718eb0e7a9640d5d67a9d750ac2d62854edadc6a6ba7d5086` |
| 2 | donation, **exact in-kind £3,400 → 3400.00–3400.00 GBP** (`value_field`); donor company + Companies House id; ReceivedDate/AcceptedDate present | 15923 v0 | Dan Carden (4651), Liverpool Walton, Lab | 2 | 2026-06-28 | 2026-06-29 | https://interests-api.parliament.uk/api/v1/Interests/15923 | `402b3712abb121993f32491f257fc2cadc3cfe382803c2ad2e01bdfe1b105e73` |
| 3 | land/property, **no monetary field → value NULL** (`none`); `IsSoleOwner=false` → owner `joint`; registrability booleans in `fields_raw` | 15854 v0 | Sarah Pochin (5403), Runcorn and Helsby, Reform | 6 | 2026-06-24 | 2026-06-24 | https://interests-api.parliament.uk/api/v1/Interests/15854 | `e461b7be25dc05be4319011409ca81c99a65014221947d20fae4dac84311dc60` |
| 4 | employment payer PARENT: no money field (`none`), owner self; the payer side of the 1→1.1 join | 15914 v0 | Liz Saville Roberts (4521), Dwyfor Meirionnydd, PC | 1 | 2026-06-26 | 2026-06-29 | https://interests-api.parliament.uk/api/v1/Interests/15914 | `f15d0e13f9a93eb451d951a61062180ae09a5697ec117de906f1b8128183243a` |
| 5 | ad hoc payment CHILD of #4: `parentInterestId=15914`, **exact 500.00 GBP** + `HoursWorked "5.00"` (Decimal WITHOUT currencyCode — the not-money disambiguator), `IsPaymentDonated`, non-ASCII `Gŵyl` (UTF-8) | 15915 v0 | Liz Saville Roberts (4521) | 1.1 | 2026-06-26 | 2026-06-29 | https://interests-api.parliament.uk/api/v1/Interests/15915 | `2f50d45ee0c0f70ec5abb1b092ff27551f5db37245508707109e66a0427a79b6` |

Rationale: #1 is the design doc's canonical UK example made real (gold.rs
`uk_interest_fixture`: 70000–open GBP); #2 exercises exact-amount money, the donor
payload, and date fields; #3 exercises the no-value categorical branch plus the only
deterministic non-self owner mapping; #4+#5 exercise the parent/child filing pair,
`parentInterestId` in details, money-vs-non-money `Decimal` disambiguation (§3.2), and
UTF-8 handling. Together they cover every §3.4 value rule except R2b/R3-multi-donor
(both single-string variants of already-fixtured grammars; R2b additionally appears in
list evidence E5 id 15383 for the expected-output cross-check). Expected outputs per
automation policy (no human gate): high-confidence extraction + second-model
cross-check, published `unverified`, sampling-audit queue.

## 8. Evidence log (retrieved 2026-07-05; full request/politeness detail in E24)

Archived under `docs/regimes/uk_commons_register/evidence/` **in this commit**,
sha-named (`{sha256}.{slug}.{ext}`).

| ID | URL | sha256 / note |
|---|---|---|
| E1 | https://interests-api.parliament.uk/swagger/v1/swagger.json | `c75bf11fdbf6b12d03f9a34b21a3b038e4ed30d15e32083b960c452f84116059` OpenAPI 3.0.1 contract (endpoints, param semantics, schemas) |
| E2 | https://interests-api.parliament.uk/api/v1/Categories?Take=20 | `44d4de490888c42132a2a16da4b8af50ad4512fd0cfb1f0e808b16d74fe77002` 12 categories (scheme table §3.1) |
| E3 | https://interests-api.parliament.uk/api/v1/Registers?Take=20 | `4aa42371e64cc595c65c3f95462238b7acf04d3d82b8b6c8412b2fac22d53126` 47 register versions; latest id 803 pub 2026-06-29; fortnightly spacing; id gap (no 802) |
| E4 | https://interests-api.parliament.uk/api/v1/Registers?Skip=40&Take=20 | `45d75bca490b9178751ae3ec68bb30ba5a8a772aed44dd0319fc01db09bd3775` earliest register id 511 pub 2024-03-18 |
| E5 | .../api/v1/Interests?CategoryId=8&Take=5&SortOrder=PublishingDateDescending | `638bb1a3d02ca019b1d7e92496e7bb85e8d8be4997d5545768d58ce7fa046621` 201 shareholdings; both threshold strings; alternates incl. 15383 (threshold (i)) |
| E6 | .../api/v1/Interests?CategoryId=3&Take=5&... | `2db5ccc57c3bf556b3cb04eba95f903f97307140d8c28eb80d2fa893dfa2799e` 631 donations; Decimal+GBP `"3400.00"`; null vs `""` PaymentDescription (15899/15928) |
| E7 | .../api/v1/Interests?CategoryId=12&Take=2&ExpandChildInterests=true&... | `5c362f9ab25fbd8627b482397b16cd0318279af69f8483170017a2503bb0a59d` employment parents + nested childInterests (1.1 and 1.2 children) |
| E8 | .../api/v1/Interests?CategoryId=1&Take=2&... | `99a39c794a034cb439274d5a7f221dd669983f66eaa1aa81a34804be55b9c3c1` 604 ad hoc payments as individual items with parentInterestId |
| E9 | .../api/v1/Interests?CategoryId=4&Take=2&... | `04b3f2e7f16a02774b2420b0afb5dffd2e4f28028379150a1c4593804a253267` 668 UK gifts (donor payload, exact GBP) |
| E10 | .../api/v1/Interests?CategoryId=5&Take=2&... | `2609d42bf883259f2f6a94c5d9de26b3fa6f2056b6b2232a232b7653ff3b5852` 445 visits; `VisitLocations[]`/`Donors[]` nested Field rows |
| E11 | .../api/v1/Interests?CategoryId=6&Take=2&... | `a3ecdee8dfcbf1e55338de4bffc69c82fca4821c57c1b461775c39874b3178e6` 19 non-UK gifts |
| E12 | .../api/v1/Interests?CategoryId=7&Take=3&... | `93e1b9b885de942353d5d60d953df3675f0f65f52051bd5d0a8af9ad119173f6` 214 land/property; NO money fields; IsSoleOwner both states |
| E13 | .../api/v1/Interests?CategoryId=9&Take=2&... | `1fe33b1c3a3f5b63185061b3fe6f1f4c7f36632c7cfd59a19b935f8b35d942c6` 938 miscellaneous (`Unpaid role`) |
| E14 | .../api/v1/Interests?CategoryId=10&Take=3&... | `4df0cb7a03ddc41c6b79366d1d1201e62b82736d2dc92f1deea55ffd418f31d1` 33 family employed; registrationDate NULL (2704); non-empty updatedDates incl. pre-publication dates |
| E15 | .../api/v1/Interests?CategoryId=11&Take=2&... | `f0d10b4d0a97a3ba4e04a9e2f43fca105d43eb72ae3f1e2e142a7e2313044051` 23 family lobbying |
| E16 | https://interests-api.parliament.uk/api/v1/Interests/15475 | `8b8613ade949e0b718eb0e7a9640d5d67a9d750ac2d62854edadc6a6ba7d5086` (fixture #1) |
| E17 | https://interests-api.parliament.uk/api/v1/Interests/15923 | `402b3712abb121993f32491f257fc2cadc3cfe382803c2ad2e01bdfe1b105e73` (fixture #2) |
| E18 | https://interests-api.parliament.uk/api/v1/Interests/15854 | `e461b7be25dc05be4319011409ca81c99a65014221947d20fae4dac84311dc60` (fixture #3) |
| E19 | https://interests-api.parliament.uk/api/v1/Interests/15914 | `f15d0e13f9a93eb451d951a61062180ae09a5697ec117de906f1b8128183243a` (fixture #4) |
| E20 | https://interests-api.parliament.uk/api/v1/Interests/15915 | `2f50d45ee0c0f70ec5abb1b092ff27551f5db37245508707109e66a0427a79b6` (fixture #5) |
| E21 | https://members-api.parliament.uk/api/Members/4051/RegisteredInterests | `a2e44070e5da5207d764b678c8ff4a5c139adc2ac0b453113e553edfa5ff0a46` REJECTED route: prose-blob interests; has createdWhen/lastAmendedWhen/deletedWhen; serves Last-Modified |
| E22 | https://interests-api.parliament.uk/api/v1/Registers/803/document?type=Updated | `324468cb5dfa8305ed757eb21855915e70f45a332f0250460dfe753ae04cd8b2` official register PDF (`%PDF-1.6`, 244 KB) — snapshot/audit artifact |
| E23 | https://publications.parliament.uk/pa/cm/cmregmem.htm | `4a220e0e5cbdfff388453dd946bb86f8d8c9e485ad98e486bbe65764eb699357` Cloudflare challenge 403 ("Just a moment...") — legacy-route tried-log |
| E24 | (our record) | `2026-07-05-interests-api.retrieval.json` — every request (27 total: 24×200, 2×404 robots, 1×403 Cloudflare), client config, politeness stats, byte-stability variance tests |

## Quirks log (append-only, dated)

- 2026-07-05 · `Take` is hard-capped at 20 by the API contract (E1) — backfill of the
  current register (~4,000 interests across categories) is ~200 pages; at ≥2 s spacing
  that is minutes, not hours. Plan sweeps accordingly.
- 2026-07-05 · NO ETag/Last-Modified anywhere on interests-api (E24); members-api
  (the rejected route) DOES serve Last-Modified. The conditional-GET habit does not
  transfer; `PublishedFrom`/`UpdatedFrom` windows are the incremental primitive.
- 2026-07-05 · `RegisterId` defaults to the LATEST register on /Interests queries
  (E1 param doc) — undated sweeps are snapshot-scoped by default; historical register
  states need explicit RegisterId (untested, open question).
- 2026-07-05 · Interests publish ROLLING between fortnightly register dates (E16
  published 06-17; neighbouring registers 06-15/06-29) — discovery must poll, not
  wait for register publications.
- 2026-07-05 · `Field.value` is polymorphic by `Field.type`: Decimal arrives as a
  JSON **string** (`"3400.00"` — rust_decimal-friendly), Boolean as bool, Int as
  number, DateOnly as `"YYYY-MM-DD"` string. Money ⇔ `typeInfo.currencyCode` present:
  `HoursWorked` is a Decimal WITHOUT currencyCode (E20) — never key money on the
  field name or type alone.
- 2026-07-05 · Complex fields (`Donor[]`, `VisitLocation[]`) carry data in `values`
  (array of array-of-Field), with top-level `value` null (E10).
- 2026-07-05 · Two empty-string conventions observed in the same field across rows:
  `PaymentDescription: null` (15899) vs `""` (15928), E6 — Silver keeps verbatim.
- 2026-07-05 · `summary` embeds a formatted amount (`Arab Investments Limited -
  £3,400.00`, E17) duplicating the typed `Value` — display artifact; parsing money
  from it is forbidden (§3.4).
- 2026-07-05 · `registrationDate` can be NULL (migrated legacy row, E14 id 2704) and
  `updatedDates` entries can PREDATE `publishedDate` (E14 id 2704:
  2015/2017 updates, published 2024-07-23) — publication here is API-era migration,
  not first disclosure. `notified_date` stays honest (registrationDate or NULL).
- 2026-07-05 · `category.number` is a STRING (`"1.1"`), not a number; category ids
  are NOT the category numbers (id 8 = number "7"). Join on id, display number.
- 2026-07-05 · Register ids are not contiguous (…801, 803; no 802 — E3): opaque ids,
  order by publishedDate.
- 2026-07-05 · Human-entered free text ships verbatim, typos included
  (`resigtered`, E19/E7) and non-ASCII (`Gŵyl`, E20) — UTF-8 end to end, no cleanup.
- 2026-07-05 · The interests-api host serves our identified UA without challenge
  (24/24), while publications.parliament.uk (Cloudflare) blocks it (E23) — per-host
  posture differs within parliament.uk; the polite-fetching probe ladder stays
  per-host.

## Operational notes (politeness incidents, outages)

- 2026-07-05 · 27 requests total this task (E24): 24×200 on interests-api +
  members-api, 2×404 (robots probes), 1×403 (publications.parliament.uk Cloudflare
  challenge — single probe, recorded, no retry, no evasion per polite-fetching
  skill). Concurrency 1, ≥2.1 s enforced spacing, zero 429s, zero throttling
  observed.
- 2026-07-05 · Byte-stability variance test: within-session re-GETs of
  /Interests/15475 and /Interests/15854 byte-identical (2/2, E24) — first
  confirmation for the §7 pinning rule; cross-session confirmation falls to the
  capture leg.
- 2026-07-05 · Capture leg (061b): all five §7 fixtures re-fetched in a NEW
  session (5 requests, concurrency 1, ≥2.2 s spacing, identified UA + From,
  zero 429s) — 5/5 sha256 byte-identical to the pins. Cross-session byte
  stability CONFIRMED; the raw-byte pinning rule stands, normalized-hash
  fallback not needed. Retrieval record:
  `evidence/2026-07-05-uk-fixture-capture-061b.retrieval.json`.
