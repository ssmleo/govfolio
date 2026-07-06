---
# RegimeSurvey front-matter (validated shape). COMPOUND doc — THREE regimes under one
# adapter (§0). Every claim: {claim, evidence:[files]}. Shared fields at top; per-regime
# maps under `regimes:`. Primary-block scalars describe the compound as a whole.
compound: true
adapter: "eu_fr_de_annual"
jurisdiction: "eu"            # primary block = EU Parliament; fr/de under regimes:
bodies: ["European Parliament", "France — Haute Autorité pour la transparence de la vie publique (HATVP)", "Deutscher Bundestag"]
record_types: [interest]
value_precision: "exact"      # all three current-state exact (DE historical banded — §DE)
regime_type: "periodic_declaration"   # design §5.5 Tier 3 "Annual declarations (EU-P, FR, DE)" — FIRST periodic_declaration regimes
language: [en, fr, de, mul]   # EU multilingual (24), FR fr, DE de
legal_basis:
  claim: "THREE distinct start-of-mandate private-interest declaration regimes, each its own disclosure_regime row (§0). EU: Annex I to the EP Rules of Procedure — Code of Conduct for Members regarding integrity and transparency, Art. 4(2)(a-d) (the DPI form reprints these verbatim, E-EU3/E-EU2). FR: lois n2013-906/907 du 11 octobre 2013 (transparence de la vie publique); DIA scope + open-data reusability + the PATRIMONY reuse ban anchored on Art. LO 135-2 code electoral (E-FR-law). DE: Elfter Abschnitt Abgeordnetengesetz (AbgG) §§45,47,48 + Verhaltensregeln Anlage 1 GO-BT; exact-amount publication since the Transparenzgesetz of 8 Oct 2021 (E-DE1)."
  evidence: ["E-EU2", "E-EU3", "E-FR-opendata", "E-FR-law", "E-DE1", "E-DE2"]
who_files:
  claim: "EU: every MEP (~705). FR (v1 scope): national-level filers — deputes (2534 rows), senateurs (2119), gouvernement/ministres (131); French MEPs (europe, 328) and ~9,000 local officials are OUT of v1 politician rosters (§FR.2). DE: every Mitglied des Bundestages / MdB (~630, 21. Wahlperiode)."
  evidence: ["E-FR-liste", "E-EU-list", "E-DE-bio"]
formats: [open_data_xml, pdf_text, server_rendered_html]
access: {method: "EU: anonymous HTTPS GET of per-MEP DPI PDFs on www.europarl.europa.eu (served). FR: anonymous HTTPS GET of the HATVP open-data CSV index + per-declaration XML (served, Etalab open licence). DE: per-MdB HTML behind an Enodia (Radware) proof-of-work bot gate — browser-engine seam required (§DE.2).", session_required: false, captcha: "EU none; FR none; DE Enodia POW challenge to ALL non-browser clients (E-DE-block)"}
personal_data_to_redact: ["FR: the source itself redacts declarant private fields in-band as '[Donnees non publiees]' (E-FR-dia). EU: sections (E)/(F) may name third parties; FR activProfConjointDto names the spouse's employer. DE: contract-partner names may be anonymised by the member ('Kunde 1, Baugewerbe', E-DE1). Keep verbatim in Bronze/Silver; default to NOT indexing non-politician names (product/legal decision, flagged)."]
tos_and_politeness:
  claim: "EU + FR served the identified UA 200 on every request; DE bot-gated (browser-engine seam). Concurrency 1, >=2s (>=3s europarl per its robots crawl-delay). FR merged XML carries Last-Modified (conditional GET). Full log E-RET."
  evidence: ["E-RET"]
open_questions:
  - {question: "Currency enum (core::domain::enums::Currency) has only EUR/GBP/USD — EU MEPs from non-euro states declare in national currency (Adamowicz: PLN, E-EU2). v1 fail-closed: non-EUR value -> NULL + review_task 'unmapped_currency', raw amount+currency kept. RECOMMEND extending Currency to the EU national currencies (PLN,HUF,SEK,DKK,CZK,RON,BGN) — a snapshot-visible core change, founder/core decision.", tried: ["confirmed PLN on a real term-10 DPI (E-EU2); EUR confirmed on another (E-EU-aftias)"]}
  - {question: "DE per-MdB disclosure fragment: the 'Veroeffentlichungspflichtige Angaben' block loads via a dynamic AJAX fragment behind the Enodia gate; the exact fragment endpoint + HTML grammar were NOT captured (reader returned page chrome only, E-DE-member). Build leg discovers them via the browser-engine seam.", tried: ["reader on a member biografien page returned nav chrome, not the disclosure fragment (E-DE-member); rules doc gives the authoritative published FORMAT (E-DE1)"]}
  - {question: "DE historical 10-Stufen intermediate boundaries (backfill of 18./19. WP) — the 2013 announcement (E-DE2) pins the endpoints (1.000 / 3.500 / 250.000); the full 10-tier table (§DE.6) is corroborated by WebSearch, NOT yet from an archived pre-2021 Bundestag legend. Build/backfill leg pins a webarchiv snapshot before ingesting banded terms.", tried: ["2013 endpoints archived (E-DE2); intermediate tiers via WebSearch only"]}
  - {question: "EU DPI discovery completeness: is there a machine-readable INDEX of all MEP DPI links, or must we crawl /meps/en/{id}/{NAME}/declarations per MEP? The data.europarl.europa.eu open-data portal (403 to plain client) may expose a MEP dataset — not pursued this leg.", tried: ["full-list/all gives MEP ids (E-EU-list); declarations tab per MEP embeds the DPI link (E-EU-decltab); portal host 403'd (E-EU-portal403)"]}
  - {question: "FR DIA remuneration multi-year semantics: each item carries montant per year (E-FR-firmin: 2017..2022). Gold value = latest declared year's exact amount (low==high); all years ride details.montants. Confirm no cross-year double-count at promotion.", tried: ["firmin/pannier show 3-6 year arrays of exact euros (E-FR-firmin, E-FR-pannier)"]}
  - {question: "FR modificative (dim/diam/dspm) supersession: items carry motif CREATION|MODIFICATION|SUPPRESSION (E-FR-dia); a modificative references the prior declaration how? version-keyed filing per §0.1; deterministic linkage across dia->diam not yet mapped.", tried: ["pannier dia is declarationModificative=true with 14 CREATION motifs (E-FR-pannier); SUPPRESSION/MODIFICATION variants unobserved in samples"]}
regime_versions:
  - {effective_from: "2021-10-19", change: "DE: Transparenzgesetz 8 Oct 2021 replaces the 10-Stufen bands with exact euro/cent publication; Beteiligungen threshold 25%->5% (E-DE1)", evidence: ["E-DE1"]}
  - {effective_from: "2023-11-01", change: "EU: reformed Code of Conduct (Annex I) — the DFI (Declaration of Financial Interests, income brackets) becomes the DPI (Declaration of Private Interests, free 'Income amount' field), sections A-G (E-EU3)", evidence: ["E-EU3"]}
  - {effective_from: "2013-01-01", change: "DE: 10-Stufen banded system introduced (in force ~2013->2021; historical backfill only, §DE.6)", evidence: ["E-DE2"]}
