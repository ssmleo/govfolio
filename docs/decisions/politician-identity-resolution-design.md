# Cross-time/cross-body politician-identity resolution — design (Phase 1 of goal "br identity collisions + exhaustive backfill")

Status: PROPOSED, ready to build in this same session. Grounded entirely in
(a) `docs/decisions/br-identity-collision-remediation.md`'s already-audited
JULIO CESAR DOS SANTOS fix, (b) `agents/JOURNAL.md` 2026-07-09's CARLOS
ALBERTO DE SOUZA finding, and (c) a 5-way scout survey of every other live
regime's raw source data (results in §2), not assumed.

## 1. The defect, restated precisely

`crates/pipeline/src/stages/roster.rs::resolve_hits` (the shared function
behind both `seed_roster` and `resolve_politician`) matches a filing to a
politician on exactly `(politician_alias.alias, mandate.district,
mandate.body)` — no time dimension, no person-identity dimension. Two
different real people who share a filed name, district, and body — at any
distance in time, even the same instant — collapse onto one `politician`
row, because the SQL simply cannot see that they are different humans.

Two confirmed live instances, same root cause:

- **JULIO CESAR DOS SANTOS** (BA, `DEPUTADO FEDERAL`, both 2018) — two real
  candidacies filed ~0.7s apart in the same seed pass. Fixed by a one-off
  repoint (`crates/worker/src/bin/fix-br-julio-cesar-santos-ba-2018.rs`,
  commit 47f9c3c). This particular *same-pass* sub-case is now also guarded
  going forward by `crates/adapters/br/src/seed.rs::identity_collision_counts`
  (added after the fix), which refuses to seed either candidate when 2+
  DISTINCT candidates in one discovery pass share `(alias, district)` within
  one body. **That guard only sees candidates within a single
  `seed_candidates_year` call — it cannot see a match against a politician
  seeded in a DIFFERENT call (a different year).**
- **CARLOS ALBERTO DE SOUZA** (SP, `DEPUTADO FEDERAL`, 2014 vs 2022, 8 years
  apart) — found via `check-br-identity-collisions` after the 2014 real
  write (`agents/JOURNAL.md` 2026-07-09). This is exactly the gap the
  same-pass guard above cannot close: the 2014 candidate was seeded first;
  the 2022 candidate's `seed_roster` call found "already seeded" (1 hit,
  `roster.rs:51`) and silently reused the existing row. **Not fixed** —
  flagged for this goal.

Both are the *same* underlying bug (resolve_hits has no way to tell two
same-name-district-body people apart), differing only in whether the two
candidacies were seeded in the same call or different calls. A fix at the
`resolve_hits` level closes both instances with one mechanism, and prevents
a third (same-body-same-name, N years apart, discovered in a THIRD call)
before it happens.

## 2. Survey: does every live/near-live regime have a durable per-filer ID?

Five parallel scouts read every non-`br` regime's `docs/regimes/*.md`
Source Authority File plus its adapter source. Summary:

| Regime | Durable person ID in raw source? | Currently wired into `roster.rs`? | Confirmed collision precedent |
|---|---|---|---|
| `br` | **Yes** — `NR_CPF_CANDIDATO` (masked to sentinel `-4` from the 2024 cycle on) + `NR_TITULO_ELEITORAL_CANDIDATO` (voter-registration number, unmasked in every cycle checked, incl. 2024) — `docs/regimes/br/AUTHORITY.md` `identifiers_available` | **Yes**, live, real data | **2 confirmed** (JULIO CESAR, CARLOS ALBERTO) |
| `us_house` | **None** — index XML + PDF carry name/district/`DocID` (a document id, not a person id) only | Yes, live, real data | None confirmed; not searched for beyond this survey |
| `us_senate` | **None** — listing + report page carry name only (no state, no member id) | Yes, live, real data | Doc itself flags an *unmitigated* risk: "100 sitting members; small collision surface but NOT zero" (`docs/regimes/us_senate.md:174-176`) |
| `uk_commons_register` | **Yes** — `member.id` (parliament.uk MNIS numeric id), already captured through Silver/`details.member_id` | **No** — production identity is `IdentityMode::Unbound` (nil-ULID placeholder); no `RunnerBinding` built yet | None (regime not live through roster.rs) |
| `canada_ciec` | **Yes** — `clientId` GUID, already captured through Silver/`details.client_id` | **No** — same `Unbound` placeholder; `roster.rs`/`clientId` join explicitly deferred in `adapter.rs`'s own doc comment | None (not live) |
| `australia_register` | **None** — name + electoral division + state only, even after LLM-vision extraction; filename disambiguators (`ChesterD`/`ChestersL`) are document keys, not person ids | Not built yet | None confirmed, but same-surname-different-electorate pairs already exist in real data |
| `eu_parliament` | **Yes** — `mep_id` (europarl.europa.eu), captured in Silver/`details.mep_id` | **No** — `Unbound` placeholder, no roster.rs call at all | None (not live) |
| `france_hatvp` | **Partial** — `id_origine` ("tribun" id) exists in the raw discovery index (`liste.csv`) and is documented as a strong join key, but is **not yet plumbed into `SilverRow`** in `fr.rs` | **No** | None (not live) |
| `germany_bundestag` | **Yes** — `mdb_id` (Bundestag biografien id), captured in Silver/`details.mdb_id` | **No** — same `Unbound` placeholder | None (not live) |

