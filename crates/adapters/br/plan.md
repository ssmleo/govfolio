# `br` (Brazil) — Phase 3 spec (holding)

Author: spec-writer. Source: `docs/regimes/br/AUTHORITY.md` (committed survey) +
`crates/adapters/br/fixtures/` (`MANIFEST.json` + 3 sampled cases). Scope: TSE
candidacy-time asset declarations (`declaração de bens`), `record_types: [holding]`
only, `DEPUTADO FEDERAL`/`SENADOR`(+suplentes). This is the project's first
`holding`-record-type regime — every prior adapter is
transaction/interest/change_notification, so nothing here should assume the shape
of those regimes. Companion artifact: `crates/adapters/br/src/details.rs` (the
`(br, holding)` details contract, compiles + tests pass — see bottom of this file).

## BLOCKING open item — read this first

**`Currency::BRL` does not exist.** `crates/core/src/domain/enums.rs`'s `Currency`
enum is `{ EUR, GBP, USD }` (its own doc comment: "Closed set for now; extend as
regimes land"). Every `br` holding's `VR_BEM_CANDIDATO` value is BRL. `GoldCandidate.value`
is a `ValueInterval` whose `currency: Currency` field cannot represent BRL today.
This is a one-line, additive enum variant (plus updating the
`wire_format_matches_sql_check_literals` test in `enums.rs`) — the DB column itself
is unconstrained (`currency char(3)`, no CHECK, `crates/core/migrations/0001_core.sql:121`),
so no migration is needed, only the Rust enum. **This is a `crates/core` change,
outside spec-writer's scope (`crates/adapters/br` only) and outside a single-adapter
PR's normal blast radius** — flagging for rust-builder/auditor to land as a small
precursor change before any `br` `normalize()` can construct a valid `GoldCandidate`.
Not fixed here; do not guess a workaround (e.g. mis-tagging BRL as USD) — that would
silently corrupt money data (invariant 7).

## Field-mapping table (source → Gold / details)

Fixture citations use `<case>:<line>` against
`crates/adapters/br/fixtures/<case>/input.json`. Cases: `typical_house_vehicle_land`
(A, 3 items, clean baseline), `amendment_post_election_2026` (B, 3 items, late
timestamp), `zero_asset_deputado` (C, 0 items).

| Source field | Gold column / details field | Fixture citation | Notes |
|---|---|---|---|
| `bem_candidato.SQ_CANDIDATO` (join key from `consulta_cand`) | `politician_id` resolution input (not itself a Gold column) | A:67, B:67, C: n/a (no rows) | Per-cycle only, minted fresh each election (AUTHORITY.md `identifiers_available`) — not usable alone as a durable cross-cycle politician key. |
| `consulta_cand.NR_TITULO_ELEITORAL_CANDIDATO` | politician resolution input (internal only) | A:40, B:40 (both `[SYNTHETIC-TITULO]` in fixtures) | Unmasked in both 2022 and 2024 cycles per AUTHORITY.md — the more durable cross-cycle personal key, since CPF is suppressed from 2024. PII: never surfaces past Bronze/internal resolution (`personal_data_to_redact`). |
| `consulta_cand.NR_CPF_CANDIDATO` | politician resolution input (internal only, pre-2024 cycles) | A:23, B:23 (both `[SYNTHETIC-CPF]`) | PII, suppressed (sentinel `-4`) from the 2024 cycle onward — do not treat as always-present. Never surfaces past Bronze/internal resolution. |
| `consulta_cand.DT_NASCIMENTO` | not a Gold/details field — PII only | A:39, B:39 (both `[SYNTHETIC-DOB]`) | `personal_data_to_redact`; must never reach `details` or any public surface. |
| `consulta_cand.DS_CARGO` | discovery-time filter, not a Gold column | A:17, B:17, C:17 (all `"DEPUTADO FEDERAL"`) | Keep only `DEPUTADO FEDERAL`/`SENADOR`/`1º SUPLENTE`/`2º SUPLENTE` (AUTHORITY.md scope). All 3 fixtures are `DEPUTADO FEDERAL` — **no `SENADOR`/suplente fixture exists** (sampler note: screened, none stood out; flag for test-designer/builder discretion). |
| `bem_candidato.DT_ELEICAO` (repeated per row; also on `consulta_cand`) | `as_of_date` (required for `record_type == holding` per `GoldCandidate::validate`) | A:63/84/105, B:63/84/105, C: n/a | `DD/MM/YYYY` (Brazilian format, e.g. `"02/10/2022"`) — parse with `NaiveDate::parse_from_str(s, "%d/%m/%Y")`, not ISO. First non-ISO date format in this codebase; a genuinely new parse rule, not a copy of any prior regime. |
| `bem_candidato.ANO_ELEICAO` | `details.election_year` | A:58/79/100 (`"2022"`), B same | Distinct source field from `DT_ELEICAO`; kept separately in details for cycle-based queries without date-parsing `as_of_date`. |
| `bem_candidato.CD_TIPO_BEM_CANDIDATO` + `DS_TIPO_BEM_CANDIDATO` | `asset_class` (Gold, required, non-`Option`) + `details.asset_type_code_raw` / `details.asset_type_label_raw` + `details.asset_class` (deliberately redundant copy) | A:69/70 (`21`/veículo), A:90/91 (`12`/Casa), A:111/112 (`13`/Terreno), B:69/70 (`32`/quotas), B:90/91 & B:111/112 (`97`/VGBL) | See dedicated code→`AssetClass` table below — no official TSE code table exists (confirmed absent by the surveyor), so this table is spec-writer's own construction from the 5 codes actually observed. |
| `bem_candidato.DS_BEM_CANDIDATO` | `asset_description_raw` (Gold) + `details.asset_description_raw` (redundant copy, see details.rs doc comment) | A:71 (moto), A:92 (casa), A:113 (terreno), B:71/92/113 | Verbatim, invariant 2. PII-screened by the sampler (no addresses/plates/phones in these 3 cases — not guaranteed at full scale, see edge cases). |
| `bem_candidato.VR_BEM_CANDIDATO` | `value` (`ValueInterval`, `low == high`, `currency: BRL` — **blocked**, see above) + `details.value_raw` (verbatim pre-parse string) | A:72 (`"15000,00"`), A:93 (`"10000,00"`), A:114 (`"900000,00"`), B:72/93/114 | Comma-decimal, no thousands separator observed in any of the 9 sampled line items (largest: `900000,00`). Parse: strip `.` (thousands-separator defense, not observed but plausible for larger real-estate values at full scale — see edge cases), replace `,` with `.`, parse as `rust_decimal::Decimal`. Exact value: `low == high` (no banding — `AUTHORITY.md` front-matter: `value_precision: "exact"`, `band_table: []`). |
| `bem_candidato.NR_ORDEM_BEM_CANDIDATO` | `details.line_item_ordinal` (not a Gold column) | A:68/89/110 (`3`/`1`/`2`), B:68/89/110 (`1`/`2`/`3`) | 1-based ordinal within one candidate's declaration; recommend as part of the fingerprint composition (see amendment edge case) alongside `SQ_CANDIDATO`. |
| `bem_candidato.DT_ULT_ATUAL_BEM_CANDIDATO` + `HH_ULT_ATUAL_BEM_CANDIDATO` | `details.last_updated_date_raw` / `details.last_updated_time_raw`; **NOT** wired into Gold columns or supersession logic in this spec | A:73–74/94–95/115–116 (all `02/10/2022`/`23:21:28` — matches election date, non-amendment baseline), B:73–74/94–95/115–116 (all `13/05/2026`/`16:24:17` — ~44 months post-election) | See "Amendment-timestamp ambiguity" edge case — do not treat a changed value here as a confirmed per-candidate rectification without auditor sign-off. |
| — (no field observed anywhere in either source table) | `owner` — leave `None` for every `br` holding | n/a | No self/spouse/dependent distinction exists in the schema (AUTHORITY.md open question, re-confirmed: no owner-like column in any of the 9 sampled line items). `None`, not `Owner::Unknown` — this regime structurally has no owner concept to fail to resolve, unlike e.g. `us_house` where a blank owner column is a real absence-of-signal case. |
| — (no ISIN/ticker/registry id in source) | `instrument_id` — always `None` | n/a | AUTHORITY.md `identifiers_available.instrument`: "none... no ISIN/ticker/registry identifier of any kind." Never guess (invariant 3). |
| `bem_candidato.SQ_CANDIDATO` (per cycle) | `filing_id` external-id input | A:67, B:67 | MANIFEST.json packaging note: Brazil's natural filing unit is one candidate's whole declaration for one cycle — recommend `FilingRef.external_id` derived from `SQ_CANDIDATO` (+ year, since `SQ_CANDIDATO` is only unique within one cycle's file set). Multiple `GoldCandidate` holding rows (one per asset item) share one `filing_id`. |
| `consulta_cand` (whole row; not itself a Gold field) | `regime_id` | n/a | Fixed to the `br` `disclosure_regime` row; not source-derived. |

## `CD_TIPO_BEM_CANDIDATO` → `AssetClass` table (spec-writer construction — no official source)

The surveyor confirmed no published TSE code table exists (`AUTHORITY.md` field-mapping
table: "no separate published code table found — full code->AssetClass mapping is a
spec-writer task, not resolved here"). Every code below is grounded in the label
(`DS_TIPO_BEM_CANDIDATO`) actually paired with it in one of the 3 fixtures — nothing
here is invented from an assumed external table. `AssetClass` has exactly 9 variants:
`Equity, Bond, Fund, Option, Crypto, Commodity, RealEstate, Private, Other`.

| `CD_TIPO_BEM_CANDIDATO` | `DS_TIPO_BEM_CANDIDATO` (verbatim) | → `AssetClass` | Confidence | Citation |
|---|---|---|---|---|
| `12` | "Casa" | `RealEstate` | High — a house is real estate, unambiguous | A:90–91 |
| `13` | "Terreno" | `RealEstate` | High — land/plot is real estate, unambiguous | A:111–112 |
| `21` | "Veículo automotor terrestre: caminhão, automóvel, moto, etc." | `Other` | High — no vehicle bucket exists in `AssetClass`; matches `AUTHORITY.md`'s own suggestion and the established convention elsewhere (`us_house` has no vehicle-specific bucket either) | A:69–70 |
| `32` | "Quotas ou quinhões de capital" | **`Private`** (RESOLVED by audit, overriding this spec's `Other` default — see below) | High (post-audit) | B:69–70 |
| `97` | "VGBL - Vida Gerador de Benefício Livre" | **`Other`** (RESOLVED by audit, blessing this spec's default) | High (post-audit) | B:90–91, B:111–112 |
| any other code (not one of the 5 above) | — | `Other` (fail-closed default) | By construction | — |

**RESOLVED by independent audit (post-spec-writer; both re-derived from fresh
research + the fixture evidence itself, not rubber-stamped):**
- `32` "Quotas ou quinhões de capital" → **`AssetClass::Private`.** Fixture instance
  `B:71` reads verbatim *"1500 QUOTAS DE CAPITAL NA EMPRESA BUTIQUE DO CONDOMINIO
  LTDA — CNPJ..."* — an explicitly named, non-public Sociedade Limitada (Ltda) stake,
  Brazil's dominant privately-held company form (its "quotista" owners are the
  private-company analog of a shareholder; TSE separately codes publicly-tradable
  stock as "Ações"). This project already reserves `Private` for exactly this shape
  of holding (`crates/adapters/us_house/src/tables.rs:66`, mapping the US House
  ethics-guide category "Ownership Interests in Privately-Held Partnerships,
  Corporations, and S Corporations"). Defaulting a named, unambiguous Ltda stake to
  `Other` would waste real signal `Other`'s fail-closed default is meant to catch
  genuinely unclassifiable codes, not this one.
- `97` "VGBL" (Vida Gerador de Benefício Livre) → **`AssetClass::Other`** (spec-writer's
  original default, blessed not overridden). VGBL is a SUSEP-regulated (insurance
  regulator, not the securities regulator CVM), life-insurance-wrapped private-pension
  product whose value tracks a policyholder-chosen underlying fund — it is not itself
  a directly-held fund unit. This project already has the structurally analogous US
  product bucketed the same way: `us_house/tables.rs` puts Variable Annuity ("VA" —
  an insurance wrapper whose value also tracks an underlying fund) into `Other`,
  alongside Fixed Annuity ("FN") and Whole Life Insurance ("WU") — an established
  cross-regime convention that insurance/annuity wrappers bucket to `Other` even when
  fund-like. Keep `Other`.
- **Recommended implementation shape** (for rust-builder, not built here): a
  `asset_class_for_code(code: &str) -> Option<AssetClass>` lookup mirroring
  `us_house`'s/`us_senate`'s established convention (`crates/adapters/us_house/src/tables.rs:56`) —
  `Some(...)` for all 5 codes now resolved above (`12`/`13`→`RealEstate`, `21`→`Other`,
  `32`→`Private`, `97`→`Other`, all "High" confidence post-audit — see the code table),
  `None` (fail-closed default, `AssetClass::Other` + a confidence penalty at the
  caller) only for a code NOT in this table at all, i.e. one never observed in these
  3 fixtures. (This paragraph originally described `32`/`97` as unresolved,
  falling through the `None` path like a low-confidence code — that framing is now
  stale after the audit resolution above and has been corrected here; test-designer
  correctly caught the contradiction between this paragraph and the resolved code
  table when drafting `expected.gold.json` confidence scores. `32`/`97` are pinned,
  ordinary `Some(...)` entries now — do not re-introduce a confidence penalty for
  them without a new reason.)

## Parse strategy & rationale

**Deterministic CSV parse — confirmed, matches `AUTHORITY.md`'s "Parse strategy &
rationale" section.** The fixture `input.json` shape (`{"consulta_cand": {...one
row...}, "bem_candidato": [...0-or-more rows...]}`) is a **sampler packaging
decision**, not the native TSE format (`MANIFEST.json packaging_note`, explicit) —
the real source is two separate `;`-delimited, quoted, Latin-1-encoded CSVs
(`consulta_cand_<year>.zip`, `bem_candidato_<year>.zip`) joined at parse time by
`SQ_CANDIDATO`. Confirmed against all 3 fixtures: every field observed is a plain
string-typed CSV column (no nested structures, no free-text prose requiring LLM
extraction) — this holds even for the "amendment" case (still structured rows, just
with a different timestamp) and the "zero-asset" case (structurally an empty array,
not a missing key or malformed row). No LLM-fallback seam is needed for this
regime's core parse, matching `AUTHORITY.md`'s own conclusion.

**Row unit**: one `StagingRow` per `bem_candidato` line item (not one per candidate).
Each `StagingRow.payload` should carry the item's own fields plus whatever
`consulta_cand` join fields `normalize()` needs (`SQ_CANDIDATO`, `DS_CARGO`,
`NR_TITULO_ELEITORAL_CANDIDATO`/`NR_CPF_CANDIDATO` for politician resolution) — exact
join-field set is a rust-builder implementation decision, not fully specified here.
A candidate with zero declared assets (case C) legitimately produces **zero**
`StagingRow`s for that candidate — this must not trip invariant 6's fail-closed
zero-row check, which has to be scoped to the whole fetch/parse run (e.g. the whole
nationwide ZIP), never to one candidate's declaration (see edge cases).

**Date parsing**: `DD/MM/YYYY` (Brazilian convention), first non-ISO date format in
this codebase — `NaiveDate::parse_from_str(s, "%d/%m/%Y")`, not a reused ISO parser.

**Encoding**: source CSVs are Latin-1-encoded per `AUTHORITY.md`; the sampler already
transcoded to UTF-8 when producing `input.json` (visible accented characters, e.g.
"Xapuri", "quinhões"). The real `parse()` stage (reading raw bytes from Bronze) must
replicate this Latin-1→UTF-8 transcode itself — fixture-based conformance testing
won't exercise it since fixtures are pre-decoded JSON.

**Value parsing**: comma-decimal, no thousands separator observed in the 9 sampled
line items (up to `900000,00`) — strip `.` defensively (thousands-separator guard for
larger real-estate values not yet observed), replace `,`→`.`, parse `rust_decimal`.
Never treat as a float (invariant 7). **Blocked on `Currency::BRL`, see top of file.**

## Politeness config

Matches every prior phase this session (surveyor/sampler `AUTHORITY.md`/`MANIFEST.json`)
and the established production convention already used by every other adapter's
`politeness()` (`crates/adapters/*/src/adapter.rs`, e.g.
`crates/adapters/us_house/src/adapter.rs:57`, `crates/adapters/uk_commons_register/src/adapter.rs:106`):

```rust
PolitenessCfg::new(Duration::from_secs(2), "ssm.leo@outlook.com") // concurrency defaults to 1
```

- Identified UA: `PolitenessCfg::user_agent()`'s standard
  `govfolio-bot/0.1 (+https://govfolio.io; ssm.leo@outlook.com)` format (the
  research-only `"govfolio.io research (contact: ...)"` string in `AUTHORITY.md` was
  the survey's own ad hoc UA, not the production adapter's).
- Concurrency 1 (default), min-interval 2s — matches the sampler's own capture
  politeness (`MANIFEST.json`: `concurrency: 1`, `min_interval_seconds: 2`) and every
  other production adapter.
- **Fetch target: `cdn.tse.jus.br` only**, using the hardcoded, stable URL pattern
  `https://cdn.tse.jus.br/estatistica/sead/odsele/{consulta_cand,bem_candidato}/{dataset}_{YEAR}.zip`
  (no `[_{UF}]` suffix — see `MANIFEST.json`'s `uf_zip_pattern_correction`: the
  per-UF URL pattern `AUTHORITY.md`'s Data catalog section currently documents
  404s; per-UF CSVs ship *inside* the single nationwide ZIP instead). `cdn.tse.jus.br`
  carries no `robots.txt` at all.
- **Never target `dadosabertos.tse.jus.br/api/` on a recurring basis** — its
  `robots.txt` disallows `/api/`; that CKAN endpoint was a one-time, human-supervised
  discovery aid this session, not a fetch-loop target (`AUTHORITY.md tos_and_politeness`).
- Conditional GETs: CONFIRMED by audit (`curl -sI`) that `cdn.tse.jus.br`'s ZIP
  endpoints DO return `ETag`/`Last-Modified` — rust-builder should use
  `PoliteClient::get_conditional`, not a content-hash-based fallback.
- `docs/regimes/br/AUTHORITY.md` itself needs a write-back correcting the per-UF URL
  pattern (`MANIFEST.json`'s `uf_zip_pattern_correction.action_needed`) — out of scope
  for this pass (`docs/regimes/br/` is read-only reference per this task), flagged
  here so it isn't lost.

## Edge-case list

1. **Zero-asset candidates** (case C, `zero_asset_deputado`): a valid (`DS_SITUACAO_CANDIDATURA
   = "APTO"`) candidacy with `bem_candidato: []` is a legitimate "no assets declared"
   outcome, not a fetch/parse failure (`AUTHORITY.md` Quirks log, confirmed directly).
   `parse()` must emit zero `StagingRow`s for this candidate without tripping
   invariant 6 — that check belongs at the whole-run level, never per-candidate.

2. **Amendment-timestamp ambiguity** (`DT_ULT_ATUAL_BEM_CANDIDATO`/`HH_ULT_ATUAL_BEM_CANDIDATO`,
   case B `amendment_post_election_2026`). `AUTHORITY.md`'s `amendment_mechanism`
   section currently characterizes a changed value here as the per-item supersession
   trigger (invariant 1: supersede, never update). But the sampler's own fresh
   evidence (`MANIFEST.json amendment_timestamp_caveat`) found that, across all 27
   nationwide `bem_candidato_2022` UF files, **85–99% of an entire state's rows share
   one identical timestamp**, with **zero candidates showing mixed/partial per-item
   update dates** — a pattern much more consistent with a bulk backend
   re-timestamp/reindex event (e.g. a migration script touching the column
   state-wide) than genuine, individually-triggered candidate rectifications. Naively
   wiring this field into supersession logic risks manufacturing a spurious
   "correction" event for nearly an entire state's candidate population every time
   TSE's backend happens to touch this column.
   **RESOLVED — BLESSED by independent audit:** the auditor freshly downloaded TSE's
   real `bem_candidato_2022.zip` and independently re-derived the timestamp
   distribution across AL/MG/SP (97-98%+ of each state's rows sharing one
   state-specific dominant date, zero candidates with mixed per-item dates in any
   state sampled) — confirming the bulk-retimestamp read, not an artifact of the
   sampler's own analysis. **Decision: compute the idempotency fingerprint from row
   *content*** (`SQ_CANDIDATO` + `NR_ORDEM_BEM_CANDIDATO` + `CD_TIPO_BEM_CANDIDATO` +
   `DS_BEM_CANDIDATO` + `VR_BEM_CANDIDATO`), not from the timestamp — a
   content-identical re-touch is absorbed idempotently (`ON CONFLICT DO NOTHING`,
   invariant 4) rather than firing a spurious supersession; genuine content changes
   still trigger real supersession (invariant 1). The raw timestamp is still captured
   verbatim in `details` (`last_updated_date_raw`/`last_updated_time_raw`) for
   forensic visibility, just not trusted as a supersession trigger. **For
   test-designer**: characterize `amendment_post_election_2026` in
   `expected.gold.json` as exercising idempotent absorption of a content-identical
   re-touched row (timestamp differs, fingerprint unchanged, no superseding row) —
   NOT as a genuine content-based re-file/correction.

3. **`CD_TIPO_BEM_CANDIDATO -> AssetClass` mapping** — see the dedicated table above;
   2 of the 5 observed codes (`32`, `97`) are flagged low-confidence and default to
   `Other` pending auditor confirmation. Any code not in the table (i.e. every code
   not observed in these 3 fixtures) must also default to `Other`, never guessed.

4. **Currency::BRL missing** — see "BLOCKING open item" at the top of this file.

5. **PII must never reach `details` or any public surface**: `NR_CPF_CANDIDATO`,
   `NR_TITULO_ELEITORAL_CANDIDATO`, `DT_NASCIMENTO` are internal-resolution-only
   (`AUTHORITY.md personal_data_to_redact`) and are deliberately absent from
   `BrHoldingDetailsV1` — do not add them later without a redaction plan.

6. **`DS_SITUACAO_CANDIDATURA` filtering — open, not resolved this pass.** All 3
   fixtures show `"APTO"` (valid candidacy). Whether a candidacy later ruled
   `INAPTO`/cancelled should still have its (once-valid) asset declaration ingested
   as a Gold fact isn't addressed by `AUTHORITY.md` or resolved here — flagged for
   auditor/test-designer; no fixture exercises a non-`APTO` case.

7. **`SENADOR`/suplente coverage untested** — all 3 fixtures are `DEPUTADO FEDERAL`.
   The `DS_CARGO` discovery filter must still admit `SENADOR`/`1º SUPLENTE`/
   `2º SUPLENTE` per `AUTHORITY.md` scope even though no fixture exercises that path
   (sampler's own note: screened during selection, judged not required to satisfy the
   3-case gate). Builder/test-designer discretion to add a dedicated fixture.

8. **TSE sentinel values** (`#NULO`, `-1`, `-3`, `-4`) are observed elsewhere in
   `consulta_cand` (e.g. `NR_FEDERACAO: "-1"`, `NM_FEDERACAO: "#NULO"` for
   `TP_AGREMIACAO: "PARTIDO ISOLADO"` candidates, `zero_asset_deputado:31-34`) and are
   the documented mechanism for the 2024 CPF suppression (`"-4"`). None of the 3
   fixtures show a sentinel in any `bem_candidato` field this contract covers, but the
   parser should treat an unexpected sentinel in any of those fields as a fail-closed
   condition (invariant 6), not silently parse `"-4"` as a real code/value.

9. **Value parsing at scale**: only comma-decimal, no-thousands-separator values were
   observed (largest: `900000,00`). A thousands-separator variant (e.g.
   `"1.500.000,00"`) is plausible for larger real-estate holdings at full scale but
   not directly observed — the recommended strip-`.`-then-replace-`,` parse handles
   both forms, but this hasn't been tested against a real thousands-separated value.

10. **Per-UF ZIP URL pattern is wrong in `AUTHORITY.md`** — confirmed 404 by the
    sampler (`MANIFEST.json uf_zip_pattern_correction`); the correct fetch target is
    the single nationwide ZIP (per-UF CSVs ship inside it). Not fixed here (read-only
    reference this pass) — flagged for the next `AUTHORITY.md` write-back.

## `details.rs` summary

`crates/adapters/br/src/details.rs` defines `BrHoldingDetailsV1` (9 required fields,
no `Option`s — see file's own doc comment for why) and `holding_details_schema()`.
Compiles and its 2 unit tests pass (`cargo test -p br`); `cargo clippy -p br
--all-targets -- -D warnings` and `cargo fmt -p br -- --check` are both clean. This is
a **skeleton only** — no `tests/details_schema_snapshot.rs`, no committed JSON Schema
under `crates/pipeline/schemas/details/`, and no `(br, RecordType::Holding)` arm added
to `crates/pipeline/src/conformance.rs`'s registry. Per the `schema-contracts` skill's
documented bootstrap order, that snapshot/registration step needs the full adapter
crate wired (Cargo.toml deps for the whole `JurisdictionAdapter` impl, a
`tests/details_schema_snapshot.rs`, `UPDATE_SNAPSHOT=1 cargo test -p br --test
details_schema_snapshot`) — that is Phase 4 (rust-builder) work, not done here.

## Handoff — concrete next actions

1. **DONE (orchestrator precursor)**: `Currency::BRL` added to
   `crates/core/src/domain/enums.rs`; `crates/core/schemas/gold_candidate.json`
   regenerated (`UPDATE_SNAPSHOT=1 cargo test -p core --test schema_snapshot`);
   `cargo test -p pipeline --test role_evals` re-confirmed 11/11 green after the
   change (independently re-verified by audit too).
2. **DONE (audit)**: `AssetClass` mappings decided — `32`→`Private`, `97`→`Other` (see
   the code table above for the audit's reasoning).
3. **DONE (audit)**: fingerprint-from-content recommendation BLESSED (see edge case 2)
   — independently re-derived against a fresh TSE download, not just the sampler's
   own analysis.
4. **test-designer**: write `expected.silver.json`/`expected.gold.json` for the 3
   existing cases per this mapping table (using the resolved `AssetClass` decisions
   above and characterizing case B per the fingerprint decision — idempotent
   absorption, not a genuine correction); consider whether a dedicated
   `SENADOR`/suplente or non-`APTO`-status fixture is warranted (edge cases 6–7).
5. **rust-builder**: implement `discover`/`fetch`/`parse`/`normalize` (`adapter.rs`,
   `parse.rs`, `normalize.rs`, `tables.rs`-equivalent for the `AssetClass` lookup),
   the `tests/details_schema_snapshot.rs` + `crates/pipeline/schemas/details/br.holding.json`
   snapshot, and the `(br, RecordType::Holding)` arm in
   `crates/pipeline/src/conformance.rs`. Audit finding: `cdn.tse.jus.br` DOES return
   `ETag`/`Last-Modified` on its ZIP endpoints (confirmed via `curl -sI`) — use
   `PoliteClient::get_conditional`, not a content-hash-based fallback (this plan's
   politeness section originally said this wasn't checked; it has been, and
   conditional GETs work).