regimes:
  eu_parliament_dpi: {jurisdiction: "eu", body: "European Parliament", value_precision: "exact", format: "pdf_text (multilingual form)", record_types: [interest]}
  fr_hatvp_dia:       {jurisdiction: "fr", body: "HATVP", value_precision: "exact", format: "open_data_xml", record_types: [interest]}
  de_bundestag:       {jurisdiction: "de", body: "Deutscher Bundestag", value_precision: "exact", format: "server_rendered_html (bot-gated)", record_types: [interest]}
---

# EU Parliament DPI + France HATVP + Germany Bundestag — Annual Declarations — Source Authority File

> **Internal context; the public methodology page derivation requires founder review
> (residual human lane: methodology PUBLIC copy).** Goal 064 leg A (spec). Written
> BEFORE any adapter code, per the adapter template (design §5.1, plan Task 8) and the
> uk_commons_register / canada_ciec / australia_register pattern
> (`docs/regimes/uk_commons_register.md`, `docs/regimes/canada_ciec.md`,
> `docs/regimes/australia_register.md`).

Scope: govfolio's **Tier-3 "annual declarations" epoch-1 seeds** (design §5.5, §5.7;
EPOCHS E1). This ONE adapter (`eu_fr_de_annual`) covers **three distinct regimes** —
the European Parliament **Declaration of Private Interests (DPI)**, the French **HATVP
déclaration d'intérêts et d'activités (DIA)**, and the German **Bundestag
veröffentlichungspflichtige Angaben (Nebentätigkeiten)** — because the goal's single
acceptance command dispatches by one adapter name (§0). They are govfolio's **first
`periodic_declaration` regimes** (design regime_type vocab; contrast the Tier-2
change-notification registers UK/CA/AU). Every row is `record_type = interest`
(existence of an activity / holding / income, no transaction, no valued snapshot).

Evidence citations `E-*` refer to §9. All retrievals 2026-07-05, identified UA
`govfolio.io research (contact: ssm.leo@outlook.com)` + `From:` header, concurrency 1,
≥2 s spacing (≥3 s europarl). **`www.bundestag.de` is Enodia-bot-gated to all non-browser
clients** (§DE.2): DE content was obtained via a browser-engine reader and DE canonical
raw-byte pins are deferred to the capture/build leg (australia_register precedent).
Everything is archived under `docs/regimes/eu_fr_de_annual/evidence/{eu,fr,de}/` **in this
same commit**, sha-named (`{sha256}.{slug}.{ext}`).

Per automation policy (`docs/decisions/automation-policy.md`), the goal's "HUMAN
completes expected.*.json" step is superseded: the test-designer authors expecteds
independently (FR: deterministic XML parse; EU: schema-constrained vision extraction +
second-model cross-check; DE: HTML parse via the browser-engine seam), records publish
`unverified`, sampling-audit queue.

---

## 0. Architecture — one crate, three source sub-adapters, three regime rows

**Decision: (b)+(c) hybrid — ONE adapter crate `eu_fr_de_annual` registering the single
conformance name, with THREE internal source sub-adapters (`src/eu`, `src/fr`, `src/de`),
producing THREE `disclosure_regime` rows and THREE Silver tables / details-schema sets.**
NOT (a) "one parser with source modes": the three parse inputs share *nothing* — a
structured XML feed, a multilingual PDF form, and bot-gated HTML — so a single parser with
modes would be three parsers wearing a trench coat. NOT (c) "three separately-registered
adapters": the goal's acceptance is the single command
`cargo run -p pipeline --bin conformance -- eu_fr_de_annual`, so there is exactly one
adapter registration and one fixtures root.

Rationale, concretely:
- **Conformance dispatch.** The harness resolves the ONE adapter name `eu_fr_de_annual`,
  then iterates fixtures under `crates/adapters/eu_fr_de_annual/fixtures/<source>_<case>/`
  where `<source>` ∈ `{eu, fr, de}` encodes which sub-adapter parses it (mirrors the goal's
  note). The crate's `Adapter` impl reads the source discriminant from the FilingRef /
  fixture directory and dispatches `discover`/`fetch`/`parse` to the matching sub-module.
- **Three regimes, not one.** Three `disclosure_regime` rows are seeded (slugs
  `eu_parliament_dpi`, `fr_hatvp_dia`, `de_bundestag`; jurisdictions `eu` supranational,
  `fr`, `de`; all `regime_type = periodic_declaration`; all `value_precision = exact`).
  Each row is what the Gold `regime_id` points at, what `/regimes` scorecards, and what
  the details schemas are keyed on — they must be distinct.
- **Three Silver tables** (`stg_eu_parliament_dpi`, `stg_fr_hatvp_dia`,
  `stg_de_bundestag`) and **three details-schema sets** (`§EU.5`, `§FR.5`, `§DE.5`) —
  the shapes are genuinely different; one table would be a union-typed mess.
- **Shared spine.** The sub-adapters share the crate's politeness client config, the
  `record_type = interest` mapping discipline (§0.1), the value→ValueInterval rules
  (§0.1), the fingerprint/promotion machinery, and this SAF. That shared spine is the
  reason they live in ONE crate rather than three.

Directory layout the test-designer + builder align to:
```
crates/adapters/eu_fr_de_annual/
  src/lib.rs                 # Adapter impl: Source enum + dispatch
  src/eu/{mod,fetch,parse,details}.rs
  src/fr/{mod,fetch,parse,details}.rs
  src/de/{mod,fetch,parse,details}.rs
  fixtures/eu_<case>/  fr_<case>/  de_<case>/   # source encoded in the dir name
crates/pipeline/schemas/details/
  eu_parliament_dpi.interest.json
  fr_hatvp_dia.interest.json
  de_bundestag.interest.json
```

### 0.1 Shared conventions (all three sources)

- **record_type = `interest`, always.** These are start-of-mandate/periodic declarations
  of the *existence* of activities, holdings and income — no buy/sell (`transaction`
  needs `side`+`transaction_date`), no valued point-in-time snapshot (`holding` needs
  `as_of_date`, `gold.rs`). `GoldCandidate::validate()` requires nothing extra for
  `Interest`. EU section (D) "holdings" and FR `participationFinanciereDto` are holdings in
  the plain sense but carry no `as_of_date` → `interest`, not `holding` (UK/CA/AU
  precedent).
- **regime_type = `periodic_declaration`** (design §4.2 vocab; Tier 3). The base
  obligation is a per-mandate declaration; EU "update on each change" and FR modificatives
  (`dim`/`diam`) are handled as **version-keyed filings** (`{external_id}@{version}`, UK
  precedent, §0.1 filing), not as a `change_notification` regime.
- **value → ValueInterval.** EUR amounts → `ValueInterval` with `low == high` (exact;
  `value.rs`). FR: latest declared year's `montant` (exact €). EU: the `Income amount`
  figure (exact) — periodicity (`monthly`/`quarterly`/`annual`) rides `details`, never
  annualised into the value (that would invent a number). DE: the exact euro/cent figure.
  **Non-EUR EU currencies** (PLN etc., E-EU2) → `value = NULL` + `review_task
  "unmapped_currency"`, amount+currency kept verbatim in `details`/`asset_description_raw`
  (front-matter open question; UK "unmapped currencyCode → review" precedent). Qualitative
  rows (memberships, "None"/`neant`) → `value = NULL`.