**Conclusion driving scope**: today, only three regimes actually call
`resolve_hits` with real production data — `us_house`, `us_senate`, `br`.
Of those, only `br` has a durable ID available. The other six regimes
(`uk_commons_register`, `canada_ciec`, `australia_register`,
`eu_parliament`, `france_hatvp`, `germany_bundestag`) are pre-`RunnerBinding`
stubs — wiring each one's own `roster.rs` integration is separate, future,
per-regime work (already tracked in each adapter's own doc comments), out of
scope here. What IS in scope, per the goal's explicit ask, is making the
*shared mechanism* ready for them: a universal, nullable signal that costs
nothing when absent and that br can start using today.

## 3. Design: a universal `external_identifier` signal

### 3.1 Schema (expand-only migration)

```sql
alter table politician add column external_identifier text;
```

One nullable column on `politician` (a real person, not a filing) — not
scoped per regime, not unique-constrained. Rejected alternatives:

- **A separate `politician_external_id` table keyed by regime** — over-built
  for what's needed today (every live regime has at most one ID namespace
  per politician; nothing in this schema lets one politician hold mandates
  under two different national ID systems in practice). If that ever
  becomes real, it's an additive follow-up, not a blocker now.
- **A `unique` constraint on the column** — rejected. The column mixes
  namespaces across regimes (a CPF and an MNIS id are both plain `text`);
  a hard uniqueness constraint would risk a spurious conflict if two
  unrelated regimes' id-spaces ever coincided, for a benefit (global
  duplicate detection) nothing here needs. Per-regime uniqueness is already
  implied by `resolve_hits`'s own `(alias, district, body)` scoping, which
  stays regime-local.

No existing column is touched; `wikidata_qid` (a different, cross-regime
universal id) is untouched and orthogonal.

### 3.2 Plumbing: two structs gain one optional field each

- `pipeline::stages::roster::RosterMember` gains
  `pub external_identifier: Option<String>` — the seed-time signal.
- `pipeline::run::FilingIdentity` gains
  `pub external_identifier: Option<String>` — the publish-time signal.

Every existing construction site outside `br` sets this to `None`:
`us_house::seed::roster_from_index_xml` (`RosterMember`),
`us_house::binding::UsHouseBinding::filing_identity`,
`crates/worker/src/bin/local_br.rs`'s demo roster (a bounded local proof
binary, not real data), and the three test-only `FilingIdentity` literals in
`crates/pipeline/tests/{backfill_suppression,e2e_local,publication_gates}.rs`.
This is a mechanical, zero-behavior-change addition for every one of those
sites — proven by the existing test suites staying green unchanged.

`br` populates both from the SAME source field, at the two points that
already read `consulta_cand`:

- `crates/adapters/br/src/seed.rs::extract_identity` (seed time, reads the
  raw joined-declaration bytes directly — already independent of the
  `ctx.pool.is_some()` PII gate that governs `SilverRow`, since it reads
  Bronze bytes, not Silver) gains `NR_CPF_CANDIDATO`/
  `NR_TITULO_ELEITORAL_CANDIDATO` extraction.
- `crates/adapters/br/src/binding::BrBinding::filing_identity` (publish
  time, reads the already-staged `stg_br` row) uses its existing
  `nr_cpf_candidato`/`nr_titulo_eleitoral_candidato` fields (already staged
  today, just never read for resolution).

Both sites apply the SAME selection rule: prefer CPF when it is present and
not the documented sentinel `"-4"` (`AUTHORITY.md` `regime_versions`, 2024
amendment); otherwise fall back to the voter-registration number, which
`AUTHORITY.md` confirms stays unmasked in every cycle checked so far,
including 2024. `None` only when neither field is present. This value is
used purely as an internal resolution key — same PII handling discipline as
today (never surfaced past Bronze/internal resolution, never added to any
public-facing struct or API response).

### 3.3 `resolve_hits`: the actual disambiguation

