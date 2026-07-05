---
# RegimeSurvey front-matter (validated shape). Every claim: {claim, evidence:[files]}
jurisdiction: "ca"
bodies: ["Office of the Conflict of Interest and Ethics Commissioner (federal)"]
legal_basis:
  claim: "TWO legal instruments feed ONE public registry: the Conflict of Interest Act (S.C. 2006, c. 9, s. 2 — applies to ~3,000 appointed public office holders incl. ministers, ministers of state, parliamentary secretaries, ministerial staff, GIC appointees) and the Conflict of Interest Code for Members of the House of Commons (applies to all 343 MPs). Statute/Code full text NOT yet archived (laws-lois.justice.gc.ca linked from every details page but not fetched — open question); per-type obligations are anchored on the statutory-requirement boxes the registry prints VERBATIM on each details page: Act 25(2) declarable assets ≤120d (E22), 25(3) liabilities ≥$10,000 'but not the amount' ≤120d (E23), 25(4) outside activities ≤120d (E24), 25(5) gifts ≥$200 ≤30d + 51(1)(a) registry duty (E20), 26(1) summary statements ≤120d (E25); Code 14(1) gifts acceptability rule (E21), 21(3) material changes ≤60d (E28), 23(1) Commissioner-prepared disclosure summaries (E27)."
  evidence: ["E20", "E21", "E22", "E23", "E24", "E25", "E27", "E28", "E40 important-notices (two-regime structure, filer populations)"]
who_files:
  claim: "Registry mixes six affiliation roles (search-form legend, E1): Members of Parliament, Ministers and Ministers of State, Parliamentary Secretaries, Ministerial Staff, Governor in Council Appointees, Public Office Holders. v1 adapter scope: the three POLITICIAN roles only (MPs cac94a19-…, Ministers c8c94a19-…, Parliamentary Secretaries d2c94a19-…) — ministerial staff/GIC appointees/other POHs are not in govfolio politician rosters (would flood unresolved_filer, invariant 3). Role-scoped queries verified live (E36: ministers' declarable assets = 114; E37: MPs' outside activities = 0; E38: ministers' outside activities = 16)."
  evidence: ["E1", "E36", "E37", "E38", "E40"]
record_types: [interest, change_notification]
value_precision: "none"
band_table:
  # NOT a banded regime and NOT encoded into ValueInterval — value is NULL on every row (§3.6).
  # This table pins the statutory registrability floors printed in the archived record bytes,
  # for the methodology page and any future founder-level revisit of the NULL-value decision.
  - {raw: "Public Declaration of Liabilities ($10,000 or more) — h1 template text; Act 25(3): 'liabilities of $10,000 or more … but not the amount'", low: null, high: null, observed: true}   # deliberately NOT low=10000: rules-threshold, not record data (§3.6, UK R4 precedent)
  - {raw: "Act 25(5): 'gift or other advantage that has a value of $200 or more' (statutory box, E20)", low: null, high: null, observed: true}   # same rule: never inferred into value
cadence_and_lag:
  claim: "Rolling, event-triggered publication: gifts ≤30d after acceptance (Act 25(5), E20); material changes ≤60d after the change (Code 21(3), E28); appointment-triggered initial declarations ≤120d (Act 25(2)/(3)/(4)/26(1), E22–E25); Code disclosure summaries follow the Commissioner's review process, no fixed public window archived (E27). Observed same-day-ish freshness: gift received 2026-06-30 disclosed 2026-06-30 (E20); material change dated 2025-07-22 disclosed 2026-06-26 — 11 months (Code items surface via the summary/review cycle, E28). disclosure_lag_days stays NULL: per-type windows live in §3.1, one number would lie."
  evidence: ["E20", "E22", "E23", "E24", "E25", "E27", "E28"]
formats: [server_rendered_html]
access: {method: "anonymous HTTPS GET, server-rendered ASP.NET Core (Kestrel) pages with GET query-string filters; no session, no cookies set, no CSRF anywhere on public pages", session_required: false, captcha: "none — identified UA served 48/48 requests without challenge (E44)", notes: "NO ETag/Last-Modified on any response → conditional GETs impossible; disclosureFrom/disclosureTo windowed queries are the incremental primitive (E3). robots.txt is a catch-all HTML route — no robots policy is served (E41); politeness limits are self-imposed (invariant 10)."}
historical_depth: {from: "2008-05-02 (earliest disclosure date under sortBy=declarationDisclosureDate&sortDir=asc, E4). Registry content is migrated from the previous SharePoint site (ExternalClass… div artifacts inside dd content, E14) — migration completeness unverified", evidence: ["E4", "E14"]}
identifiers_available: {politician: "stable source-native person GUID (clientId) on every card and details page, linking to /en/client?clientId= profile with affiliations; MP profiles carry party + Constituency + start date (E33: Groleau, Conservative, Beauce); minister/staff profiles carry office + dates (E32). Resolution = name+constituency/office join against roster, keyed by clientId as politician external id", instrument: "none — assets/corporations are free text (numbered Quebec companies, 'IGA Famille Groleau'); no ticker/ISIN/registry id anywhere; instrument_id stays NULL (invariant 3)"}
amendment_mechanism:
  claim: "No amendment documents and no version/update timestamps exist. Observed mutability: declarationStatus flips Active → 'No Longer Applicable', which ADDS a bg-warning badge to the details page (E29) and to list cards (E35 — 2,461 NLA rows) — i.e. the SAME URL's bytes change with no detection channel (no updatedFrom filter in the form legend, E1). Content edits beyond status: unobserved. v1 fail-closed handling in §3.8: fetch once per declarationId; a re-fetch that yields different bytes for a stored declaration ⇒ new Bronze doc + freeze that declaration + review_task. Material Changes are NEW declarations (own GUID) — notices, not amendments; no supersession guessing (§3.8)."
  evidence: ["E1", "E29", "E35"]
personal_data_to_redact: ["Third-party personal data is published deliberately: compliance-measure screens NAME private individuals (a friend 'Philip Baldwin', a common-law partner 'Ms. Stephanie O'Brien' with employer details — E16; that type is OUT of v1 scope, which also limits exposure); in-scope Disclosure Summaries carry spouse/common-law-partner employment and assets (E27); gift circumstances may reference family members (Act 25(5) covers family acceptance, E20). Keep verbatim in Bronze/Silver/details; whether govfolio's PUBLIC rendering surfaces or search-indexes non-politician individuals is a product/legal decision — flagged, default to not indexing non-politician names."]
tos_and_politeness:
  claim: "Public registry published for exactly this purpose ('a searchable database that anyone can access. It lets Canadians see how federal officials are complying', E40). No ToS gate encountered; /en/important-notices is a welcome/transition notice with no access restrictions (E40). No robots policy served (catch-all HTML, E41). Politeness: concurrency 1, ≥2.2s interval, identified UA 'govfolio.io research (contact: ssm.leo@outlook.com)' + From header — 48 requests this task, zero 429s, zero challenges (E44)."
  evidence: ["E40", "E41", "E44 retrieval log"]
language: [en, fr]
open_questions:
  - {question: "Archive the Conflict of Interest Act + Members' Code full text (material-change window under the Act, Travel s.12-ish basis, Code gift disclosure threshold, exempt/controlled asset definitions) — legal_basis anchors beyond the per-page statutory boxes", tried: ["laws-lois.justice.gc.ca and the Code are linked from every details page (links archived in E20–E28 bytes) but not fetched — request budget; statutory boxes archived verbatim instead"]}
  - {question: "affiliationRole filter semantics: does it match the client's affiliation AT filing time or their CURRENT affiliation? (role-change cases could leak in/out of sweeps)", tried: ["role-scoped counts fetched (E36–E38); a filer with a role change spanning declarations is needed to distinguish — none identified politely"]}
  - {question: "Status-flip detection: no updatedFrom/status-change feed exists (form legend E1) — how do we learn a stored declaration went 'No Longer Applicable'?", tried: ["full form legend enumerated (E1); §2.3 documents the periodic full re-sweep option — deliberately OUT of the v1 green path"]}
  - {question: "In-place content edits (beyond the status badge): do declarations get corrected at the same declarationId?", tried: ["within-session re-GETs byte-identical 2/2 (E44 variance tests); no edit case observed; §3.8 freezes on unexplained byte drift"]}
  - {question: "Disclosure Summary supersession chain (initial vs annual-review summaries of the same MP): deterministic linkage absent — same client + type + later date is a GUESS", tried: ["Groleau material change (2025-07-22 items) and summary (2026-06-26) coexist, summary already folds the change in (E27/E28); no linkage field anywhere"]}
  - {question: "Section-label vocabulary completeness for itemized types — 8 labels observed on one summary (E27) + 2 on one material change (E28, one UPPERCASE with statute ref); 'Dependent children' sections unobserved", tried: ["one full Disclosure Summary + one Notice of Material Change archived; §3.5 owner grammar accepts observed labels + documented prefixes only, else review_task"]}
  - {question: "'Gift received date' missing on 1/30 Gifts (Code) list cards (E12) — nullable, but why (legacy migration? optional field?)", tried: ["29/30 cards carry it; the missing card not detail-fetched (budget); Silver keeps it nullable"]}
  - {question: "EN↔FR content parity guarantee (is one side authoritative? is translation lagged?)", tried: ["one EN/FR pair sampled — same GUID, parallel path, fully translated content (E20 vs E30); GUID parity of filter values confirmed (E31); 'OCIEC Translation' badge (E29) proves some content is office-translated"]}
  - {question: "Cross-session byte stability of details pages (single research session so far)", tried: ["within-session re-GETs of 2 details pages byte-identical 2/2 (E44); normalized-hash fallback defined in §7"]}
  - {question: "The ~98-row gap between the unfiltered total (8,398, E1) and the sum of the 15 declarationType counts (8,300, §3.1) — presumably the six 'Other documents' declarationReportType kinds", tried: ["all 15 type counts fetched (§3.1); report-type kinds not enumerated (out of scope, budget)"]}
  - {question: "Migration completeness of pre-2026 records from the old SharePoint registry (ExternalClass… artifacts prove migration)", tried: ["earliest row 2008-05-02 (E4); official claim 'all the same information is available' (E40) — not independently verifiable"]}
  - {question: "Site transition risk: this is an officially 'temporary website' with phased improvements toward ethicscanada.ca (E40) — layout drift is EXPECTED", tried: ["nothing to try; recorded as the sentinel's top watch item for this adapter"]}