- **Multilingual (raw is sacred).** `asset_description_raw` is stored in the ORIGINAL
  filing language — FR French, DE German, EU the MEP's own language (any of 24; a single
  DPI can be entirely Polish, E-EU2). `details.language` records it (`fr`/`de`/ISO code /
  `mul`). No translation on the green path (invariant 2).
- **Instrument = NULL at parse.** Companies/funds are free text (FR `nomSociete`, EU/DE
  entity names) → below-threshold by default ⇒ `instrument_id` NULL + review per the
  resolution waterfall (invariant 3).
- **asset_class stays honest.** `equity`/`private` only where the record is genuinely a
  company/partnership holding (EU (D), FR `participationFinanciere`, DE Beteiligungen);
  everything else `other`. No creative bucketing (UK/CA/AU precedent).
- **owner.** Default `self`. FR `activProfConjointDto` (spouse's professional activity) →
  `spouse` (the one non-self branch across the three). EU/DE have no spouse columns → `self`.
- **filing.** `filing_type = "declaration"`; `published_at` = the source publication date
  at `00:00:00Z` (date-precision convention); `supersedes_filing_id` = deterministic
  `{external_id}@{version-1}` lookup when the prior version was observed, else NULL.
- **Fail closed** (invariant 6): an unknown EU section letter, FR `typeDeclaration`/section
  tag, or DE category heading outside the archived vocabulary FREEZES that sub-adapter +
  opens a review_task. Zero rows from a fetched non-empty document freezes; an empty
  discovery sweep is a normal quiet period, not a freeze.

### 0.2 Legal / residual-human flags (READ THIS)

- **FR patrimony declarations (`dsp`/`dspm`/`dspfm`) are OUT of v1 scope — legal.**
  Art. LO 135-2 code électoral: déclarations d'intérêts et d'activités (`dia`) of MPs are
  *published and freely reusable* (Etalab open licence), but the *déclaration de situation
  patrimoniale* is consultation-only and **republishing all or part of it is an offence
  punishable by a €45,000 fine** (E-FR-law). govfolio republishes → it ingests **only
  `dia`** (and `dim`/`diam` modificatives), NEVER `dsp*`. The 2,625 `dsp*` rows and the
  577 "publication en préfecture à venir" rows in the index (E-FR-liste) are excluded at
  discovery. This is a hard scope boundary + a residual-human/legal flag.
- **Public methodology copy** derived from this SAF stays in the residual human lane
  (pricing/legal/methodology PUBLIC copy — root CLAUDE.md).

---

## §EU — European Parliament, Declaration of Private Interests (DPI)

### EU.1 Regime metadata
| Field | Value |
|---|---|
| jurisdiction | `eu` (supranational) |
| body | `European Parliament` |
| regime slug | `eu_parliament_dpi` |
| regime_type | `periodic_declaration` — filed by end of the first part-session after elections, or within 30 days of taking office, and updated by end of the month after each change (form header, E-EU3) |
| value_precision | `exact` — the reformed DPI (2023) uses a free `Income amount` field; real filings carry exact figures + a periodicity (E-EU2 PLN, E-EU-aftias EUR). The prior DFI's income BRACKETS are gone (regime_versions) |
| cadence | tier 3: daily discovery check, bulk on publication (design §5.5) |
| source_url | https://www.europarl.europa.eu/meps/en/full-list/all |
| document endpoint | `GET /erpl-app-public/mep-documents/DPI/{term}/{uuid}_{ts}.pdf` — one PDF per MEP per term; the canonical Bronze document |
| declarations tab | `GET /meps/en/{id}/{NAME}/declarations` — embeds the current-term DPI link (E-EU-decltab) |
| currency | EUR contextually, BUT MEPs may declare in national currency (PLN observed, E-EU2) — §0.1 unmapped-currency rule |

### EU.2 Discovery
**Data-source: per-MEP DPI PDFs on `www.europarl.europa.eu` (served to the identified
client).** The open-data PORTAL host `data.europarl.europa.eu` 403s the plain client
(E-EU-portal403) and is not used. `robots.txt` (E-EU-robots) sets `Crawl-delay: 3` (for
UA `008`) and disallows `/meps/*/pdf` + `/meps/*/xml` (the on-the-fly list generators) but
NOT the `/erpl-app-public/...DPI...` document path.

Algorithm:
1. `GET /meps/en/full-list/all` → MEP ids (E-EU-list). (Or per-term full lists.)
2. Per MEP: `GET /meps/en/{id}` (302 → `/{NAME}/home`) then
   `GET /meps/en/{id}/{NAME}/declarations` → scrape the `DPI/{term}/{uuid}_{ts}.pdf` href
   (E-EU-decltab; both current-term and prior-term DPIs are listed).
3. `fetch` the DPI PDF once per `{uuid}` (or per `{ts}` — the filename timestamp is the
   version, §EU.2 filing) → Bronze (sha256, invariant 2).
4. Politeness: concurrency 1, ≥3 s spacing (honour the robots crawl-delay), identified UA
   + `From:`; served 200 throughout (E-RET). No conditional-GET validator probed this leg.

**Filing / version:** `external_id = "{uuid}"` (the DPI document id); `version` = the
filename `{ts}` millis (a reissue = new timestamp = new version). `published_at` = the
declaration `date` field at 00:00Z. Politician resolution: MEP id (E-EU-list) is the stable
join key; the form prints Surname/First name (E-EU3). Zero/multi roster hits ⇒ fail closed
(`unresolved_filer`).

### EU.3 Document anatomy (the Bronze document; E-EU3, E-EU2, E-EU-aftias)
A single PDF form, **generated in the MEP's chosen language** (E-EU3 English; E-EU2 fully
Polish — headers included). Fixed sections keyed on the **Code of Conduct article
reference** (`Art. 4(2)(a-d)`, language-stable Latin letters) not on the localized header
text:

| Section | Art. | Content | Table columns |
|---|---|---|---|
| **(A)** | 4(2)(a) | Occupation(s) in the 3 years before office + board/committee memberships in that period | Occupation/Membership · [None] · Income amount · Nature of benefit · Periodicity |
| **(B)** | 4(2)(b) | Remunerated outside activity where total > **EUR 5 000 gross/calendar year** | Field/nature+entity · Income amount · Nature · Periodicity |
| **(C)** | 4(2)(c) | Board/committee membership or other outside activity | Membership/Activity · [None] · Income amount · Nature · Periodicity |
| **(D)** | 4(2)(d) | **Holding** in a company/partnership with public-policy implications OR giving significant influence | Holding(policy) · Holding(influence) · [None] · Income amount · Nature · Periodicity |
| **(E)** | Rule 35a(4) | Third-party support (financial/staff/material) for political activities — third-party identity disclosed | free text (names third parties) |
| **(F)** | — | Other direct/indirect private interests | free text |
| **(G)** | — | Additional information | free text |
| footer | — | `date: DD/MM/YYYY` + publication statement | |