`resolve_hits` changes from a plain existence query to a filtered one. New
signature (private fn, only 2 callers, both in this same file — zero blast
radius outside `roster.rs`):

```rust
async fn resolve_hits(
    executor: E,
    regime: &RegimeBinding,
    filer_name: &str,
    district: &str,
    external_identifier: Option<&str>,
    as_of_year: Option<i32>,
) -> anyhow::Result<Vec<String>>
```

Query widens to also select each candidate hit's stored
`external_identifier` and earliest `mandate.start_date` year:

```sql
select p.id, p.external_identifier, min(m.start_date) as earliest_start
from politician p
join politician_alias a on a.politician_id = p.id
join mandate m on m.politician_id = p.id
where a.alias = $1 and m.district = $2 and m.body = $3
group by p.id, p.external_identifier
```

Then, in Rust, each raw hit is kept or excluded:

1. **Both sides have an external_identifier and they differ** → excluded
   (confirmed different person — this is the JULIO CESAR/CARLOS ALBERTO
   case, now caught regardless of same-pass or cross-pass).
2. **Both sides have an external_identifier and they match** → kept,
   confirmed (skips the year-window check entirely — an ID match is
   strictly stronger evidence than a date heuristic).
3. **The incoming identifier is `None`, OR the stored one is `None`
   (pre-fix legacy row, or a regime with no ID at all)** → fall through to
   the year-window check (§3.4). This is deliberately permissive: it
   preserves today's behavior for every politician seeded before this fix
   (all of which have `external_identifier = NULL`), so nothing already
   correctly resolved regresses.