regime_versions:
  - {effective_from: "2008-05-02", change: "Earliest observed registry row (E4); the Conflict of Interest Act era (in force 2007 per common knowledge — NOT asserted until the statute text is archived, see open questions)", evidence: ["E4"]}
---

# Canada — CIEC Public Registry (Conflict of Interest Act + Members' Code) — Source Authority File

> **Internal context; the public methodology page derivation requires founder review
> (residual human lane: methodology PUBLIC copy).** Goal 062 leg A (spec). Written
> BEFORE any adapter code, per the adapter template (design §5.1, plan Task 8) and the
> uk_commons_register/us_senate pattern (`docs/regimes/uk_commons_register.md`,
> `docs/regimes/us_senate.md`).

Scope: the **public registry of the Office of the Conflict of Interest and Ethics
Commissioner** (ciec-ccie.parl.gc.ca) — govfolio's second non-transaction adapter and
first `change_notification`-record regime. One govfolio regime covers BOTH legal
instruments served by the registry (Act + Members' Code): single publication venue,
single format, single cadence; the instrument is record-level metadata
(`details.law`, §5). v1 ingests the ten financial-substance declaration types (§3.2)
filed by the three politician roles (§2.4); everything else is explicitly out of
scope (§3.3). **No record ever carries a monetary value** (§3.6) — `value` is NULL on
every row, by statute ("but not the amount", E23).

Evidence citations `E1..E44` refer to §8. All retrievals 2026-07-05, identified UA
`govfolio.io research (contact: ssm.leo@outlook.com)` + `From:` header, concurrency 1,
≥2.2 s spacing (E44). Everything is archived under
`docs/regimes/canada_ciec/evidence/` **in this same commit**.

Per automation policy (`docs/decisions/automation-policy.md`), the goal's "HUMAN
completes expected.*.json" step is superseded: the test-designer authors expecteds
independently (high-confidence extraction + second-model cross-check), records publish
`unverified`, sampling-audit queue.

## 1. Regime metadata

| Field | Value |
|---|---|
| jurisdiction | `ca` (national) |
| body | `Office of the Conflict of Interest and Ethics Commissioner (federal)` — one regime row covering the Act + the Members' Code (rationale above; per-record instrument in `details.law`) |
| regime_type | `change_notification` — the obligation is event-triggered (gift accepted → declare ≤30d; material change → notice ≤60d; appointment → initial declarations ≤120d), published rolling; design §5.5 Tier 2 names "Change-notification registers (UK, AU, **CA**)" explicitly. The Code's annual-review summaries add a periodic flavor, but the registry's publication model is rolling declarations, not periodic snapshots |
| value_precision | `none` — no record carries a value, a band, or a threshold FIELD; the Act mandates detail "sufficient … to identify the source and nature of the liability **but not the amount**" (E23). Statutory registrability floors ($10,000 liabilities / $200 gifts) live in the RULES, not the records → never encoded (§3.6) |
| cadence | rolling; per-type statutory windows: 30d gifts (Act), 60d material changes (Code), 120d initial declarations (Act) — §3.1 |
| disclosure_lag_days | NULL — one number would misrepresent the per-type windows (front-matter cadence claim) |
| source_url | https://ciec-ccie.parl.gc.ca/en/public-registry |
| list endpoint | `GET /en/public-registry` (full page) · `GET /en/public-registry/cards` (bare fragment, same params — the sweep endpoint, E2) |
| details endpoint | `GET /en/public-registry/Details?declarationId={guid}` (canonical Bronze document, §2.5) |
| person endpoint | `GET /en/client?clientId={guid}` (profile: affiliations, per-person declarations; E32/E33) |
| roster endpoint | `GET /en/profile?affiliationRole={guid}` (person enumeration + compliance status; E34) |
| FR mirror | `/fr/registre-public[...]` — same GUIDs, parallel paths, translated content (§3.9; E30/E31) |
| currency | CAD contextually — but no value is ever stored (§3.6), so no currency is ever emitted |
| cadence tier | 2 (design §5.5): discover hourly–daily; latency target same day |

## 2. Discovery

**Data-source decision: the registry's server-rendered HTML behind GET query
parameters.** No machine-readable route exists; evidence-ranked:

1. **`/en/public-registry` + `/cards` fragment (CHOSEN):** anonymous, stateless
   (no cookies/session/CSRF), fully server-rendered cards with stable GUID keys for
   declarations, persons, types, roles, statuses; date-window + role + type filters;
   deterministic dt/dd + sectioned-div grammar (§3.4). The `/cards` URL returns the
   bare card fragment (E2) — same data, less chrome; use it for sweeps.
2. **JSON API (REJECTED — none exists):** `swagger/v1/swagger.json` → 404 (E43);
   `Accept: application/json` on the cards endpoint is ignored, HTML comes back
   (E44 api_probes). The regime-research probe found no dedicated API host.
3. **Old SharePoint registry (GONE):** the current site is the official replacement
   ("the registry looks a little different … all the same information is available",
   E40); migrated content shows `ExternalClass…` SharePoint artifacts (E14, E21).

### 2.1 Query surface (verbatim from the search form, E1)

| Param | Values | Notes |
|---|---|---|
| `page` | 1-based | 30 cards/page ("8398 Result(s) — page 1 of 280", E1) |
| `searchTerm` | free text | "Name, gift, source..." |
| `declarationType` | 15 GUIDs (§3.1 census) | |
| `declarationReportType` | 6 GUIDs (Compliance Orders, Examinations (Act), Exemptions (Section 38), Inquiries (Code), Penalties, Waivers (Section 39)) | ALL out of scope (§3.3) |
| `affiliationRole` | 6 GUIDs (§2.4) | |
| `declarationStatus` | Active `ecfdb65d-…` / No Longer Applicable `6e4f9ba2-…` | §3.8 |
| `disclosureFrom` / `disclosureTo` | `YYYY-MM-DD` | windowed sweeps verified: June 2026 = 86 rows (E3) |
| `sortBy` | `declarationDisclosureDate` (default) \| `lastName` \| `firstName` | |
| `sortDir` | `desc` (default) \| `asc` | asc probe reached 2008-05-02 (E4) |

List responses carry `N Result(s) — page X of Y`; no JSON, no HATEOAS. **No response
serves ETag/Last-Modified and no cookies are set** (E44) — conditional GETs are
impossible; the date window is the incremental primitive.

### 2.2 Pagination contract

30 cards/page; walk `page += 1` until the printed page count is reached. The result
count is recomputed per request — a changed `N Result(s)` mid-walk means the window
moved under us; restart that sweep (idempotent writes make the re-walk free). Sweep
`sortDir=asc` on `declarationDisclosureDate` so late-arriving rows append past the
high-water mark instead of shifting earlier pages.

### 2.3 Discover algorithm + politeness

1. Three windowed sweeps of `GET /en/public-registry/cards` — one per in-scope role
   GUID (§2.4) — with `disclosureFrom = hwm − 30d`,
   `sortBy=declarationDisclosureDate&sortDir=asc`. The 30-day overlap is free
   (idempotency). Do NOT filter by declarationType in sweeps (15×3 sweeps would burn
   requests); filter client-side on the card's type badge into the §3.2 in-scope set.
2. Emit `FilingRef` per in-scope card: `external_id = declarationId` (lowercase GUID
   from the Details href), declaration-type label + GUID, `clientId`, client name,
   client title suffix (nullable), law line, `Disclosed on` date, NLA badge presence.
   New filing ⇔ unseen `(regime_id, external_id)`. Idempotent: `ON CONFLICT DO
   NOTHING`. Out-of-scope-type cards are counted in run stats but emit nothing.
3. `fetch`: `GET /en/public-registry/Details?declarationId={guid}` (EN) once per new
   `external_id` → store the raw response bytes as the Bronze document
   (sha256-addressed, invariant 2). One Bronze doc = one declaration. FR capture is
   deferred (§3.9) — the FR page is a SEPARATE document addressable later without
   refetching EN (raw is sacred).
4. **Status flips are NOT detectable incrementally** (no updated-since channel, E1).
   The v1 green path ingests declarations once. A periodic full re-sweep
   (280 pages ≈ 10 min at ≥2 s — quirks log) MAY be scheduled later as its own lane;
   re-fetched bytes that differ freeze that declaration (§3.8). Not v1.
5. Politeness (invariant 10): concurrency 1; ≥2 s min interval; exponential backoff
   on 429/5xx; identified UA + `From:` header (served 48/48 without challenge — no
   us_senate-style fingerprint problem on this host, E44); no robots policy served
   (catch-all HTML route, E41) so these self-imposed limits govern.

### 2.4 Politician resolution & role scope

Role GUIDs (search-form legend, E1 — Dynamics-CRM-style fixed ids):

| Role | GUID | v1 |
|---|---|---|
| Members of Parliament | `cac94a19-d04e-e111-b8ea-00265535a320` | **in** |
| Ministers and Ministers of State | `c8c94a19-d04e-e111-b8ea-00265535a320` | **in** |
| Parliamentary Secretaries | `d2c94a19-d04e-e111-b8ea-00265535a320` | **in** |
| Ministerial Staff | `ccc94a19-…` | out (not politicians; no roster) |
| Governor in Council Appointees | `c6c94a19-…` | out |
| Public Office Holders | `d0c94a19-…` | out |

Every card and details page carries `clientId` — a stable source person GUID linking
to `/en/client?clientId=` (E32/E33). MP profiles print party + **Constituency** +
mandate start (E33: Groleau · Conservative Party of Canada · Beauce · 2025-05-09);
minister/staff profiles print office + tenure ranges (E32). Resolution: store
`clientId` as the politician external id after a first match of client name (+
constituency for MPs / office for ministers) against the roster (roster seeding from
the House of Commons member list + Wikidata is the builder/test leg's concern,
mirroring us_house Task 9). Zero or multiple roster hits ⇒ fail closed —
`review_task reason = "unresolved_filer"` (target `canada_ciec:{declarationId}`), no
filing row, no Gold rows (invariant 3). Once a `clientId` is bound, subsequent
declarations resolve by GUID join, no name matching.

### 2.5 Filing model

- `filing.external_id = declarationId` (lowercase GUID). No version qualifier in v1:
  the source exposes no update timestamps or version history, so there is nothing to
  key a version on (contrast UK `{id}@{version}`); §3.8 freezes on unexplained
  mutation instead.
- `filing_type = "declaration"`; `filed_date` = the `Disclosure date` (§3.7);
  `published_at` = same date at 00:00:00Z — **date-precision convention** (the source
  exposes a date, not a timestamp; midnight-UTC is a documented convention, not
  fabricated precision); `discovered_at` = ours.
- `supersedes_filing_id` = NULL always (no deterministic linkage exists, §3.8).

## 3. Record anatomy

### 3.1 Declaration-type census (all 15, verified live 2026-07-05; counts at retrieval)

| declarationType GUID | Label (badge) | Law | Count | Grammar family | v1 |
|---|---|---|---|---|---|
| `c37a52a7-b858-47db-9bb8-2a6d1f29dc14` | Declarable Assets | Act 25(2) (E22) | 862 (E5) | B flat description | **in** |
| `3f99bf47-a35a-494e-8b78-fe00533857b9` | Liabilities | Act 25(3) (E23) | 336 (E6) | B | **in** |
| `dd98430c-ff79-4c96-b1dc-e881095abe4e` | Outside Activities | Act 25(4) (E24) | 875 (E7) | B | **in** |
| `acdd6784-b1ef-48b5-80ba-08c3c49ef733` | Summary Statements (Act) | Act 26(1) (E25) | 1,910 (E8) | B (+ optional divestment block, E25) | **in** |
| `5924f660-7f25-4702-a727-bc15b1b85dba` | Disclosure Summaries (Code) | Code 23(1) (E27) | 1,645 (E9) | C itemized | **in** |
| `2da4414a-caa4-497b-8fed-9b31d84f68cf` | Material Changes | Code 21(3) (E28); Act-side basis unarchived (open question) | 302 (E10) | C itemized | **in** |
| `abe9286c-93c6-40d7-8fda-16304d3b6b61` | Gifts (Act) | Act 25(5) (E20) | 1,264 (E11) | A typed fields | **in** |
| `665b3de7-7a3d-4227-b077-a390f1d14d88` | Gifts (Code) | Code 14(1) (E21) | 308 (E12) | A | **in** |
| `453089cd-f784-4c49-99ba-d3dd38ff4b98` | Forfeited Gifts (Act) | unarchived | 90 (E13) | A (same 4 dt labels, E13) | **in** |
| `5eda55e6-2726-4ee0-ae4c-ad3d79541281` | Sponsored Travel | Code (s.15 presumed, unarchived) | 347 (E14) | A′ (Destination/Sponsor/Purpose/Dates) | **in** |
| `af4085f1-0baf-4450-a48e-81d7e175f944` | Travel | Act (unarchived) | 39 (E15) | B | **in** |
| `9c59f81f-5a80-4e3f-a49d-de50faab503d` | Compliance Measures | Act s.29 screens (E16 prose) | 123 (E16) | B prose | out |
| `94b3d6f8-cb5f-4cc0-8c03-367612e53c1e` | Recusals | Act 25(1) (per E16 prose) | 173 (E17) | B | out |
| `76e97e8a-fdfc-4e77-9650-adb41c9ec8d6` | Private Interest | unarchived | 6 (E18) | B | out |
| `5bc55a37-f19e-45b2-8222-44a5c5d3b074` | Other Appropriate Documents | — | 20 (E19) | mixed | out |

Sum = 8,300 vs unfiltered 8,398 — the ~98-row gap is presumably the
`declarationReportType` documents (open question). A card whose type label/GUID is
outside this census ⇒ FREEZE adapter + review_task (a new type is a rules change;
invariant 6).

### 3.2 v1 scope declaration (what govfolio ingests)

**IN (10 types):** Declarable Assets, Liabilities, Outside Activities, Summary
Statements (Act), Disclosure Summaries (Code), Material Changes, Gifts (Act),
Gifts (Code), Forfeited Gifts (Act), Sponsored Travel, Travel — the
financial-substance declarations: asset positions, debts, income-bearing activities,
received advantages, and their change notices. Note: **controlled assets are never
published as a list** — their only public trace is the divestment line in Summary
Statements ("Divestment of: Publicly Traded Securities by the establishment of blind
trust(s) / by sale", E25), which is exactly why Summary Statements are in scope.
MPs file NO standalone outside-activity declarations (0 rows, E37) — their
activities/assets/liabilities live inside Disclosure Summaries (E27).

**OUT:** Compliance Measures (CoI screens — administrative prose naming private
third parties, E16), Recusals (procedural), Private Interest (6 rows, procedural),
Other Appropriate Documents, and all six `declarationReportType` document kinds
(enforcement/administrative). Filers outside the three politician roles (§2.4) are
also out — the same declaration types filed by GIC appointees/staff are simply not
swept.

### 3.3 Details-page anatomy (the Bronze document; E20–E29, E45, E46)

Layout identical across all 11 fetched details pages:

1. `<h1>` — the declaration-type title. Observed vocabulary (integrity input, §3.10):
   `Public Declaration of Assets` (E22) · `Public Declaration of Liabilities
   ($10,000 or more)` (E23) · `Public Declaration of Outside Activities` (E24, E29) ·
   `Summary Statement` (E25, E26) · `Disclosure Summary` (E27) · `Notice of Material
   Change` (E28) · `Public Declaration of Gifts or Other Advantages` (Act, E20) ·
   `Public Statement of Gifts or Other Benefits` (Code, E21 — **Act and Code gift
   titles differ**).
2. Registry-section box: `As required under the <i><a href="https://laws-lois…">
   Conflict of Interest Act</a></i>` (or the Code) + a `Statutory requirement(s):`
   block quoting the legal text verbatim — template text per type, archived per page.
3. Card header: client link `/en/client?clientId={guid}` with name; OPTIONAL
   `· {title}` suffix (`Member of Parliament` E27/E28, `Secretary of State (Defence
   Procurement)` E23, ABSENT on E20 — nullable); optional badges: `bg-info` `OCIEC
   Translation` (office-translation provenance) and `bg-warning` `No Longer
   Applicable` (both on E29).
4. First `<dl>`: `Declaration type` (badge label, e.g. `Gifts (Act)`) · `Disclosure
   date` (`YYYY-MM-DD`) · `Regime` (`Conflict of Interest Act` | `Conflict of
   Interest Code for Members of the House of Commons`).
5. `<hr/>` + payload (per grammar family, §3.4).
6. Card footer: `Disclosed on YYYY-MM-DD` (must equal the dd, §3.10).

### 3.4 Payload grammar families

| Family | Types | Shape |
|---|---|---|
| **A** typed fields | Gifts (Act/Code), Forfeited Gifts | second `<dl>`: `Nature` · `Source` · `Circumstance` · `Gift received date` (`YYYY-MM-DD`, **nullable** — 29/30 on E12) |
| **A′** typed fields | Sponsored Travel | `Destination` · `Sponsor` · `Purpose` (may wrap in legacy `ExternalClass…` div, E14) · `Dates` (`YYYY-MM-DD – YYYY-MM-DD (N days)`) |
| **B** flat description | Declarable Assets, Liabilities, Outside Activities, Travel, Summary Statements | `Description` dd containing `<div><div title="{TypeLabel}">{text}</div></div>`; Summary Statements add a boilerplate compliance sentence + optional `<strong>Divestment of:</strong>` block (E25); the title-div can be EMPTY with the text in sibling divs (E26) |
| **C** itemized | Disclosure Summaries (Code), Material Changes | `Description` dd containing `<div id="{DECLARATION-GUID-UPPERCASE}" title="{TypeLabel}">` wrapping repeated `.ciec-summary-field` blocks: `.ciec-declaration-disclosurelabel` (section, e.g. `Assets`, `Liabilities`, `Spouse's/Common-Law Partner's assets`, `INVESTMENT IN PRIVATE CORPORATIONS [Paragraph 24(1)(a)]`) + `.ciec-declaration-disclosurecontent` holding one or more `.ciec-declaration-disclosureitem` divs, **each with its own stable GUID `id`** (E27: 8 sections; E28: 2 sections). Material-change items open with a `Date of change: YYYY/MM/DD` line (slash format!, E28) |

List cards flatten family B/C payloads into a clamped `<p
class="declaration-card-description">` with `<br>` separators — **cards are sweep
input only; the details page is the parse input** (cards lose the item GUIDs and
section structure).

### 3.5 Row semantics: record_type, rows-per-document, owner

**record_type decision (design §4.2 vocab: transaction | holding | interest |
change_notification):**

- **Material Changes → `change_notification`.** The document is literally a "Notice
  of Material Change" (E28) notifying a change event within a statutory window (Code
  21(3)) — the vocabulary's fourth type exists for exactly this. `validate()`
  requires nothing extra (gold.rs: `Interest | ChangeNotification => {}`).
- **All other in-scope types → `interest`.** Not `transaction`: no buy/sell/exchange
  events, no transaction dates, no sides. Not `holding`: `holding` requires
  `as_of_date` (gold.rs) and connotes a valued position snapshot; CIEC declarations
  are threshold-triggered statements that an interest/advantage EXISTS, with no
  values and no snapshot date — the UK-register `interest` shape exactly.

**Rows per Bronze document:**

| Family | StagingRows / Gold rows |
|---|---|
| A / A′ / B | exactly 1 |
| C | one per `ciec-declaration-disclosureitem` (≥1), document order; `section_label_raw` + `item_id_raw` carried per row |

**Owner map (design vocab self/spouse/dependent/joint/unknown; NULL allowed):**

| Condition | Gold `owner` | Evidence |
|---|---|---|
| family B/A′ declarations (Act): assets/liabilities/activities/summary/travel | `self` — the statutory duty covers the RPOH's OWN assets ("all of his or her assets", E22), liabilities ("his or her liabilities", E23), positions (E24) | E22–E25 |
| family C item whose section label starts `Spouse's/Common-Law Partner's` | `spouse` | E27 (two such sections) |
| family C item whose section label starts `Dependent` | `dependent` — UNOBSERVED; grammar accepts the prefix and flags (−0.05, §6) | fail-soft |
| family C item under any other KNOWN label (`Assets`, `Liabilities`, `Activities`, `Other Sources of Income`, `Investment in Private Corporations`, `Affiliated corporations`, case-insensitive, optional `[…]` statute-ref suffix per E28) | `self` | E27, E28 |
| family C item under an UNKNOWN label | reject row → review_task (vocabulary outside archived grammar) | fail closed |
| Gifts (Act/Code), Forfeited Gifts | `NULL` — the Act covers acceptance by the RPOH **or a family member** (E20) and the acceptor is not structurally identified; free-text `Circumstance` is never parsed for owner (guessing) | E20, E21 |

Joint ownership appears only in free text ("Joint owner with three other
individuals…", E27) — text is never parsed into `owner = joint` (guessing;
invariant 3 spirit). The verbatim text is the record.

### 3.6 Value → ValueInterval: NULL, always (the defining mapping)

No declaration carries an amount, a band, or a per-record threshold field — the Act
explicitly requires detail "sufficient … to identify the source and nature of the
liability **but not the amount**" (statutory box, E23). The statutory registrability
floors — $10,000 (liabilities h1 + 25(3), E23) and $200 (gifts, 25(5) box, E20) —
are RULES text rendered as per-type template boilerplate, not per-record data: the
uk_commons_register §3.4 R4 precedent applies verbatim ("thresholds are in the
RULES, not the record … value stays NULL, never inferred"). Contrast with UK R2a
(where a per-record FIELD carried the filer's chosen threshold string): no such
field exists here.

⇒ `value = NULL` on every Gold row; `value_precision = 'none'` at the regime level;
no currency is ever emitted. Any future revisit (e.g. encoding `low=10000` on
liabilities from the h1 template text) is a founder-level methodology decision —
documented in front-matter `band_table`, deliberately not taken here.

Never parse `$` amounts out of `Nature`/`Description` free text (e.g. a gift
description quoting a price) — there is no typed money anywhere in this source.

### 3.7 Dates

| Gold field | Rule |
|---|---|
| `notified_date` | the `Disclosure date` dd (`YYYY-MM-DD`). Rationale: the public declaration IS the notification act this registry records; no earlier filing/registration date is published. Required — a details page without it is rejected (§3.10) |
| `transaction_date`, `as_of_date` | NULL always |
| filing `filed_date` | = disclosure date |
| filing `published_at` | disclosure date at 00:00:00Z (date-precision convention, §2.5) |
| `Gift received date` (family A, nullable) | → `details.gift_received_date`; NEVER promoted to `transaction_date` (record_type is `interest`, not `transaction`) |
| `Date of change: YYYY/MM/DD` (family C material-change items — SLASH format, E28) | → `details.date_of_change` per item (nullable: extraction is fail-soft, raw text always survives in the item text) |
| Sponsored-travel `Dates` range (E14) | → `details.travel_start` / `details.travel_end` / raw string kept |

### 3.8 Mutability, status, supersession (fail-closed handling)

- **Status flip is the one observed mutation:** `Active → No Longer Applicable`
  renders a `bg-warning` badge on cards (E35: 2,461 NLA rows) AND on the details page
  (E29) — same URL, changed bytes. There is no update timestamp and no
  changed-since query (form legend, E1). v1 records status AT FETCH TIME
  (`details.no_longer_applicable`); later flips on ingested declarations are
  invisible to the green path (documented limitation + §2.3 re-sweep option).
- **Unexplained byte drift:** if any declaration is ever re-fetched (re-sweep,
  manual) and the bytes differ from the stored Bronze doc: store the new bytes as a
  new Bronze document (raw is sacred), do NOT create a filing, freeze that
  declaration + `review_task reason = "canada_ciec_mutated_declaration"` — mutation
  semantics are unarchived, so promotion would be guessing (invariant 6).
- **No supersession wiring in v1:** Material Changes are independent notices (own
  GUID, no reference to the summary they modify — E28 vs E27 show the summary
  already folding the change in); successive Disclosure Summaries of the same MP
  have no linkage field. `(client, type, later date)` is a guess — `supersedes_*`
  stays NULL everywhere; the deterministic-linkage upgrade path (if the source ever
  exposes one) runs through the promotion machinery, never the parser.

### 3.9 Bilingual rule (EN/FR)

The registry is fully bilingual: parallel paths (`/en/public-registry` ↔
`/fr/registre-public`, E31), IDENTICAL GUIDs for declarations/types/roles (E31), and
translated content — the FR details page of fixture #7's sibling renders the same
gift with French labels (`Nature`/`Provenance`/`Circonstances`/`Date de réception du
cadeau`) and translated free text (E30 vs E20). Some content is office-translated,
flagged by the `OCIEC Translation` badge (E29).

**v1 rule: English is the ingestion language.** Bronze = the EN details page;
`asset_description_raw` = EN text verbatim; `details.language = "en"`;
`details.ociec_translation` records the badge (provenance that the displayed text is
a translation). The FR page is a distinct, later-addressable document (same GUID,
parallel URL) — a future goal can archive FR alongside without refetching EN.
Which side is legally authoritative is unknown (open question); we store what we
parse and label it.

### 3.10 Integrity cross-checks (parse/discovery-time REJECTS, not scores)

1. `<h1>` must be in §3.3's title vocabulary AND consistent with the
   `Declaration type` dd per the §3.1 census (e.g. `Gifts (Act)` ⇔ `Public
   Declaration of Gifts or Other Advantages`); unknown pair ⇒ FREEZE + review_task.
2. `Regime` dd must be exactly `Conflict of Interest Act` or `Conflict of Interest
   Code for Members of the House of Commons`.
3. `Disclosure date` dd must equal the footer `Disclosed on` date.
4. Family C: the wrapper `div id` (uppercase GUID) must equal the requested
   `declarationId`; every `disclosureitem` must carry a GUID `id` and sit under a
   non-empty section label. (Families A/B don't repeat the GUID — the pipeline
   threads the requested id.)
5. A Bronze details document parses to ≥1 StagingRow, exactly 1 for families A/A′/B;
   0 rows ⇒ freeze + review_task (invariant 6). Scope note: an EMPTY windowed
   discovery sweep is a normal quiet period, not a freeze — the zero-row rule
   applies to parsing fetched documents.
6. Discovery sweep sanity per §2.2 (`N Result(s)` stability across a walk).
7. Client link must parse to a well-formed `clientId` GUID.

## 4. Silver contract — `StagingRow` (stg_canada_ciec)

Source-faithful; verbatim values from the fetched EN details page; whitespace
collapsed at HTML text extraction (documented per field), no entity resolution, no
translation. One StagingRow per §3.5 row. This is the shape `expected.silver.json`
asserts. test-designer authors against THIS table, not parser code. DDL mirrors
us_house: linkage columns `id`, `raw_document_id`, `created_at` + dedup key
`unique (raw_document_id, row_ordinal)`; `stg_meta` carries run linkage.

| Field | Type | Req | Content |
|---|---|---|---|
| `declaration_id` | string (lowercase GUID) | yes | from the fetch URL (threaded by the pipeline; families A/B never print it) |
| `row_ordinal` | integer ≥1 | yes | 1-based document order (always 1 for families A/A′/B) |
| `item_id_raw` | string\|null | yes | family C: the `disclosureitem` GUID verbatim (UPPERCASE as printed); null otherwise |
| `section_label_raw` | string\|null | yes | family C: `disclosurelabel` text verbatim (case + `[…]` suffix intact, E28); null otherwise |
| `h1_title_raw` | string | yes | `<h1>` text, collapsed |
| `declaration_type_raw` | string | yes | `Declaration type` dd verbatim (`Gifts (Act)`, `Disclosure Summaries (Code)`, …) |
| `law_raw` | string | yes | `Regime` dd verbatim (§3.10 check 2) |
| `client_id` | string (lowercase GUID) | yes | from the client href |
| `client_name_raw` | string | yes | client anchor text verbatim (accents intact — `Roxanne Gagné`, UTF-8) |
| `client_title_raw` | string\|null | yes | the `· {title}` suffix, collapsed; null when absent (E20) |
| `disclosure_date_raw` | string | yes | `Disclosure date` dd as printed (`YYYY-MM-DD`) |
| `no_longer_applicable` | boolean | yes | presence of the `bg-warning` NLA badge (E29) |
| `ociec_translation` | boolean | yes | presence of the `bg-info` `OCIEC Translation` badge (E29) |
| `description_raw` | string\|null | yes | families B/C: the row's text content, collapsed (family B: full description incl. divestment block text; family C: this item's text incl. any `Date of change:` line); null for families A/A′ |
| `fields_raw` | jsonb | yes | payload `<dl>` pairs verbatim as `{dt: dd-inner-HTML-text}` (families A/A′; `{}` for B/C) — lossless carrier for `Nature`/`Source`/`Circumstance`/`Gift received date`/`Destination`/`Sponsor`/`Purpose`/`Dates`; unknown dt labels land here without error (open vocabulary), load-bearing extractions have grammars (§5) |
| `confidence` | number [0,1] | yes | §6 scoring |
| `extractor` | string | yes | `canada_ciec/html@1` |

## 5. `details` contracts — (canada_ciec, interest) and (canada_ciec, change_notification)

Two schemars types in `crates/adapters/canada_ciec/src/details.rs`, snapshots
committed at `crates/pipeline/schemas/details/canada_ciec.interest.json` and
`crates/pipeline/schemas/details/canada_ciec.change_notification.json`
(adapter-local placement per the T8d audit ruling recorded in us-house.md §5;
schema-contracts skill learnings apply — doc comments are contract surface). Field
lists (no Rust here by task rule):

**`CanadaCiecInterestDetailsV1`** — every in-scope type except Material Changes:

| Field | JSON type | Req | Source |
|---|---|---|---|
| `declaration_id` | string | yes | StagingRow.declaration_id |
| `row_ordinal` | integer ≥1 | yes | StagingRow.row_ordinal |
| `item_id` | string\|null | no | StagingRow.item_id_raw (family C summaries) |
| `section_label` | string\|null | no | StagingRow.section_label_raw verbatim |
| `declaration_type_raw` | string | yes | StagingRow.declaration_type_raw |
| `law` | string enum `act`\|`code` | yes | derived from `law_raw` (§3.10 check 2 guarantees the binary) |
| `h1_title` | string | yes | StagingRow.h1_title_raw |
| `client_id` | string | yes | StagingRow.client_id (resolution audit trail) |
| `client_title` | string\|null | no | StagingRow.client_title_raw |
| `no_longer_applicable` | boolean | yes | StagingRow.no_longer_applicable |
| `ociec_translation` | boolean | yes | StagingRow.ociec_translation |
| `language` | string const `en` | yes | §3.9 |
| `gift_received_date` | string date\|null | no | fields_raw `Gift received date` (families A; nullable, E12) |
| `gift_source` | string\|null | no | fields_raw `Source` verbatim (query-hot copy) |
| `gift_circumstance` | string\|null | no | fields_raw `Circumstance` verbatim |
| `travel_destination` | string\|null | no | fields_raw `Destination` (A′) |
| `travel_sponsor` | string\|null | no | fields_raw `Sponsor` (A′) |
| `travel_dates_raw` | string\|null | no | fields_raw `Dates` verbatim (range + day count) |
| `travel_start` / `travel_end` | string date\|null | no | parsed from `Dates` (fail-soft: null + raw survives) |
| `fields` | object (string→string) | yes | StagingRow.fields_raw verbatim |

**`CanadaCiecChangeNotificationDetailsV1`** — Material Changes: all fields above
PLUS:

| Field | JSON type | Req | Source |
|---|---|---|---|
| `date_of_change` | string date\|null | no | parsed from the item's `Date of change: YYYY/MM/DD` line (slash format normalized to ISO; fail-soft — null + raw text survives in `asset_description_raw`) |

### 5.1 StagingRow → GoldCandidate mapping (cite: E20–E28 per §3)

| GoldCandidate field | Rule |
|---|---|
| `record_type` | `change_notification` when `declaration_type_raw = Material Changes`; else `interest` (§3.5) |
| `asset_description_raw` | family A: `Nature` dd verbatim; family A′: `Purpose` dd verbatim (destination/sponsor ride details); families B/C: `description_raw` verbatim. Empty ⇒ reject row (invariant 2) |
| `asset_class` | `other` always — no structured asset typing exists anywhere in the source; type labels ("Declarable Assets") span property/farms/businesses and free text is never bucketed (no creative classification; UK precedent) |
| `side` | NULL (validate() requires it only for transactions) |
| `transaction_date` | NULL |
| `as_of_date` | NULL |
| `notified_date` | parse `disclosure_date_raw` (`YYYY-MM-DD`) — required (§3.7, §3.10) |
| `value` | NULL always (§3.6) |
| `owner` | §3.5 map |
| `instrument_id` | NULL at parse; company names in free text (numbered corporations, `IGA Famille Groleau`, E27) are below-threshold by default ⇒ stays NULL + review_task per the resolution waterfall (invariant 3) |
| `extraction_confidence` | StagingRow.confidence |
| `extracted_by` | StagingRow.extractor |
| `fingerprint` | canonical sha256 over (filing_id, ordinal, content) — Task 6 machinery |
| `details` | §5 object per record_type, validated against its snapshot schema at promotion (invariant 5) |
| filing | §2.5: `external_id = declaration_id`, `filing_type "declaration"`, `filed_date` = disclosure date, `published_at` = disclosure date 00:00Z, `supersedes_filing_id` NULL (§3.8) |

## 6. Extraction strategy (spec-writer exclusive; builders read it HERE)

**Decision: deterministic, no LLM seam on the green path** (extraction-strategy
skill; design §5.3). The Bronze document is server-rendered ASP.NET template HTML
with fixed selectors and a small closed grammar — coded parser territory; LLM-first
would be an anti-pattern. Same posture as us_senate (machine-generated HTML), minus
the session/fingerprint problems.

1. **Primary path** — `scraper` crate (html5ever DOM + CSS selectors, proven on
   us_senate). Selectors: `h1`; the registry-section statutory box is IGNORED
   (template text; archived in evidence, never parsed); card header client anchor
   (`a[href*="clientId="]`) + `.text-muted` title suffix; badge spans (`.bg-warning`,
   `.bg-info`) by TEXT match (`No Longer Applicable`, `OCIEC Translation`); first
   `dl` pairs by dt text; payload per family: A/A′ second `dl` → `fields_raw`;
   B `dd > div` text (title-div may be empty with content in siblings, E26 — take
   the dd's full text content minus the show-more button); C
   `div.ciec-summary-field` → label + `.ciec-declaration-disclosureitem[id]` items.
   Whitespace-collapse every text join (`\s+` → single space, trim); decode HTML
   entities; UTF-8 end-to-end (accented names `Gagné`, `Kelly-Rhéaume`; French
   quotes possible).
2. **Confidence scoring** (per row): start 1.00; −0.02 `no_longer_applicable`
   (status semantics thin, §3.8); −0.02 `ociec_translation` (office-translated
   text); −0.05 family C `Dependent…` section label (owner mapping unobserved,
   §3.5); −0.05 material-change item without a parseable `Date of change:` line
   (date lost to details, raw survives). Hard REJECTS (not scores): §3.10 checks,
   unknown section label (§3.5), unknown declaration type (§3.1), unparseable
   `disclosure_date_raw`, empty `asset_description_raw` — row/doc goes to review,
   never low-confidence Gold (invariant 6 over confidence).
3. **LLM seam**: none wired for this adapter's green path. The whole source is
   clean markup; there are no scans, no PDFs.
4. **Escalation criteria** (record the flip here + quirks log if taken): (a) any
   selector in (1) stops matching on a fetched page — this is the EXPECTED failure
   mode given the source is an officially temporary website (E40): freeze first,
   re-archive evidence, extend the grammar against the new bytes; (b) html5ever
   alters/drops a data cell's text on a well-formed fixture; (c) list-card grammar
   diverges from details-page grammar in a way that breaks discovery filtering.
5. **Fetch client**: plain `reqwest` with the identified UA + `From:` header —
   proven by 48 served requests, zero challenges (E44). No bot-manager problem on
   this host (contrast us_senate §2.5); if that ever changes: freeze + work item,
   no evasion.
6. **Cache by sha** (design §5.3): re-extraction only on `extractor` version bump.

## 7. Conformance fixtures (test-designer captures; DO NOT commit from this leg)

Selection: one representative per grammar family across both laws and all three
in-scope roles, covering the goal's asked diversity (an asset, an outside activity,
a liability) plus the controlled-asset trace (divestment summary), both itemized
types (spouse-section owner mapping + per-item GUIDs + date-of-change), and a typed
gift. All verified live 2026-07-05; canonical bytes = the raw response body of
`GET /en/public-registry/Details?declarationId={guid}` (EN); sha256 pinned below and
archived byte-for-byte in `docs/regimes/canada_ciec/evidence/` (same commit — the
capture leg re-fetches and confirms the sha; drift procedure below).

**Pinning rule (raw bytes, with a defined drift procedure):** pin the sha256 of the
RAW RESPONSE BYTES. Evidence this is sound: within-session re-GETs of two details
pages byte-identical 2/2 (E44 variance tests); pages set no cookies and contain no
session tokens, no CSRF, no timestamps beyond the data itself (grep-verified, E44).
Known drift causes and responses on re-capture: (a) the NLA badge APPEARED —
that is §3.8 source mutation, not pin drift: keep the archived Active-state bytes as
the fixture (Bronze is immutable), record the flip in the quirks log, optionally
fixture the NLA state as an additional case; (b) bytes differ but parsed content is
logically identical (serialization drift — plausible on a phased-rollout site,
E40): switch pinning to the **parsed-content hash**: sha256 over the UTF-8
serialization of `h1 \n client_id \n declaration_type \n law \n disclosure_date \n`
then per row (document order) `section_label \t item_id \t
asset_description_or_fields` — fields as sorted `dt=dd` pairs joined by `|`,
whitespace-collapsed — terminated by `\n`; defined here so capture and conformance
implement it identically; record the flip in the quirks log (SAF-first).
Cross-session byte stability is an open question until the capture leg re-verifies
(us_senate precedent: its leg-B capture became the confirmation).

| # | Case | Declaration | Filer (clientId) | Role | Type / Law | Disclosed | URL | sha256 (raw bytes) |
|---|---|---|---|---|---|---|---|---|
| 1 | flat asset declaration (family B): `Sole ownership of a residential rental unit in Winnipeg, Manitoba.` — owner self, value NULL | `30c94327-3108-f111-81a2-001dd8b72449` | Rebecca Chartrand (`5b99c2bd-7b2a-f011-8195-001dd8b72449`) | Minister of Northern and Arctic Affairs | Declarable Assets / Act 25(2) | 2026-04-14 | https://ciec-ccie.parl.gc.ca/en/public-registry/Details?declarationId=30c94327-3108-f111-81a2-001dd8b72449 | `4531a973b004a2cbcaf68ebca9df849991614a15fe7fedf2270391bf6ff2a408` |
| 2 | flat liability (family B): `Line of credit with CIBC` — the h1 `($10,000 or more)` + statutory 'but not the amount' page; value NULL is THE test | `a4542986-719d-f011-819d-001dd8b72449` | Stephen Fuhr (`1c26de25-482b-f011-8195-001dd8b72449`) | Secretary of State (Defence Procurement) | Liabilities / Act 25(3) | 2026-05-04 | https://ciec-ccie.parl.gc.ca/en/public-registry/Details?declarationId=a4542986-719d-f011-819d-001dd8b72449 | `c3e9df01f2d1e5c3aa68f5096005ab3853c876e80ac4b31adfe8105be392b61a` |
| 3 | flat outside activity (family B): `Member of the Board of Directors of 3H0 Foundation Society, …` | `39e5bbfe-5a8e-f011-819c-001dd8b72449` | Randeep Sarai (`f30de0ad-2778-e511-bec6-002655368060`) | Secretary of State (International Development) | Outside Activities / Act 25(4) | 2025-10-07 | https://ciec-ccie.parl.gc.ca/en/public-registry/Details?declarationId=39e5bbfe-5a8e-f011-819c-001dd8b72449 | `03061e491fc555f323cb8d928fc9de18a1a0b38a7750fba2f1ae82ee854dcd7a` |
| 4 | summary statement with **divestment block** (family B + strong-block): `Divestment of: Publicly Traded Securities by the establishment of blind trust(s) / by sale` — the only public controlled-asset trace | `e882485d-719d-f011-819d-001dd8b72449` | Stephen Fuhr (`1c26de25-482b-f011-8195-001dd8b72449`) | Secretary of State (Defence Procurement) | Summary Statements (Act) / Act 26(1) | 2026-05-04 | https://ciec-ccie.parl.gc.ca/en/public-registry/Details?declarationId=e882485d-719d-f011-819d-001dd8b72449 | `e631c24d51957d11b9bf2b03806c7771e7c793ea133f3c68b0493fe1c74b1cb4` |
| 5 | itemized MP disclosure summary (family C): 8 sections incl. `Spouse's/Common-Law Partner's assets` + `…income` (owner=spouse rows), per-item GUIDs, numbered-corporation free text (instrument NULL) | `877aeea7-e1b1-4348-bd5d-808c7758fb22` | Jason Groleau (`f0f4e0ff-7b2a-f011-8195-001dd8b72449`) | Member of Parliament (Beauce) | Disclosure Summaries (Code) / Code 23(1) | 2026-06-26 | https://ciec-ccie.parl.gc.ca/en/public-registry/Details?declarationId=877aeea7-e1b1-4348-bd5d-808c7758fb22 | `c95a66fa36c59ee06390c5ea0e45fc231bd54136f1fc3eb1c1c67115f2681485` |
| 6 | itemized material change (family C → **change_notification**): 2 sections (`Activities`, `INVESTMENT IN PRIVATE CORPORATIONS [Paragraph 24(1)(a)]` — uppercase+statute-ref label variant), `Date of change: 2025/07/22` slash-format extraction | `c7bf3da3-9669-f111-81a9-001dd8b72449` | Jason Groleau (`f0f4e0ff-7b2a-f011-8195-001dd8b72449`) | Member of Parliament (Beauce) | Material Changes / Code 21(3) | 2026-06-26 | https://ciec-ccie.parl.gc.ca/en/public-registry/Details?declarationId=c7bf3da3-9669-f111-81a9-001dd8b72449 | `9eb65e0e239169232e4bef76a924c5d731183af499895d7630c869cbbfa60df2` |
| 7 | typed Code gift (family A): `Nature`/`Source`/`Circumstance`/`Gift received date 2026-06-06`, Code h1 variant (`Public Statement of Gifts or Other Benefits`), owner NULL | `3f544e69-d268-f111-81a9-001dd8b72449` | Jennifer McKelvie (`9afbf0c2-172c-f011-8195-001dd8b72449`) | Member of Parliament (Ajax) | Gifts (Code) / Code 14(1) | 2026-06-15 | https://ciec-ccie.parl.gc.ca/en/public-registry/Details?declarationId=3f544e69-d268-f111-81a9-001dd8b72449 | `2b95fa9a9e1f133446317ff8c53ffe02543af539e47239e45935beaa2be2e762` |

All seven filers are inside the v1 role scope (§2.4). Alternates archived but NOT
selected (out-of-scope filers; grammar evidence only): Aziz Gifts (Act) E20
(ministerial staff — proves the Act-gift grammar + h1 variant), Wilkinson asset E45
(ambassador), Tessier activity E46 (GIC appointee), Felizarta boilerplate-only
summary statement E26 (the divestment-less variant — its grammar is a subset of #4).
Rationale: #1–#3 are the goal's asked diversity (asset/liability/activity) on the
simplest grammar; #4 adds the divestment block + controlled-asset trace; #5
exercises per-item rows, spouse-owner sections, and item GUIDs at scale; #6
exercises the `change_notification` record_type, the label-variant grammar, and
slash-date extraction; #7 exercises typed fields + the Code h1 variant + owner NULL.
Together they cover every §3.4 grammar family and every §3.5 owner branch observed
in evidence (Dependent-section and A′ sponsored-travel remain evidence-only — E14 —
both single-variant extensions of already-fixtured grammars). Expected outputs per
automation policy (no human gate): high-confidence extraction + second-model
cross-check, published `unverified`, sampling-audit queue.

## 8. Evidence log (retrieved 2026-07-05; full request/politeness detail in E44)

Archived under `docs/regimes/canada_ciec/evidence/` **in this commit**, sha-named
(`{sha256}.{slug}.html`).

| ID | URL | sha256 / note |
|---|---|---|
| E1 | https://ciec-ccie.parl.gc.ca/en/public-registry | `232501112f4372636d326c657150c289df2ce56ff8fdd2ed4bd0a2699d8e3692` search page: full filter legend (15 types, 6 roles, 2 statuses, report types, date pickers, sort), 8,398 total, first cards |
| E2 | https://ciec-ccie.parl.gc.ca/en/public-registry/cards?page=2&declarationType=3f99bf47-… | `0e8d9f7c1521e6697fd1eab7116d17fb4b38cef30ff105a4bd8f72eb90732fb8` bare card fragment (sweep endpoint) |
| E3 | …/en/public-registry?page=1&disclosureFrom=2026-06-01&disclosureTo=2026-06-30 | `f2690ce4383bd0662cef151fc367e8243ea2eb381684f34340f7bb2923a1045b` date-window sweep works: 86 rows |
| E4 | …/en/public-registry?page=1&sortBy=declarationDisclosureDate&sortDir=asc | `d67f83da7b1e9b8041f758fba43215a569fae4222651222b14199df86adb6095` earliest rows: 2008-05-02 |
| E5 | …?declarationType=c37a52a7-… (Declarable Assets) | `b1e5766c85fac23c1335dc7411bdecd246e17aa8f4072d043863ce0ba27a8afe` 862 rows |
| E6 | …?declarationType=3f99bf47-… (Liabilities) | `993b057ca472a150b87cbfb822bf64327a21b7f0470fae785213065821b2583f` 336 rows |
| E7 | …?declarationType=dd98430c-… (Outside Activities) | `d71076a9d85c23df640b0b355f294899b2b83883397d1b3e1c12e07785d99c64` 875 rows |
| E8 | …?declarationType=acdd6784-… (Summary Statements (Act)) | `8456b3e35a094585e54a0dce9964b48778e82fc430bc26ad0a9dcc220443bd10` 1,910 rows |
| E9 | …?declarationType=5924f660-… (Disclosure Summaries (Code)) | `e029e630019a1172774a7ff34b10aae346bfb500b4c789c63de2c7ccf3c1592e` 1,645 rows |
| E10 | …?declarationType=2da4414a-… (Material Changes) | `f4d8f7bc0152bd11a69c775029c25b19ea118d4c3caf23d983fa897b30b0e74a` 302 rows |
| E11 | …?declarationType=abe9286c-… (Gifts (Act)) | `c9c888029f07214df171710ca3a429e9589a771dd5160ba3e236281d952bb7ad` 1,264 rows |
| E12 | …?declarationType=665b3de7-… (Gifts (Code)) | `1e3cc030a4d7b120e8e8a1e5583df26a1edd758374647741e88aea15b83d7c5e` 308 rows; `Gift received date` on 29/30 cards (nullable) |
| E13 | …?declarationType=453089cd-… (Forfeited Gifts) | `456e07892af03442e11d506c1538e0ff55aa4653130ef37f36392bcff08170ad` 90 rows; same 4 dt labels as gifts |
| E14 | …?declarationType=5eda55e6-… (Sponsored Travel) | `f995cdfb3176ba0419da10ec0ef36dc69c2751b6667322338a78d4eb8d717b44` 347 rows; Destination/Sponsor/Purpose/Dates; `ExternalClass…` SharePoint artifacts |
| E15 | …?declarationType=af4085f1-… (Travel) | `9207e2e2de668a79215332968c06b690998a45f73de49890883d06a3ff6db3b0` 39 rows |
| E16 | …?declarationType=9c59f81f-… (Compliance Measures) | `9a85bea6240500e540b594a15df219749ff8ecabc0381c6418bc9418bd83a66b` 123 rows; CoI screens naming private third parties (out of scope; redaction claim) |
| E17 | …?declarationType=94b3d6f8-… (Recusals) | `a6a20c3b47ec884ae2a14502d47670d8f801c2ba0511918f11539636e5e24251` 173 rows |
| E18 | …?declarationType=76e97e8a-… (Private Interest) | `9207c2f4e2f50984bc705edc4af93a1b2bb615d1f9f01ce73f82e3eebb1d99d6` 6 rows |
| E19 | …?declarationType=5bc55a37-… (Other Appropriate Documents) | `b6ca393b1bec69a33b6fd892d6fed1ff762d27e6bdd106ada3a2c6f98c68ff54` 20 rows |
| E20 | …/Details?declarationId=115dc125-a574-f111-81ab-001dd8b72449 | `89d1790197e103e45f261d7ac39c58cd9ba8329b8c6fdf2f6eba6127d5534af8` Gifts (Act) grammar + statutory 25(5)/51(1)(a) box (filer = ministerial staff → grammar evidence, not fixture) |
| E21 | …/Details?declarationId=3f544e69-d268-f111-81a9-001dd8b72449 | `2b95fa9a9e1f133446317ff8c53ffe02543af539e47239e45935beaa2be2e762` (fixture #7, Gifts (Code), Code 14(1) box) |
| E22 | …/Details?declarationId=30c94327-3108-f111-81a2-001dd8b72449 | `4531a973b004a2cbcaf68ebca9df849991614a15fe7fedf2270391bf6ff2a408` (fixture #1, Declarable Assets, Act 25(2) box) |
| E23 | …/Details?declarationId=a4542986-719d-f011-819d-001dd8b72449 | `c3e9df01f2d1e5c3aa68f5096005ab3853c876e80ac4b31adfe8105be392b61a` (fixture #2, Liabilities, Act 25(3) 'but not the amount' box) |
| E24 | …/Details?declarationId=39e5bbfe-5a8e-f011-819c-001dd8b72449 | `03061e491fc555f323cb8d928fc9de18a1a0b38a7750fba2f1ae82ee854dcd7a` (fixture #3, Outside Activities, Act 25(4) box) |
| E25 | …/Details?declarationId=e882485d-719d-f011-819d-001dd8b72449 | `e631c24d51957d11b9bf2b03806c7771e7c793ea133f3c68b0493fe1c74b1cb4` (fixture #4, Summary Statement + divestment block, Act 26(1) box) |
| E26 | …/Details?declarationId=fbc2a000-863a-f111-81a7-001dd8b72449 | `4a68d19893d4238fb76b90b491008882f7b23e9ac7675b7208b66f8b98712c74` boilerplate-only Summary Statement variant (empty title-div quirk) |
| E27 | …/Details?declarationId=877aeea7-e1b1-4348-bd5d-808c7758fb22 | `c95a66fa36c59ee06390c5ea0e45fc231bd54136f1fc3eb1c1c67115f2681485` (fixture #5, itemized Disclosure Summary, Code 23(1) box, spouse sections, item GUIDs) |
| E28 | …/Details?declarationId=c7bf3da3-9669-f111-81a9-001dd8b72449 | `9eb65e0e239169232e4bef76a924c5d731183af499895d7630c869cbbfa60df2` (fixture #6, Notice of Material Change, Code 21(3) box, Date of change 2025/07/22, wrapper GUID id) |
| E29 | …/Details?declarationId=8eaa8c32-d964-f111-81a9-001dd8b72449 | `59a6001f6b5251d74c25c03d002868c85fe63c5055157180e5a3c520443940e8` NLA details page: `No Longer Applicable` warning badge + `OCIEC Translation` info badge |
| E30 | https://ciec-ccie.parl.gc.ca/fr/registre-public/Details?declarationId=115dc125-… | `56ae2e89593964be682b71b45f421805becc232e22913b256c0de14312ad2a7f` FR mirror of E20: same GUID, translated labels + content |
| E31 | https://ciec-ccie.parl.gc.ca/fr/registre-public | `780c9fec799bac0decc93c67f758fbcf6508143286dd40927723c063e55c6f78` FR list: filter GUIDs identical to EN |
| E32 | https://ciec-ccie.parl.gc.ca/en/client?clientId=4f8c0d03-57a4-ec11-8155-001dd8b72449 | `739c01033565654c62b46b47a91651367ef2c50c0ba4fa1176b319c89986cc2c` client profile: affiliations with offices + tenure ranges; per-client declaration list |
| E33 | https://ciec-ccie.parl.gc.ca/en/client?clientId=f0f4e0ff-7b2a-f011-8195-001dd8b72449 | `b55527b54b99af1892d41e7c085b1b2c4bee4f584867b8b6ba5f7473ebf3a009` MP client profile: party + Constituency (Beauce) + start date |
| E34 | https://ciec-ccie.parl.gc.ca/en/profile | `1c45781053083bac6796dedca77659306899bc61550e52c196028fcc5497aa7a` profile search: role filter + compliance-status filter (roster enumeration surface) |
| E35 | …/en/public-registry?declarationStatus=6e4f9ba2-… | `de371a4f6b36dffab929a72c5a3fa96473482cd181a79578ad238c7609b88aef` 2,461 NLA rows; bg-warning badge on cards |
| E36 | …?declarationType=c37a52a7-…&affiliationRole=c8c94a19-… | `5ddfe5075e85ebc3a0ce14faa1c79284da53255a0a40e4f736abf15b71d450e1` role filter works: ministers' declarable assets = 114 |
| E37 | …?declarationType=dd98430c-…&affiliationRole=cac94a19-… | `063851fd1d51880cc0b987cfa2ded977823bdfa7b68265fba235847d9762c733` MPs' outside activities = 0 (Act-only type) |
| E38 | …?declarationType=dd98430c-…&affiliationRole=c8c94a19-… | `d16f699bf4a769ccfb49945116f63111299ca0b5e923d6aab474c0791f2b2807` ministers' outside activities = 16 (fixture #3 sourced here) |
| E39 | …?declarationType=acdd6784-…&affiliationRole=c8c94a19-… | `fdc4c9054425cbd9752bc1fda66036bed24a7b09a4ba5ee90ade6b1b2031aaa9` ministers' summary statements = 82 (fixture #4 sourced here) |
| E40 | https://ciec-ccie.parl.gc.ca/en/important-notices | `757425ede36c2fb8fccae2ab34ded1f2217827f94ec26cb87843717239dba3f2` two-regime structure (~3,000 Act / 343 Code), registry purpose, TEMPORARY-website transition notice |
| E41 | https://ciec-ccie.parl.gc.ca/robots.txt | `a85a1098188b68ef63f5232f3d1ee8a7b76cac41f737adc5968ab09d6c833d44` catch-all HTML (no robots policy served) |
| E42 | https://ciec-ccie.parl.gc.ca/ | `4d3e7e9fd7a6d0158b2a98d387d86dcc7c41a7d690a6419e3135619faf31d193` bilingual splash (EN/FR entry) |
| E43 | https://ciec-ccie.parl.gc.ca/swagger/v1/swagger.json | `5da82440799a585648c6545efe0dd3b36048ab793e5d13a3d6b281960c7c5fec` 404 HTML — no documented API (tried-log) |
| E44 | (our record) | `2026-07-05-ciec-registry.retrieval.json` — every request (48 total: 47×200, 1×404 swagger probe), client config, politeness stats, byte-stability variance tests, api probes |
| E45 | …/Details?declarationId=be05dcef-5166-f111-81a9-001dd8b72449 | `9a9ae8bec70fcd290ea20fbaf26ff3b5b407932ab318196c9cfcfce5d06bc6af` alternate flat-asset details (Wilkinson, ambassador — out-of-scope filer; grammar evidence + variance-test subject) |
| E46 | …/Details?declarationId=59a77521-a570-f111-81ab-001dd8b72449 | `a1f261d9ebba8fd764374b050e42bad5fb5347a68d733a16e3b4f68e66bf0e1b` alternate outside-activity details (Tessier, GIC appointee — out-of-scope filer; grammar evidence) |

## Quirks log (append-only, dated)

- 2026-07-05 · The site is an officially **temporary website** in phased transition
  to ethicscanada.ca (E40) — layout drift is EXPECTED; the sentinel's top watch item
  for this adapter. Grammar breaks freeze first, then re-archive (§6.4).
- 2026-07-05 · robots.txt is a catch-all HTML route (200 with the home page, E41) —
  no robots policy exists to follow; self-imposed politeness governs.
- 2026-07-05 · NO ETag/Last-Modified anywhere; no cookies set on any public page
  (E44). Conditional GETs impossible; `disclosureFrom` windows are the incremental
  primitive (contrast us_house).
- 2026-07-05 · `/en/public-registry/cards` serves the bare card fragment with the
  same query params (E2) — sweep with it, parse details pages only.
- 2026-07-05 · Act vs Code gift pages differ in h1 (`…Gifts or Other Advantages` vs
  `Public Statement of Gifts or Other Benefits`, E20/E21) — the h1↔type integrity
  table (§3.10) is per-law.
- 2026-07-05 · MPs file ZERO standalone Outside Activities declarations (E37) —
  that type is Act-only; MP substance arrives via itemized Disclosure Summaries.
- 2026-07-05 · Itemized types carry **per-item stable GUIDs** and section labels
  (E27/E28); flat types carry neither. Section labels vary in case and may append a
  statute reference (`INVESTMENT IN PRIVATE CORPORATIONS [Paragraph 24(1)(a)]`,
  E28) — match case-insensitively with optional `[…]` suffix.
- 2026-07-05 · Two date formats in one source: dd/footer dates are ISO `YYYY-MM-DD`;
  material-change item text uses `Date of change: YYYY/MM/DD` (slashes, E28).
- 2026-07-05 · `Gift received date` is nullable (29/30 on E12).
- 2026-07-05 · Legacy SharePoint migration artifacts (`ExternalClass…` wrapper divs,
  E14) appear inside dd content — strip to text, never key on them.
- 2026-07-05 · Status is a display flag: `No Longer Applicable` adds a bg-warning
  badge on cards AND the details page (E29/E35) — the details URL's bytes MUTATE on
  flip, with no detection channel (E1). §3.8 governs.
- 2026-07-05 · `OCIEC Translation` bg-info badge (E29) marks office-translated
  content — provenance for the bilingual rule (§3.9).
- 2026-07-05 · Sum of the 15 type counts (8,300) ≠ unfiltered total (8,398): ~98
  rows are presumably the `declarationReportType` documents (open question).
- 2026-07-05 · Full-registry sweep cost: 280 pages at ≥2 s ≈ 10 min — a periodic
  full re-sweep (status-flip detection) is cheap enough to schedule later; kept OUT
  of the v1 green path (§2.3).
- 2026-07-05 · Fixed GUIDs everywhere (Dynamics-CRM-style): declaration types,
  roles, statuses, persons (clientId), declarations, items — join on GUIDs, display
  labels.

## Operational notes (politeness incidents, outages)

- 2026-07-05 · 48 requests total this task (E44): 47×200 + 1×404 (deliberate swagger
  probe). Concurrency 1, ≥2.2 s enforced spacing, identified UA + From on every
  request, zero 429s, zero challenges, zero throttling observed.
- 2026-07-05 · Byte-stability variance test: within-session re-GETs of a flat-asset
  details page (E45) and fixture #5 (E27) byte-identical (2/2, E44) — first
  confirmation for the §7 pinning rule; cross-session confirmation falls to the
  capture leg.