Value evidence (E-EU2, Adamowicz term-10, PL): `4940 PLN — miesięcznie`, `5062 PLN —
kwartalnie`, `17608 PLN — miesięcznie` (rental), plus `Brak`(=None) rows and `informacja
publiczna` (the MEP salary). E-EU-aftias: `11000 EUR`, `50 EUR`. → **amount + currency +
periodicity, exact.**

### EU.4 Silver — `StagingRow` (stg_eu_parliament_dpi)
One StagingRow per non-empty declared line (per section row). Source-faithful, verbatim,
original language.
| Field | Type | Req | Content |
|---|---|---|---|
| `dpi_uuid` | string | yes | document id (threaded from the URL) |
| `version_ts` | string | yes | filename `{ts}` (version) |
| `mep_id` | integer | yes | EP MEP id (resolution) |
| `surname_raw` / `first_name_raw` | string | yes | form header verbatim |
| `parliamentary_term` | integer | yes | `{term}` from the URL (e.g. 10) |
| `section` | string enum `A`..`G` | yes | article-keyed (§EU.3) |
| `row_ordinal` | integer ≥1 | yes | 1-based within the document |
| `entry_text_raw` | string | yes | occupation/activity/membership/holding text, verbatim (original language) |
| `income_amount_raw` | string\|null | yes | `Income amount` cell verbatim (`4940 PLN`, `11000 EUR`, `informacja publiczna`, null) |
| `benefit_nature_raw` | string\|null | yes | `Nature of the benefit` cell |
| `periodicity_raw` | string\|null | yes | `Periodicity` cell (`monthly`/`miesięcznie`/…) |
| `none_flag` | boolean | yes | the `None`/`Brak`/`X` no-income marker |
| `declaration_date_raw` | string | yes | footer `date` (`DD/MM/YYYY`) |
| `lang` | string | yes | detected form language (ISO 639-1 or `mul`) |
| `confidence` | number [0,1] | yes | §EU.6 |
| `extractor` | string | yes | `eu_parliament_dpi/llm@1` |

### EU.5 details contract — (eu_parliament_dpi, interest)
`EuParliamentDpiInterestDetailsV1`, snapshot at
`crates/pipeline/schemas/details/eu_parliament_dpi.interest.json`.
Fields: `dpi_uuid`, `version_ts`, `mep_id`, `parliamentary_term`, `section` (A..G),
`row_ordinal`, `entry_text`, `income_amount_raw` (string\|null, verbatim),
`income_currency` (string\|null — parsed ISO code; `null` when non-numeric),
`periodicity` (string enum `monthly`\|`quarterly`\|`annual`\|`one_off`\|`other`\|null),
`benefit_nature` (string\|null), `declaration_date` (date), `language` (string),
`value_source` (enum `income_amount`\|`none`\|`unmapped_currency`).
Gold mapping: `record_type=interest`; `asset_description_raw=entry_text` verbatim
(empty ⇒ reject); `asset_class`= `private`/`equity` for section (D), else `other`;
`owner=self`; `notified_date`=`declaration_date`; `value`= exact `income_amount` when EUR
(low==high EUR), else NULL (+`unmapped_currency` review for a parsed-but-unlisted currency).

### EU.6 Extraction strategy (spec-writer exclusive)
**LLM-vision-first** (design §5.3 rule 2). Justification: the DPI is a **tabular PDF form
in any of 24 languages** — a deterministic parser keyed on English column headers breaks on
the Polish/French/German variants (E-EU2 is entirely Polish). The table STRUCTURE and the
`Art. 4(2)(x)` section keys are language-stable, so the prompt encodes the A-G semantics by
article reference + column position; the tool `input_schema` IS the §EU.4 StagingRow schema
(forced tool use, re-validated locally; schema-invalid ⇒ fail closed). PDFs carry a clean
text layer (E-EU3), so a `pdftotext -layout` pass is a cheap deterministic **cross-check**
(never the sole source). Confidence: start at vision wrapper; −0.05 income present but
unparseable/non-EUR; −0.02 language auto-detect low-confidence. Cross-check on impact
(watchlist) per design §5.3. Cache by sha (per document version).

### EU.7 Fixtures (≥3; raw-byte sha pins — served host, real bytes)
Pinning rule: sha256 of the raw DPI PDF response bytes for the `{uuid}_{ts}` version; a
reissue = new `{ts}` = new version (keep archived bytes, fixture the new version as an added
case). Content-normalized fallback: sha256 over ordered StagingRow tuples
`(section,row_ordinal,entry_text,income_amount_raw,periodicity_raw)`.
| # | Case | MEP | Term | sha256 (raw bytes) | URL |
|---|---|---|---|---|---|
| eu-1 | **EUR exact** income (`11000 EUR`, `50 EUR`) → ValueInterval exact; the euro-zone baseline | Georgios Aftias (256820) | 10 | `aca0b7327e3aebf36e75daf7997b7979c2d8f281b7b5a5e3c62121513965adbb` | .../DPI/10/c8d42e82-1191-49d7-864e-52631d292544_1721085706142.pdf |
| eu-2 | **non-EUR + multilingual**: fully Polish form; exact PLN amounts + `miesięcznie`/`kwartalnie` periodicity; a holding in (D); rental income → `unmapped_currency` value NULL, holding/self, LLM multilingual seam | Magdalena Adamowicz (197490) | 10 | `098b372beb85049b5cae2531abbfe8545b6f11e0c295af126500fc914bbffe07` | .../DPI/10/37b6d6f9-8d23-4d6c-8dd0-14bb5472a06e_1720532466901.pdf |
| eu-3 | **English baseline / grammar reference**: term-9 English form, sections A-G, mostly `None` + MEP-salary "public information" → the empty/qualitative case | Maria Spyraki | 9 | `317354d259f28f9e8cde021989905cb7bc094b28aad8dd9aaa0c13e15cc818fe` | .../DPI/9/f4c30b56-86cb-4242-91e3-72b31bf8267c_1700040002003.pdf |

Together: EUR-exact, non-EUR+multilingual+holding, and English-empty — spanning the value
rule, the currency fail-closed branch, section (D) holdings, and the multilingual seam.

### EU.8 Politeness — concurrency 1, ≥3 s (robots crawl-delay), identified UA + From; served 200 throughout.

---

## §FR — France HATVP, déclaration d'intérêts et d'activités (DIA)

### FR.1 Regime metadata
| Field | Value |
|---|---|
| jurisdiction | `fr` (national) |
| body | `HATVP` (Haute Autorité pour la transparence de la vie publique) |
| regime slug | `fr_hatvp_dia` |
| regime_type | `periodic_declaration` — filed within 2 months of taking office; a modificative (`dim`/`diam`) within 2 months of a substantial change |
| value_precision | `exact` — `remuneration/montant` per year in exact euros (E-FR-firmin: `62 389`, `109 344`, …) |
| source_url | https://www.hatvp.fr/open-data/ |
| index endpoint | `GET /livraison/opendata/liste.csv` (E-FR-liste — the discovery index) |
| document endpoint | `GET /livraison/dossiers/{open_data_filename}.xml` (per-declaration XML — canonical Bronze; E-FR-dia) |
| merged endpoint | `GET /livraison/merge/declarations.xml` (84 MB, all declarations, `Last-Modified` — conditional-GET/bulk primitive) |
| licence | Etalab open licence (freely reusable) — for `di`/`dia` only (§0.2) |