`seed_roster`'s existing `hits.len()` match arms (`1 => already seeded`,
`0 => insert new`, `n => bail, ambiguous`) are **unchanged** — the new
exclusion logic lives entirely inside `resolve_hits`, so a candidate whose
ID conflicts with the existing hit now naturally sees `0` hits (not "already
seeded") and falls through to the existing insert path, correctly minting a
new politician. `resolve_politician`'s existing `[one] => Some, _ => None`
match is likewise unchanged.

Consequence that must hold for this to work end to end: **`external_identifier`
must be stored on `politician` at INSERT time** (`seed_roster`'s existing
`insert into politician (id, canonical_name) ...` gains a third column) so
that by the time `resolve_politician` runs at publish time (after seeding,
per the goal's own stated per-year command order:
`seed-br-candidates` → `backfill-real-br`), a hit whose ID conflicts is
already excludable — otherwise splitting two people at seed time would
leave `resolve_politician` facing 2 unconfirmed hits it cannot pick between
at publish time, turning a silent-merge bug into a total resolution failure
for both people instead of a fix. No opportunistic backfill-on-read is
added (that would turn a read path into a writer); the one place
`external_identifier` is ever written is `seed_roster`'s own insert, plus
the one-off `fix-br-*` reconciliation bins for pre-existing rows (§4).

### 3.4 Year-window fallback (regimes without any ID)

When neither side of a hit has a usable `external_identifier` (`us_house`,
`us_senate`, and every pre-fix `br` row), the ONLY remaining signal is
plausibility of a single person's tenure span. If `as_of_year` (the
incoming record's year — `RosterMember.active_year` at seed time,
`identity.filed_date`'s year at publish time) and the hit's stored
`earliest_start` year are both available, and their absolute difference
exceeds `MAX_PLAUSIBLE_TENURE_YEARS`, the hit is excluded (fails closed —
`unresolved_filer`/a fresh politician gets minted — never silently merged).
When either date is unavailable, the check is skipped (permissive, matches
today's behavior) rather than invented.

**Threshold: `MAX_PLAUSIBLE_TENURE_YEARS = 65`.** Justified against the
longest real, documented careers in exactly the bodies this project
resolves against today, not an arbitrary term-length multiple: John Dingell
(US House, 59 years, 1955-2015) and Strom Thurmond (US Senate/House
combined career well past 48 years) are the extreme real-world ceiling for
these two currently-live non-`br` regimes; Brazilian federal deputies with
40+ year careers are documented but shorter than the US extremes. 65 years
gives comfortable headroom above every documented real case (a candidate
entering politics at 20 and still filing at 85) without being so loose it
stops meaning anything.

**Explicitly acknowledged limitation** (CLAUDE.md "surface tradeoffs, don't
hide confusion" — stating this plainly rather than quietly overselling the
mechanism): this fallback is weak by construction. CARLOS ALBERTO's real
gap was 8 years — utterly ordinary for a genuine re-candidacy, and no
threshold that avoids false-positiving on real multi-decade careers can
catch a gap that small. The year-window check only catches truly extreme,
implausible gaps (approaching or exceeding a human political-career
lifespan); it is not a general solve for same-name-different-person
collisions in ID-less regimes. The `external_identifier` mechanism is the
real fix; the year-window is defense-in-depth for the regimes that have
nothing better, with the residual risk left honestly open (matches
`us_senate.md`'s own already-documented "small but not zero" acknowledgment)
rather than presented as solved.

### 3.5 Zero-behavior-change proof for every currently-live non-`br` case

- `us_house`/`us_senate`: `external_identifier` is always `None` on both
  sides of every hit (no source data to populate it) → rule 3 (year-window)
  is the only path ever taken, and 65 years exceeds every real filed date in
  this project's data (earliest `us_house` fixture/backfill years are
  ~2012-2026) → the check never fires → identical to today's unconditional
  match.
- Every already-seeded `br` politician (pre-fix): `external_identifier =
  NULL` → same rule-3 path; `br`'s real historical span in scope (2014-2022
  today, 2006-2026 at the widest per goal 092/this goal) is at most ~20
  years → never exceeds 65 → identical to today's unconditional match.

## 4. CARLOS ALBERTO DE SOUZA: retroactive re-split (decision, grounded)

**Decision: fix now, mirroring JULIO CESAR's exact template — not a
corrections-log-only entry.**

Grounded directly in `br-identity-collision-remediation.md` §9's own closing
argument, which explicitly anticipated this: *"this plan (and its §2 sweep)
is a reasonable template for [future reconciliation]"* — and its overall
assessment (§10): *"safe to execute as a normal follow-up... dev-only,
schema-untouched, small-blast-radius, fully-diagnosed, mechanically
verifiable data correction with no open ambiguity."* Every one of those
properties holds identically for CARLOS ALBERTO: same defect class (one
`politician` row, 2 distinct CPFs via the standing
`check-br-identity-collisions` sweep — report-only, already re-run and
already found this exact row), same fix shape (mint one new
`politician`/`politician_alias`/`mandate` row for the smaller side, repoint
that side's `filing`/`disclosure_record`, recompute the moved record's
fingerprint, leave `outbox_event`/`pipeline_run` alone with the same
reasoning JULIO CESAR's plan gave), same verification plan (row-count
sweep back to zero, per-person profile completeness, CPF re-check against
raw Bronze, fingerprint self-consistency, idempotent re-run). A new one-off
bin, `crates/worker/src/bin/fix-br-carlos-alberto-souza-sp.rs`, follows
`fix-br-julio-cesar-santos-ba-2018.rs` field-for-field: dry-run by default,
`--execute` to write, one transaction, pre-change snapshot, before/after
report. Built and reviewed before its first `--execute` run against the
shared dev DB (same elevated review-before-run gate JULIO CESAR's plan
required for the first-ever politician-identity split — this is the
second, so the precedent is now established, but the gate is cheap and the
stakes are the same).

## 5. Regression plan

- New unit tests in `crates/pipeline/src/stages/roster.rs`'s own `#[cfg(test)]`
  module (or a new `pipeline/tests/roster_identity.rs` sqlx-gated suite,
  matching `roster_historical.rs`'s convention): (a) two candidates, same
  alias/district/body, same pass, different `external_identifier` → both
  seed as distinct politicians (JULIO CESAR shape); (b) same, but across two
  SEPARATE `seed_roster` calls (CARLOS ALBERTO shape); (c) a legacy row with
  `external_identifier = NULL` still resolves for a same-ID follow-up filing
  (backward compatibility); (d) an implausible year gap with no ID on either
  side fails closed; (e) a plausible year gap (e.g. 8 years, CARLOS
  ALBERTO's real gap) with no ID on either side still merges (proving the
  fallback's honest limitation, not silently "fixing" what only the ID
  mechanism can fix).
- `roster_historical.rs` (`us_house`): unchanged expectations, `None`/`None`
  passed at both call sites — must stay green byte-for-byte.
- `cargo run -p pipeline --bin conformance -- <adapter>` for every adapter
  (`br`, `us_house`, `us_senate`, `uk_commons_register`, `canada_ciec`,
  `australia_register`, `eu_fr_de_annual`, `fixture_fake`) — conformance
  never reaches `roster.rs` (adapter.parse()/normalize() only), so this
  proves the struct-field addition doesn't break serialization/fixtures
  anywhere, independent of the resolution-logic change.
- Full workspace: `cargo fmt --check`, `cargo clippy --all-targets -- -D
  warnings`, `cargo test --workspace`.
- Re-run `check-br-identity-collisions` after the CARLOS ALBERTO fix — must
  return `PASS: zero`.
