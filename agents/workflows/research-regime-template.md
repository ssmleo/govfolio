# Template — "research regime for country X"

A repeatable, copy-paste task for advancing ONE stub jurisdiction one phase along
the coverage-factory state machine (design §5.8). It does **not** re-specify the
machinery — it points at it. Authoritative playbook:
[`agents/workflows/source-exploration.md`](source-exploration.md). Registry state
that gates the work: `jurisdiction.coverage_phase`
(`stub → scouted → surveyed → sampled → specced → built → live | blocked`),
seeded worldwide by goal 065 (`crates/core/src/seed/`).

## Before you start
- **Claim the lease.** Use `jurisdiction-lease claim`, retain the returned generation,
  and use generation-CAS renew/abandon only before receipt submission. Never edit lease
  or phase fields directly. Commit locally, submit the immutable receipt, and wait for
  the singleton integrator. One jurisdiction per loop.
- **Pick the phase.** Run the phase that follows the jurisdiction's current
  `coverage_phase`. Do exactly one phase; its validated artifact is "done".
- **Epoch gate.** Only advance jurisdictions in the open epoch (`agents/EPOCHS.md`);
  within the epoch, highest `priority_score` first.

## The phase you run (from source-exploration.md — do not duplicate, read it)
| From → to | Role | Artifact | Gate / validator |
|---|---|---|---|
| stub → scouted | scout | `docs/regimes/<x>/sources.yaml` | `cargo run -p pipeline --bin validate-sources -- <x>` |
| scouted → surveyed | surveyor (+ auditor) | `docs/regimes/<x>.md` front-matter (RegimeSurvey) | `cargo run -p pipeline --bin validate-survey -- <x>`; auditor re-derives each claim from evidence |
| surveyed → sampled | sampler | `crates/adapters/<x>/fixtures/*` + MANIFEST | `cargo run -p pipeline --bin validate-manifest -- <x>` |
| sampled → specced | spec-writer + test-designer (+ auditor) | `plan.md` + `src/details.rs` + draft `expected.*.json` | schemas snapshot-committed; expecteds per automation policy |
| specced → built | planner (if large) + rust-builder | conformance-green adapter | `cargo run -p pipeline --bin conformance -- <x>` |
| built → live | (wiring) | `LIVE_REGIMES` entry + roster/seed wiring; sentinel `live_targets()` | registry `coverage_phase='live'`; `cargo test -p worker sentinel` |
| any → blocked | (any) | `blocked_reason` on the row + a filed goal | fail closed; evidence of what was tried |

## Fill the same shape the built regimes use
The 6 built regime docs are the reference structure to imitate, NOT invent around:
`docs/regimes/us-house.md`, `us_senate.md`, `uk_commons_register.md`,
`canada_ciec.md`, `australia_register.md`, `eu_fr_de_annual.md`. Each has
validated RegimeSurvey front-matter (see the field list + minimal schemas in
[source-exploration.md §"Artifact schemas"](source-exploration.md) and the
`docs/regimes/_templates/` template it names) and a §1 "Regime metadata" table:
jurisdiction · body · `regime_type` · `value_precision` · cadence ·
`disclosure_lag_days` · `source_url` · `effective_from`. Those §1 fields are
exactly what a `disclosure_regime` row carries (`crates/core/src/seed/mod.rs`
`LiveRegime`) — promoting a stub to a real regime row means filling them from
evidence.

## Non-negotiables (design §5.8 / root CLAUDE.md invariants)
- **Evidence or it didn't happen.** Every claim: `{claim, evidence:[archived file]}`.
  Archive each page you rely on to `docs/regimes/<x>/evidence/` (sha-named), same PR.
- **`unknown` beats confabulated** — legal only with a what-was-tried log.
- **Never guess** entities/thresholds; below-threshold stays NULL + review_task.
- **Politeness** (invariant 10): identified UA + contact, concurrency 1, min
  interval, conditional GETs; a hard block ⇒ fail closed to a work item, no evasion.
- **Auditor independence** — never audit your own production.
- **Write-back is part of done** — new quirks land in the regime doc in the same PR.

## Done
The phase's validator and auditor pass, the work and SAF write-back are committed locally,
and an immutable receipt is submitted. Done is the receipt reaching `applied`: only then
has the integrator proven the exact commit on green `origin/main`, appended the canonical
JOURNAL line, and projected the registry phase/lease state.