### FR.2 Discovery
**Data-source: the HATVP open-data feed** (the UK-analog structured route). `robots.txt`
(E-FR-robots) only disallows `/wordpress/wp-admin/`.

Index columns (E-FR-liste, `;`-separated):
`civilite;prenom;nom;classement;type_mandat;qualite;type_document;departement;date_publication;date_depot;nom_fichier;url_dossier;open_data;statut_publication;id_origine;url_photo`.

Scope filters at discovery:
- **`type_document` IN {`dia`,`diam`}** (déclaration d'intérêts et d'activités + its
  modificative). `di`/`dim` (interests-only, used by local officials) are in the same
  schema and could be added later; **`dsp`/`dspm`/`dspfm` are EXCLUDED — legal** (§0.2).
- **`type_mandat` IN {`depute`,`senateur`,`gouvernement`}** (national politicians in
  govfolio rosters). `europe` (French MEPs, 328 — overlap with §EU), and the ~9,000 local
  officials (`departement`/`region`/`commune`/`epci`/`ctsp`) are out of v1 (E-FR-liste
  counts).
- **`statut_publication = "Livrée"`** (delivered/published; 8,253 rows). "publication en
  préfecture à venir" (577) never becomes open data (§0.2).

Algorithm: fetch `liste.csv`; per in-scope row emit a `FilingRef`
(`external_id = {open_data_filename-stem}`, e.g. `firmin-le-bodo-agnes-dia30921-depute-76`;
`version` = declarationVersion + a `dim`/`diam` sequence); `fetch`
`/livraison/dossiers/{open_data}` → Bronze XML. Conditional-GET / weekly full-diff via the
merged XML `Last-Modified` (E-RET). Concurrency 1, ≥2 s. Politician resolution:
`(nom,prenom) + qualiteMandat.labelOrgane` (département/circonscription) join to the
Assemblée/Sénat roster; `id_origine` (E-FR-liste, the tribun id) is a strong join key for
députés.

### FR.3 Record anatomy (the Bronze document; E-FR-dia, E-FR-firmin, E-FR-pannier)
`<declaration>` (schema `declarationVersion=20171221`): top metadata (`dateDepot`,
`uuid`, `origine=ADEL`, `complete`) + `<general>` (typeDeclaration{id:`DIA`}, qualiteMandat,
dateDebutMandat, `declarationModificative` bool, `declarant`{civilite,nom,prenom,
dateNaissance; private fields = `[Données non publiées]`}) + a fixed set of **typed
sections**, each `<...Dto>` with `<items>` and a `<neant>` bool (`true` = nothing declared):

| Section tag | Meaning | asset_class | owner | value |
|---|---|---|---|---|
| `activProfCinqDerniereDto` | professional activities, last 5 years | `other` | self | `remuneration/montant`/yr (exact €) |
| `activConsultantDto` | consulting activities | `other` | self | remuneration/yr |
| `activProfConjointDto` | **spouse's** professional activity | `other` | **spouse** | (usually none) |
| `fonctionBenevoleDto` | volunteer functions | `other` | self | none |
| `mandatElectifDto` | other elective mandates | `other` | self | indemnités/yr |
| `participationDirigeantDto` | board / governing-body participation | `other` | self | remuneration/yr (E-FR-dia) |
| `participationFinanciereDto` | **financial holdings / shares** | `equity` | self | valeur / capital held |
| `activCollaborateursDto` | parliamentary collaborators' other activities (conflict info) | `other` | NULL | none |
| `observationInteretDto` | free-text observations | — | — | → details note, no Gold row |

Per-item fields (E-FR-dia / E-FR-pannier): `motif{id: CREATION\|MODIFICATION\|SUPPRESSION}`,
`commentaire`, `nomSociete`, `activite`, `remuneration{brutNet: Net\|Brut, montant:[{annee,
montant}]}`, `dateDebut`/`dateFin` (`MM/YYYY`). **`montant` is exact euros, French-formatted
with a space thousands-separator and no decimals** (`62 389`, `109 344`, E-FR-firmin) —
strip the spaces to parse to `Decimal` (invariant 7).

### FR.4 Silver — `StagingRow` (stg_fr_hatvp_dia)
One StagingRow per declared item across all non-`neant` sections.
| Field | Type | Req | Content |
|---|---|---|---|
| `declaration_stem` | string | yes | `{open_data}` filename stem (external id) |
| `declaration_uuid` | string | yes | `<uuid>` |
| `type_declaration` | string | yes | `DIA` (or `DIAM`) |
| `is_modificative` | boolean | yes | `declarationModificative` |
| `type_mandat_raw` | string | yes | `qualiteMandat.codTypeMandatFichier` (`depute`) |
| `organe_raw` | string\|null | yes | `qualiteMandat.labelOrgane` value (département) |
| `declarant_nom_raw`/`prenom_raw` | string | yes | verbatim |
| `date_depot_raw` | string | yes | `<dateDepot>` (`DD/MM/YYYY HH:MM:SS`) |
| `section_tag` | string | yes | the `<...Dto>` tag (§FR.3 vocab; unknown ⇒ freeze) |
| `row_ordinal` | integer ≥1 | yes | 1-based document order |
| `motif_raw` | string\|null | yes | `CREATION`\|`MODIFICATION`\|`SUPPRESSION` |
| `entry_fields_raw` | jsonb | yes | the item's child elements verbatim (`nomSociete`,`activite`,`commentaire`,`dateDebut`,`dateFin`, …) |
| `remuneration_raw` | jsonb\|null | yes | `{brutNet, montant:[{annee,montant}]}` verbatim (space-separated numerals kept) |
| `neant_section` | boolean | yes | whether the section was `neant` (audit) |
| `confidence` | number [0,1] | yes | §FR.6 |
| `extractor` | string | yes | `fr_hatvp_dia/xml@1` |

### FR.5 details contract — (fr_hatvp_dia, interest)
`FrHatvpDiaInterestDetailsV1`, snapshot at
`crates/pipeline/schemas/details/fr_hatvp_dia.interest.json`.
Fields: `declaration_stem`, `declaration_uuid`, `type_declaration`, `is_modificative`,
`section_tag`, `row_ordinal`, `motif` (enum), `company` (nomSociete\|null), `activity`
(activite\|null), `comment` (commentaire\|null), `date_debut`/`date_fin` (`MM/YYYY`\|null),
`brut_net` (enum `Net`\|`Brut`\|null), `montants` (array `{year:int, amount:string}` —
amounts normalized to decimal strings), `organe` (string\|null), `language` (const `fr`),
`value_source` (enum `montant_latest_year`\|`none`).
Gold mapping: `record_type=interest`; `asset_description_raw` = a stable join of
`activite`+`nomSociete`+`commentaire` (verbatim, French; empty ⇒ reject); `asset_class`
per §FR.3; `owner` per §FR.3 (`activProfConjointDto`→spouse); `notified_date`=`dateDepot`
date; `value` = the **latest declared year's** `montant` (low==high EUR) or NULL; all years
ride `details.montants`.

### FR.6 Extraction strategy (spec-writer exclusive)
**Deterministic, no LLM seam** (design §5.3 rule 1; extraction-strategy skill). The Bronze
document is well-formed XML with a fixed, self-describing schema — `quick-xml`/`serde-xml`
into source-shaped structs; `deny_unknown_fields` on the top-level so schema drift freezes,
while item child vocabularies stay tolerant. Parse the space-separated `montant` to
`Decimal`. UTF-8 throughout (accented names). Confidence: start 1.00; −0.05 an item with a
`montant` that fails Decimal parse; −0.02 `motif` MODIFICATION/SUPPRESSION (modificative
supersession unwired, open question). Hard rejects: unknown `section_tag`, unknown
`typeDeclaration`, empty description. Fetch = plain `reqwest` (served, E-RET); cache by sha.

### FR.7 Fixtures (≥3; raw-byte sha pins — served host, real bytes)
Pinning rule: sha256 of the raw per-declaration XML response bytes. Content-normalized
fallback: sha256 over ordered `(section_tag,row_ordinal,entry_fields_raw)` tuples.
| # | Case | Filer (mandat) | sha256 (raw bytes) | URL (base `https://www.hatvp.fr/livraison/dossiers/`) |
|---|---|---|---|---|
| fr-1 | **comprehensive**: all 6+ sections populated incl. `participationFinanciereDto` (equity) + exact per-year `remuneration` (`62 389`…`109 344`) → value exact, holdings, brut/net | Agnès Firmin Le Bodo (député 76) | `716e5591506a8fa9de4e1900592c855db3f213585e355e694e97253ede542d01` | `firmin-le-bodo-agnes-dia30921-depute-76.xml` |
| fr-2 | **modificative + high value**: `declarationModificative=true`, `motif CREATION` items, exact `104 778` Net remuneration → the update path + value | Agnès Pannier-Runacher (député 62) | `eacd17ff864b807344b12bcfd608fa1181456210cd75937d1e036b7c03cdee05` | `pannier-runacher-agnes-dia34763-depute-62.xml` |
| fr-3 | **minimal / neant + non-remunerated**: `participationDirigeantDto` (unpaid association, montant all 0), `observationInteretDto` free text, most sections `neant=true` → value NULL, `neant` handling | Abdelkader Lahmar (député 69) | `ec4003d075953b88af1b69c03721537a92e27647a2ef073ffd915318c3a553c3` | `lahmar-abdelkader-dia31320-depute-69.xml` |

Alternate archived, not primary: Lahmar `diam` modificative
(`af840b0294d0cbc146c982f8b39170dbdffe993827686ff8c141aaf0c33ac887`,
`lahmar-abdelkader-diam31323-depute-69.xml`).

### FR.8 Politeness — concurrency 1, ≥2 s, identified UA + From; served 200 throughout; merged XML `Last-Modified` is the incremental primitive.

---

## §DE — Deutscher Bundestag, veröffentlichungspflichtige Angaben (Nebentätigkeiten)

### DE.1 Regime metadata
| Field | Value |
|---|---|
| jurisdiction | `de` (national) |
| body | `Deutscher Bundestag` |
| regime slug | `de_bundestag` |
| regime_type | `periodic_declaration` — declared on taking up the mandate, updated continuously; published rolling on the member's page |
| value_precision | **`exact`** (current) — "Einkünfte betragsgenau (nach Euro und Cent)" since the Transparenzgesetz of 8 Oct 2021 (E-DE1). **Historical (2013–2021): `banded` 10-Stufen** — §DE.6, backfill only |
| threshold | publish income > €1,000/month or > €3,000/year; Beteiligungen > 5% share; donations > €3,000/yr (E-DE1) |
| source_url | https://www.bundestag.de/abgeordnete/biografien |
| document endpoint | per-MdB `GET /abgeordnete/biografien/{Initial}/{name}-{id}` — the "Veröffentlichungspflichtige Angaben" section (renders via a dynamic fragment, §DE.2) |
| currency | EUR |

### DE.2 Discovery + access (bot gate — browser-engine seam REQUIRED)
**`www.bundestag.de` is Enodia (Radware) bot-gated: EVERY non-browser client — the
identified UA, a stock Chrome UA string, even `/robots.txt` and `/static/appdata/*.pdf` —
gets HTTP 400 `/.enodia/challenge` (a JavaScript proof-of-work + cookie challenge,
E-DE-block).** Only a client that solves the POW passes. This is the
australia_register / us_senate precedent: production fetch MUST run through a
**browser-engine seam** (headless Chromium sending the identified UA); the sub-adapter
freezes + review_task if the seam is unavailable — never fingerprint evasion. DE content
this leg was obtained via a browser-engine reader (r.jina.ai, E-RET); **DE canonical
raw-byte pins are deferred to the capture/build leg** (§DE.7).

Algorithm: enumerate members from `/abgeordnete/biografien` (pattern
`/{Initial}/{name}-{id}`, E-DE-bio); per member fetch the biografien page via the seam and
extract the "Veröffentlichungspflichtige Angaben" fragment. **The exact fragment
endpoint + HTML grammar are an open question** (the reader returned page chrome, not the
disclosure fragment, E-DE-member) — a build-leg discovery behind the gate. Roster join:
name + Wahlkreis/Landesliste + `id` (the biografien numeric id is the stable person key).

### DE.3 Record anatomy — the 8 published categories (E-DE1, verbatim §45/§48 AbgG)
Only categories with data appear; entries are alphabetical within a category:
| # | Category (heading) | Legal basis | asset_class | typical value |
|---|---|---|---|---|
| 1 | Berufliche Tätigkeit vor der Mitgliedschaft im Deutschen Bundestag | §45(1) Nr.1 | `other` | none/exact |
| 2 | Entgeltliche Tätigkeiten neben dem Mandat | §45(2) Nr.1 | `other` | **exact €** |
| 3 | Funktionen in Unternehmen | §45(2) Nr.2 | `other` | none/exact |
| 4 | Funktionen in Körperschaften/Anstalten des öffentlichen Rechts | §45(2) Nr.3 | `other` | none/exact |
| 5 | Funktionen in Vereinen, Verbänden, Stiftungen | §45(2) Nr.4 | `other` | none |
| 6 | Vereinbarungen über künftige Tätigkeiten oder Vermögensvorteile | §45(2) Nr.5 | `other` | none/exact |
| 7 | Beteiligungen an Kapital-/Personengesellschaften (> 5 %) | §45(2) Nr.6 | `equity`/`private` | exact income if any |
| 8 | Spenden und sonstige Zuwendungen für die politische Tätigkeit (> €3,000/yr) | §48 | `other` | **exact €** |

**Published value format (E-DE1, verbatim examples)** — German decimal (comma decimal, dot
thousands):
- regular monthly: `monatlich, 1.250,43 Euro`
- regular annual: `jährlich, 4.354,23 Euro` (also for sub-€1,000/month sums that exceed
  €3,000/yr, e.g. `jährlich, 5.400 Euro`)
- one-off: `2021, genauer Betrag` (year prefix + exact amount)
- per contract partner, aggregated per year: `Mandant 1, 2021, 5.860,31 Euro`
- profit-before-tax variant: `2021, Betrag, Gewinn vor Steuern`
- non-quantifiable (e.g. stock options): `2021, Rechtsposition` → value NULL, keep raw
- anonymised partner: `Kunde 1, Baugewerbe` / `Mandant 1` / `Vertragspartner 4`
- honorary: `ehrenamtlich`

### DE.4 Silver — `StagingRow` (stg_de_bundestag)
| Field | Type | Req | Content |
|---|---|---|---|
| `mdb_id` | integer | yes | biografien id |
| `member_name_raw` | string | yes | verbatim |
| `wahlperiode` | integer | yes | e.g. 21 |
| `category_number` | integer 1..8 | yes | §DE.3 (unknown ⇒ freeze) |
| `category_name_raw` | string | yes | heading verbatim |
| `row_ordinal` | integer ≥1 | yes | 1-based within the member's disclosures |
| `entry_text_raw` | string | yes | the activity/function/holding line verbatim (German) |
| `amount_raw` | string\|null | yes | the value string as published (`monatlich, 1.250,43 Euro`, `2021, Rechtsposition`, null) |
| `partner_raw` | string\|null | yes | contract partner/auftraggeber (may be `Kunde 1`) |
| `ehrenamtlich` | boolean | yes | the honorary marker |
| `confidence` | number [0,1] | yes | §DE.6 |
| `extractor` | string | yes | `de_bundestag/html@1` |

### DE.5 details contract — (de_bundestag, interest)
`DeBundestagInterestDetailsV1`, snapshot at
`crates/pipeline/schemas/details/de_bundestag.interest.json`.
Fields: `mdb_id`, `wahlperiode`, `category_number` (1..8), `category_name`, `row_ordinal`,
`entry_text`, `amount_raw` (string\|null verbatim), `period` (enum
`monthly`\|`annual`\|`one_off`\|null), `amount_year` (int\|null), `partner` (string\|null),
`profit_before_tax` (bool), `non_quantifiable` (bool — the `Rechtsposition` case),
`ehrenamtlich` (bool), `language` (const `de`),
`value_source` (enum `betragsgenau`\|`stufe_historical`\|`none`).
Gold mapping: `record_type=interest`; `asset_description_raw=entry_text` (German, verbatim);
`asset_class` per §DE.3; `owner=self`; `notified_date`= the publication/as-of date (nullable
— the site is "continuously updated", no per-entry date; page-capture date is a documented
convention, NOT fabricated precision); `value` = the parsed exact euro/cent (low==high EUR),
NULL for `Rechtsposition`/`ehrenamtlich`-only; **for backfilled 18./19. WP → the Stufe band
→ ValueInterval range (§DE.6)**.

### DE.6 Historical 10-Stufen → ValueInterval band table (backfill only; OUT of v1 green path)
The 2013–2021 system published *regular monthly* income in ten bands (E-DE2 pins the
endpoints 1.000 / 3.500 / 250.000; intermediate tiers via WebSearch, build-leg webarchiv
pin pending — open question). When govfolio backfills the 18./19. Wahlperiode, a `Stufe N`
maps to a real `ValueInterval` (EUR), `value_source = stufe_historical`:
| Stufe | Range (EUR, monthly) | ValueInterval `low` | `high` |
|---|---|---|---|
| 1 | 1.000 – 3.500 | `1000.00` | `3500.00` |
| 2 | 3.500 – 7.000 | `3500.00` | `7000.00` |
| 3 | 7.000 – 15.000 | `7000.00` | `15000.00` |
| 4 | 15.000 – 30.000 | `15000.00` | `30000.00` |
| 5 | 30.000 – 50.000 | `30000.00` | `50000.00` |
| 6 | 50.000 – 75.000 | `50000.00` | `75000.00` |
| 7 | 75.000 – 100.000 | `75000.00` | `100000.00` |
| 8 | 100.000 – 150.000 | `100000.00` | `150000.00` |
| 9 | 150.000 – 250.000 | `150000.00` | `250000.00` |
| 10 | > 250.000 | `250000.00` | `NULL` (open-ended) |

The current-state green path (21. WP) does NOT use this table — it publishes exact amounts
(§DE.3). This is the one genuinely **banded** value shape in the compound, quarantined to
backfill and flagged for a webarchiv boundary-pin at build time.

### DE.7 Extraction strategy (spec-writer exclusive)
**Deterministic HTML parse behind a mandatory browser-engine fetch seam.** The published
data is server-rendered structured HTML (categories + entry lines), so `scraper` (CSS
selectors) is the parse tool (design §5.3 rule 1) — BUT the fetch stage MUST use the
browser-engine seam (§DE.2 Enodia gate); unavailable seam ⇒ freeze + work item, never
evasion. Parse the German-formatted amount (`1.250,43` → strip dots, `,`→`.` → `Decimal`).
Confidence: start 1.00; −0.05 amount string present but unparseable; −0.02
`non_quantifiable`. Hard rejects: unknown category, empty entry. Cache by sha per member-page
version. The exact selector grammar is finalized at build time once the fragment endpoint is
captured (§DE.2 open question); the published FORMAT (§DE.3) is the authoritative contract.

### DE.8 Fixtures (bot-gated ⇒ raw-byte pins DEFERRED to capture leg; ≥2 with justification)
Per the goal's blocked-source rule, DE fixtures are **candidate members** to be captured by
the build leg via the browser-engine seam (sha = capture leg, australia_register precedent);
the reader evidence (E-DE-member) documents the fragment gap. Selection targets the value
matrix — an exact-income case, a Beteiligung (holding) case, and a functions-only
(qualitative) case — from the 21. WP. Pinning rule at capture: sha256 of the raw member-page
(or fragment) bytes; content-normalized fallback over ordered
`(category_number,row_ordinal,entry_text,amount_raw)` tuples.
| # | Case (target) | Member endpoint pattern | sha256 |
|---|---|---|---|
| de-1 | **exact income** (category 2, `monatlich`/`jährlich, N Euro`) → ValueInterval exact | `/abgeordnete/biografien/{I}/{name}-{id}` (a 21. WP MdB with published Nebeneinkünfte) | _capture leg_ |
| de-2 | **Beteiligung** (category 7, > 5 % holding) → asset_class equity/private, income exact-or-none | idem | _capture leg_ |
| de-3 | **functions-only / none** (categories 3–5, no income) → value NULL, qualitative | idem (e.g. `al_wazir_tarek-1043422`, E-DE-member) | _capture leg_ |

Justification for deferred pins + candidate-only: `www.bundestag.de` blocks every
non-browser client (E-DE-block); no polite raw-byte route exists from the spec env. The
build leg's browser-engine seam is a prerequisite (not a nice-to-have); a backfill fixture
exercising §DE.6 Stufe bands is added when a webarchiv boundary-pin lands.

### DE.9 Politeness — browser-engine seam carries the identified UA + From, concurrency 1, ≥2 s; robots policy unretrievable (Enodia) ⇒ self-imposed limits govern (invariant 10).

---

## 9. Evidence log (retrieved 2026-07-05; full request/politeness detail in E-RET)
Archived under `docs/regimes/eu_fr_de_annual/evidence/{eu,fr,de}/` **in this commit**,
sha-named. **DE reader-extract sha = sha of the archived extraction, NOT the canonical
source bytes** (host bot-gated; §DE.2, australia precedent).

| ID | Source | Archived file (sha256 = filename prefix) |
|---|---|---|
| E-EU-robots | europarl robots.txt | `eu/…abcbbbca….robots-europarl.txt` |
| E-EU-list | /meps/en/full-list/all (MEP ids) | `eu/…e5d09096….mep-full-list.html` |
| E-EU-decltab | Adamowicz declarations tab (DPI link discovery) | `eu/…4a2bc3d6….mep-declarations-adamowicz.html` |
| E-EU3 | DPI term-9 Spyraki (EN form, sections A-G) — fixture eu-3 | `eu/317354d2….dpi-t9-spyraki-en.pdf` |
| E-EU2 | DPI term-10 Adamowicz (PL, exact PLN + periodicity, holding) — fixture eu-2 | `eu/098b372b….dpi-t10-adamowicz-pln.pdf` |
| E-EU-aftias | DPI term-10 Aftias (EUR exact) — fixture eu-1 | `eu/aca0b732….dpi-t10-aftias-eur.pdf` |
| E-EU-portal403 | data.europarl.europa.eu 403 (portal host tried-log) | `eu/58bf2215….opendata-portal-403.html` |
| E-EU-home | Adamowicz /home (resolution) | `eu/dc182d15….mep-home-adamowicz.html` |
| E-EU-declaftias | Aftias declarations tab | `eu/59753915….mep-declarations-aftias.html` |
| E-FR-robots | hatvp robots.txt | `fr/92dac542….robots-hatvp.txt` |
| E-FR-opendata | /open-data/ (endpoint URLs) | `fr/feae918d….opendata-page.html` |
| E-FR-liste | liste.csv (index, 23,459 rows; scope counts) | `fr/79c929be….opendata-liste.csv` |
| E-FR-dia | Lahmar DIA XML (schema; fixture fr-3) | `fr/ec4003d0….dia-lahmar.xml` |
| E-FR-firmin | Firmin Le Bodo DIA (all sections, exact €; fixture fr-1) | `fr/716e5591….dia-firmin-le-bodo.xml` |
| E-FR-pannier | Pannier-Runacher DIA (modificative; fixture fr-2) | `fr/eacd17ff….dia-pannier-runacher.xml` |
| E-FR-diam | Lahmar DIAM (modificative alternate) | `fr/af840b02….diam-lahmar.xml` |
| E-FR-notice | open-data notice PDF (XML field notice) | `fr/fed8b827….opendata-notice.pdf` |
| E-FR-structure | opendata-structure.xlsx (XML schema) | `fr/008ebc44….opendata-xml-structure.xlsx` |
| E-DE-block | Enodia challenge page (bot posture, raw bytes) | `de/07513f15….enodia-challenge-block.html` |
| E-DE1 | Hinweise zur Veröffentlichung (rules: 8 categories, exact-amount format) — AUTHORITY | `de/ead25236….reader-hinweise-veroeffentlichung.md` |
| E-DE2 | 2013 Stufen announcement (historical band endpoints) | `de/6c1908f8….reader-stufen-2013.md` |
| E-DE-bio | biografien index (member URL pattern) | `de/a1bb3af8….reader-biografien-index.md` |
| E-DE-member | Al-Wazir member page (fragment gap) | `de/e47d80d5….reader-member-alwazir.md` |
| E-FR-law | Art. LO 135-2 patrimony reuse ban (€45,000) | WebSearch-derived (front-matter legal_basis; not a fetched page — tried-log in E-RET) |
| E-RET | our retrieval + access-method record | `2026-07-05-eu_fr_de_annual.retrieval.json` |

## Quirks log (append-only, dated)
- 2026-07-05 · **Three regimes, one adapter** (§0): conformance dispatches by the single
  name `eu_fr_de_annual`; fixtures are source-namespaced `fixtures/{eu,fr,de}_<case>/`.
- 2026-07-05 · **DE reformed away from bands** (E-DE1): since 8 Oct 2021 income is exact
  euro/cent — the 10-Stufen table (§DE.6) is HISTORICAL (2013–2021), quarantined to
  backfill. The goal's "banded Stufe" framing describes the pre-2021 system; current DE is
  `exact`. Documented both, evidence-first.
- 2026-07-05 · **EU MEPs declare in national currency** (E-EU2 Adamowicz = PLN): the
  `Currency` enum (EUR/GBP/USD) can't hold PLN/HUF/SEK/DKK/CZK/RON/BGN → v1 fail-closed
  (value NULL + `unmapped_currency` review); RECOMMEND extending the enum (front-matter).
- 2026-07-05 · **EU DPI is multilingual** (E-EU2 fully Polish): a deterministic parser
  keyed on English headers breaks → LLM-vision-first, sections keyed on `Art. 4(2)(x)`
  (language-stable), not header text.
- 2026-07-05 · **EU value = amount + periodicity** (`4940 PLN miesięcznie`): periodicity
  rides `details`, never annualised into `value`.
- 2026-07-05 · **FR patrimony (`dsp*`) is legally un-republishable** (Art. LO 135-2,
  €45,000 fine, E-FR-law): scope excludes `dsp`/`dspm`/`dspfm` — only `dia`/`diam`
  (Etalab-reusable). Residual human/legal flag (§0.2).
- 2026-07-05 · **FR montant is space-separated exact euros** (`62 389` = 62389, E-FR-firmin)
  with `brutNet` + per-year arrays; strip spaces to `Decimal`; Gold value = latest year.
- 2026-07-05 · **FR items carry `motif` CREATION/MODIFICATION/SUPPRESSION** (E-FR-dia) and
  `declarationModificative` — the modificative/version mechanism; version-keyed filings.
- 2026-07-05 · **DE `www.bundestag.de` is Enodia-bot-gated** (E-DE-block): identified UA AND
  browser UA both 400; static assets + robots.txt gated too → browser-engine seam required,
  DE raw-byte pins deferred (australia precedent).
- 2026-07-05 · **DE disclosure loads via a dynamic fragment** not captured by the reader
  (E-DE-member): fragment endpoint + selector grammar are a build-leg discovery behind the
  gate (open question); the published FORMAT (E-DE1) is the authoritative contract.
- 2026-07-05 · **`data.europarl.europa.eu` (open-data portal) 403s the plain client** while
  `www.europarl.europa.eu` serves it (E-EU-portal403): per-host posture within europarl.eu,
  same lesson as parliament.uk (uk_commons_register).

## Operational notes (politeness incidents, outages)
- 2026-07-05 · ~35 requests total this task (E-RET): FR ~11 (all 200), EU ~11 (all 200 +
  1 deliberate 403 portal probe), DE 3 direct (all Enodia-400, bot gate) + 6 via the
  browser-engine reader (200). Concurrency 1, ≥2 s (≥3 s europarl) spacing, identified UA +
  From on every request. Zero 429s. No fingerprint-evasion attempted on the DE gate.
- 2026-07-05 · FR merged `declarations.xml` = 84 MB with `Last-Modified` — fetched HEAD only
  (per-declaration XML is the parse input); the merged file is the bulk/conditional-GET
  primitive for the build leg.
