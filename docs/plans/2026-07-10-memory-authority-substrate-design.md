# Memory + Authority Substrate v2 — design

- **Date:** 2026-07-10
- **Status:** Founder-approved 2026-07-10.
- **Provenance:** brainstorming session + 3-agent repo exploration (facts verified by direct
  file reads) + SOTA web-research pass (citations re-verified by direct fetch). All
  load-bearing facts and citations are inlined: self-sufficient with zero session context.
- **Implementing goals:** 100–103, executed by the autonomous loop. Goal files cite this
  doc's § numbers; sections MUST NOT be renumbered.
- **Relationship to `docs/plans/2026-07-04-govfolio-design.md`:** extends the agent-governance
  layer (D9's SAF pattern, the loop protocol); **no data-plane changes** — Bronze/Silver/Gold
  semantics, schemas, and pipeline stages are untouched.

## 1. Problem & context

govfolio is **fully agent-run**: development, prod maintenance, and (soon) business execution
all happen inside the autonomous loop (`agents/run-loop.sh` pipes `agents/PROMPT.md` into a
fresh `claude -p --dangerously-skip-permissions` session, forever). Between iterations the
system's only memory is the repo and its only law is the file set the prompt loads. Memory and
authority files are therefore **production infrastructure, not documentation**, and get the
data plane's engineering treatment: schemas, validators, fail-closed gates, tamper evidence.

Exploration (2026-07-10, verified by direct file reads) found six gaps:

1. **Episodic amnesia.** `agents/JOURNAL.md` is a single append-only file, ~91KB / ~96
   entries; the contract "one line per iteration" has already strained into multi-paragraph
   incident post-mortems. The loop reads ONLY the tail (`agents/PROMPT.md` load order: "tail
   of agents/JOURNAL.md"). No compaction, no per-topic retrieval, no rotation — everything
   older than the tail is functionally forgotten unless grepped by luck.
2. **Invariant 9 is prompt-enforced only.** The goal-queue allow-list (root `CLAUDE.md`
   invariant 9) is prose: `agents/workflows/orchestration.md` step 0 INTEGRITY is
   instructions, not a mechanism. NO CI job, NO script, NO Rust test enforces the
   goals↔INDEX bijection. `crates/api/src/routes/admin/loop_meta.rs` parses the index for the
   dashboard but never enumerates the goals dir. §3/§8 show why prompt-only authority is a
   known-exploited failure mode.
3. **SAF layout drift.** Only `br` and `us_house` use the directory form
   (`docs/regimes/<x>/AUTHORITY.md` + `sources.yaml` + `evidence/`); five are flat files:
   `docs/regimes/canada_ciec.md` (64.7K), `australia_register.md` (55K), `eu_fr_de_annual.md`
   (50K), `uk_commons_register.md` (49.5K), `us_senate.md` (43K). Naming drift: `us_house`
   has BOTH `docs/regimes/us_house/AUTHORITY.md` AND the legacy flat `docs/regimes/us-house.md`
   (36.6K) AND hyphenated `docs/regimes/us-house/evidence/` + `us-house/reference/`;
   `us_senate` evidence lives in hyphenated `docs/regimes/us-senate/evidence/`.
4. **No memory home for non-source knowledge.** Subsystems (api/web/pipeline/worker/infra/
   contracts) and env/ops knowledge (prod incidents, deploy lessons, cost anomalies) have no
   per-domain file with a write-back contract; lessons land in the JOURNAL and rot there.
5. **Fragmentation.** Source knowledge is spread across the SAF quirks log, `sources.yaml`
   (br + us_house only), `crates/adapters/br/plan.md` (br only), fixtures MANIFEST,
   `evidence/` dirs, and the global JOURNAL — no index or retrieval layer over any of it.
6. **No staleness/provenance metadata** on remembered facts: nothing records when a claim was
   last verified against the world, so stale and fresh facts are indistinguishable.

## 2. Prior art in-repo

The design generalizes patterns this repo has already proven; nothing in this section changes.

- **SAF (Source Authority File)** — `docs/regimes/<x>/AUTHORITY.md`, per D9. Self-description
  (template `docs/regimes/_templates/AUTHORITY.template.md`): "Living canonical context.
  Specialists MUST load this before any source-scoped task and MUST write back new learnings
  in the same PR (definition of done)." Validated YAML frontmatter = the `RegimeSurvey` schema
  (every claim `{claim, evidence[]}`; `unknown` legal only with a non-empty `tried:` log);
  five mandatory body sections: Data catalog · Field mapping (source → gold) · Parse strategy
  & rationale · Quirks log (append-only, dated) · Operational notes. Enforced fail-closed by
  `cargo run -p pipeline --bin validate-survey -- <x>`. §4.1 is this contract generalized.
- **JOURNAL** — `agents/JOURNAL.md`, append-only episodic log, one entry per iteration.
  The append-only discipline is proven; the shape and retrieval are what §4.1/§4.3 fix.
- **E1.lock.json precedent** — `docs/regimes/us-house/reference/E1.lock.json`: sha256 pins
  over fixtures/expected/schemas/SAF/evidence, policy: "Pinned artifacts are IMMUTABLE:
  amending any pinned file requires SUPERSEDING this lock (version bump + note of what changed
  and why), founder-gated like role edits — see docs/decisions/role-eval-thresholds.md".
  Verified mechanically by `cargo test -p pipeline role_evals` and
  `crates/pipeline/src/bin/epoch_gate.rs`. §4.2 is this pattern generalized from one epoch
  corpus to the whole agent-governance file set.
- **validate-\* factory** — `crates/pipeline/src/factory.rs` is the fail-closed validator
  home (`validate-sources` | `validate-survey` | `validate-manifest` bins). The new bins
  (`validate-authority`, `validate-memory`, `memory-index`, `memory-staleness`) join this
  family: same crate, same fail-closed exit discipline, same `#[test]` coverage.
- **Guardrail scripts** — `scripts/check-migration-safety.sh`, `scripts/check-tf-plan.sh`
  (per `docs/decisions/automation-policy.md`): mechanical fail-closed checks instead of
  human gates are the accepted repo pattern. §4.2 applies it to invariant 9.
- **saf-authoring skill** — `agents/skills/saf-authoring/SKILL.md` names the exact
  anti-pattern the whole substrate exists to prevent: "knowledge left in transcripts".
- **Task/decision memory** — goal files accrete `## Tn progress` + `## BLOCKED (human)`
  blocks; durable decisions live in `docs/decisions/*.md`. Both stay as-is.

## 3. Evidence base

**Headline: the evidence converges TOWARD git-file-based memory, not away from it.** Letta
(the MemGPT team) benchmarked an agent with NO memory system — just files + iterative grep —
at 74.0% on LoCoMo, beating mem0's best published config (68.5%)
(https://www.letta.com/blog/benchmarking-ai-agent-memory, 2025-08-12). And the 2025 production
ecosystem converged on frontmatter-described markdown + a tiny index as THE memory shape:
Anthropic Agent Skills
(https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills),
Cursor rules (https://cursor.com/docs/context/rules), Devin Knowledge
(https://docs.devin.ai/product-guides/knowledge), OpenHands microagents
(https://docs.openhands.dev/openhands/usage/microagents/microagents-overview).

**Adopted patterns** (research-pass verdicts; P-numbers used for traceability in §4/§8):

- **P1** Bounded always-loaded memory blocks with enforced size caps — the cap forces
  curation (MemGPT, arXiv 2310.08560; Letta memory blocks).
- **P2** Hierarchical index → leaf files: tiny always-loaded index, bodies on demand
  (Claude Code auto-memory, https://code.claude.com/docs/en/memory; Anthropic context
  engineering, https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents).
- **P3** Frontmatter `description` (trigger-phrased) + `triggers`/`aliases`/`paths` as THE
  retrieval index — the embedding substitute for self-authored corpora.
- **P4** Reflection/consolidation off the hot path: a dedicated pass distills the episodic
  log into semantic topic files, each insight citing its source entries (Generative Agents,
  arXiv 2304.03442; Letta sleep-time compute, arXiv 2504.13171; LangMem ReflectionExecutor).
- **P5** Delta-only updates, NEVER monolithic LLM rewrite: ACE "context collapse" — one
  rewrite step collapsed 18,282 tokens → 122 and dropped accuracy 66.7% → 57.1%, below the
  63.7% no-context baseline (arXiv 2510.04618, ICLR 2026).
- **P6** Bi-temporal staleness fields (`observed`/`last_verified`/`superseded_by`);
  invalidate, don't delete (Zep/Graphiti, arXiv 2501.13956 — fields yes, graph engine no).
- **P8** Session-handoff via fixed schema fields, not freeform prose (Anthropic memory-tool
  docs; OpenHands condenser — 2× cost cut at equal SWE-bench solve rate; Amp handoffs,
  https://ampcode.com/news/handoff — summary-on-summary degrades).
- **P11** Agentic grep over embeddings for self-authored corpora; revisit only past ~1k files
  (Claude Code dropped RAG: https://news.ycombinator.com/item?id=43164253; Augment:
  https://jxnl.co/writing/2025/09/11/why-grep-beat-embeddings-in-our-swe-bench-agent-lessons-from-augment/).
- **P12** Single-writer partitioned state: parallel agents read everything, write only their
  partition (Cognition, https://cognition.com/blog/multi-agents-working, 2026-04-22; MAST:
  36.9% of multi-agent failures are inter-agent misalignment, arXiv 2503.13657).
- **P13** Condensed, schema'd write-backs; raw traces never merge into shared memory
  (https://www.anthropic.com/engineering/multi-agent-research-system).
- **P14** Fresh-context reviewer: verification agents deliberately get NO shared context
  (Cognition Devin Review — avg 2 bugs/PR, ~58% severe).
- **P15** Lockfile-pinned instruction allow-list: path → sha256 over executable-instruction
  files, pre-flight verifier fails closed (npm lockfile model; in-toto/SLSA, https://slsa.dev/).
  No shipped product composes this end-to-end — the composition IS the 2026 SOTA answer.
- **P16** Hook-enforced gating below the model. Claude Code docs, verbatim: "Permission rules
  are enforced by Claude Code, not by the model"; a PreToolUse hook exiting 2 blocks the tool
  call even in bypassPermissions mode (https://code.claude.com/docs/en/permissions).
- **P19** Untrusted-content demarcation + lethal-trifecta triage (Willison,
  https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/; Microsoft spotlighting,
  arXiv 2403.14720). Government-disclosure text is attacker-influenceable third-party content.
- **P20** Constitution-with-reasons + append-only amendment log — stated reasons generalize
  better than bare rules (https://www.anthropic.com/news/claude-new-constitution).

**Rejected alternatives** (each considered and declined, with receipts):

- **mem0-style arbitrated ADD/UPDATE/DELETE memory writes** — an LLM arbiter deciding what to
  overwrite. mem0 itself abandoned arbitration in production for ADD-only (arXiv 2504.19413;
  https://mem0.ai/research). Invariant 1 (supersede, never update) already encodes the
  surviving policy; we apply it to memory entries.
- **Knowledge-graph / vector memory engines** (Zep/Graphiti-class, embedding stores) —
  server-shaped, unauditable by git diff, and the evidence (P11, headline) says grep + index
  wins at this corpus size. We keep the bi-temporal *fields* (P6) and drop the engine.
- **Git files as a multi-writer, high-churn task DB** — beads tried it and pivoted to Dolt
  within ~9 months (https://github.com/steveyegge/beads). Validates our existing split:
  claim/lease/freeze state lives in Postgres, knowledge lives in git (§4.3 boundary rule).

## 4. Design

Three planes: memory (what the system knows), authority (what it may do), hygiene (how both
stay trustworthy). All validators join the `crates/pipeline/src/factory.rs` fail-closed family (§2).

### 4.1 Memory plane

**One contract, N domains** — the SAF pattern generalized. A memory file is markdown with
schemars-validated YAML frontmatter (`MemoryFile` type in `crates/core`, validated by the
`validate-memory` bin). Frontmatter fields:

| Field | Type | Meaning |
|---|---|---|
| `domain` | enum: `regime` \| `subsystem` \| `ops` | Memory domain. `business` is reserved in the schema but NOT yet valid — validator rejects it until business-execution loops land. |
| `scope` | string | What this file is authoritative for (e.g. `docs/regimes/br`, `crates/api`). |
| `description` | string, one line | Trigger-phrased: "Load when …" — this line IS the retrieval index entry (P3). |
| `triggers` | list of strings | Task phrases that should cause a load. |
| `aliases` | list of strings | Alternate names/spellings (e.g. `us-house` for `us_house`). |
| `paths` | list of globs | Repo paths whose modification implies this file is relevant. |
| `last_verified` | date | When a human-of-record or agent last verified the load-bearing facts against the world (P6; feeds §4.3 staleness). |
| `regime_code` | string, optional | Join key to pg operational state (§4.3 boundary rule). Required when `domain: regime`. |
| `max_kb` | int, default 64 | Size budget, validator-enforced (P1). Exceeding it fails validation and forces a §4.3 consolidation pass. |

Mandatory body sections (the lean three-section shape):

- `## Context` — current distilled truth about the domain.
- `## Log (append-only, dated)` — entries carry IDs `[YYYY-MM-DD-nn]`; corrections add a new
  entry with `superseded_by:` back-references on the old one. Entries are never edited or
  deleted: invariant 1 (supersede, never update) applied to memory (P5, P6).
- `## Open questions` — known unknowns, each with what was tried.

**Regime files keep their richer shape.** `docs/regimes/<x>/AUTHORITY.md` keeps the full
`RegimeSurvey` frontmatter and the five SAF body sections (§2); the memory-contract fields
above are ADDED to the SAF frontmatter, not a replacement. The SAF quirks log adopts the
`[YYYY-MM-DD-nn]` entry-ID convention. Subsystem/ops files use the lean three-section shape.

**Locations.** Regimes stay at `docs/regimes/<x>/AUTHORITY.md`, all 7 normalized to
directory form with hyphen/underscore drift resolved (goal 102). New:
`docs/memory/subsystems/{api,web,pipeline,worker,infra,contracts}.md`,
`docs/memory/ops/{prod-incidents,deploys,cost}.md`, and
`docs/memory/_templates/MEMORY.template.md` (mirrors
`docs/regimes/_templates/AUTHORITY.template.md`).

**Index.** `docs/memory/INDEX.md`, GENERATED by the `memory-index` bin from frontmatter,
never hand-edited: one line per memory file (`path · domain · description`), ALL domains
including regimes. Loaded every iteration via the `agents/PROMPT.md` load order (§6) — tiny
index, bodies on demand (P2). CI drift gate: regeneration = no diff. `validate-memory` checks
index↔filesystem bijection: every memory file has exactly one row; every row's path exists
and validates. Retrieval = index + grep (P11); no embeddings — corpus far under ~1k files.

**Episodic layer.** `agents/JOURNAL.md` stays the append-only episodic log; the loop's
tail-read behavior is unchanged. Two changes (goal 103):

- Entry schema (P8, P13): `date | role | goal | outcome | evidence pointers | blockers`,
  hard cap ~120 words per entry. Long-form incident analysis goes into the relevant domain
  memory file's Log (with an ID) or `docs/decisions/`; the journal entry points at it.
- Rotation: after each §4.3 consolidation pass, the consolidated span moves to
  `agents/journal/archive/YYYY-MM.md`; `agents/JOURNAL.md` keeps only the unconsolidated
  tail. Archives remain grep-able history; nothing is deleted.

### 4.2 Authority plane

**Invariant 9 becomes mechanical.** Prompt-only enforcement is not a boundary (P16, §8);
E1.lock.json's tamper evidence (§2) is generalized to the whole agent-governance file set.

**`agents/AUTHORITY.lock.json`.** Schema:
`{version: int, superseded_note?: string, pinned: {<repo-path>: <sha256-hex>}}`.
`superseded_note` records what changed and why on every version bump (E1's supersede policy,
§2). Pinned set: `agents/GOVERNANCE.md`, `agents/PROMPT.md`, `agents/LOOP.md`,
`agents/workflows/orchestration.md`, `agents/roles/*.md`, `agents/archetypes/*.md`,
`agents/EFFORT.md`, `agents/EPOCHS.md`, `agents/goals/000-INDEX.md`. Goal files are NOT
content-pinned — legitimately mutable (progress blocks, checklists) — and are covered by the
bijection check instead. The lock is regenerated only via `validate-authority --write-lock`,
permitted only through the amendment path below.

**`validate-authority` bin** (fail-closed, `crates/pipeline` factory family). Checks:

(a) **Bijection:** every `agents/goals/*.md` (template excepted) MUST be listed in
    `agents/goals/000-INDEX.md`. An unlisted goal file → exit 1 + a quarantine report with
    git provenance (`git log -- <file>`) — the mechanical form of orchestration.md step 0.
(b) **Hash match:** every pinned path's sha256 matches the lock; missing or extra authority
    files → exit 1.
(c) **Amendment discipline:** any commit whose diff touches authority files MUST update the
    lock in the same commit AND its message MUST reference an INDEX-listed goal; else exit 1.
    The commit range validate-authority examines for this check (HEAD only vs. the full
    branch-since-main range) is goal 100's implementation choice; the minimum acceptable
    scope is HEAD.

**Run points** (all three; defense in depth):

1. `agents/run-loop.sh` — pre-iteration halt: run `validate-authority` before piping
   `agents/PROMPT.md` into Claude; non-zero exit stops the loop before a session launches.
2. CI — the `.github/workflows/ci.yml` guardrails job runs the bin on every push; a poisoned
   tree cannot merge.
3. `agents/workflows/orchestration.md` step 0 — the prose INTEGRITY instruction is replaced
   by "run the bin" (quarantine-report duty stays, now fed by the bin's output).

**PreToolUse hook** (repo `.claude/settings.json`, additive — MUST NOT clobber the user's
global RTK hook). The hook calls a PRE-BUILT `validate-authority --check-path <path>` binary
(never `cargo run` per tool call — hook latency budget). It blocks, with exit code 2:

- **Read** of any `agents/goals/*.md` not listed in 000-INDEX (template excepted) — an
  unlisted goal file is untrusted input the model must never even load;
- **Write/Edit** of the authority set, the lock itself, `.claude/settings*`, and the
  `.claude/hooks/` dir — unless the amendment path condition holds.

Per Claude Code's enforcement split (P16), hooks evaluate below the model and still evaluate
under `--dangerously-skip-permissions` — exactly how the loop runs (§1) — so this is the only
enforcement layer that actually binds the unattended loop.

**Amendment path.** Authority files are governed, not frozen: writes to the authority set
are permitted ONLY on an `authority/*` branch whose HEAD commit message references an
INDEX-listed goal. The hook checks the branch name (cheap, synchronous); check (c) checks
the rest (lock updated in-commit, goal referenced) at every run point. Routine queue edits
(ticking or appending a 000-INDEX row) ride the same path — branch naming plus
`--write-lock`, both mechanical. "INDEX-listed" is evaluated against the commit's own tree,
so a commit that adds the goal row and references that same goal is valid.
**Bootstrap note:** the hook is inactive until goal 100 wires it, so goal 100 can land the
lock, the bin, and the hook config on an ordinary goal branch without being blocked by the
mechanism it installs. Goals 101–103 amend pinned files and MUST use the amendment path.

**GOVERNANCE.md changes** (P20): every rule in `agents/GOVERNANCE.md` gains a one-line
rationale (stated reasons generalize better than bare rules), and the file gains an
`## Amendments (append-only, dated)` section — constitution-with-reasons plus an auditable
amendment trail.

### 4.3 Hygiene loop

Memory written but never curated becomes a liability (§8 receipts 4–5): three mechanisms
plus one boundary rule.

**Consolidation.** A standing work item (not a one-shot goal), auto-eligible when 10 loop
iterations OR >20KB of `agents/JOURNAL.md` growth have accrued since the last pass. The pass
(P4): (1) distill unconsolidated journal entries into domain memory files' Logs — every
distilled entry MUST cite its source journal date(s) and commit(s); (2) promote recurring
quirks into the relevant SAF quirks log; (3) rotate the consolidated span to
`agents/journal/archive/YYYY-MM.md` (§4.1); (4) regenerate `docs/memory/INDEX.md`.

Consolidation is **delta-only**: `validate-memory` rejects any diff that deletes or edits a
dated Log entry — corrections supersede (§4.1). Monolithic rewrites of a memory file are
BANNED: ACE measured a single monolithic LLM rewrite collapsing 18,282 tokens → 122 with
accuracy below the no-context baseline (arXiv 2510.04618; P5).

**Staleness.** `memory-staleness` bin, run in CI as a REPORT (warn, not gate): lists
load-bearing facts whose `last_verified` is older than 90 days. Report-only because staleness
needs re-verification work, not a merge block; the report feeds goal filing. Rationale: stale
memory compounds errors in autonomous operation (Anthropic Project Vend,
https://www.anthropic.com/research/project-vend-1).

**Anti-poisoning.** Third-party quoted text (scraped disclosure content, external docs,
remote-service errors) inside memory entries MUST be wrapped in fenced ` ```untrusted `
blocks; `validate-memory` lints for unfenced quoted scrape text where detectable (heuristics:
URLs + quotation blocks in Log entries). Memory files are loaded into future agent contexts,
so untrusted text written today is prompt input tomorrow — the lethal-trifecta path
(https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/; spotlighting, arXiv 2403.14720;
P19). Gov-disclosure text is attacker-influenceable.

**Boundary rule: pg = operational state, git = knowledge. Never mirror.** Operational state
lives in Postgres: `jurisdiction` (the `coverage_phase` state machine and its
`claimed_by`/`claimed_at` lease), `sentinel_watch` (frozen flag), `drift_report`,
`backfill_run`, `pipeline_run`, `review_task`, `sample_audit` — all keyed by bare
`regime_code` text. Knowledge lives in git (memory files, SAFs, decisions). Neither side ever
mirrors the other; the join key is the frontmatter `regime_code` (§4.1). Receipts: beads'
git-as-task-DB pivot (§3) validates the pg side, and the 2026-07-07 Bronze-evidence incident
(JOURNAL: 21,253 files recovered by sha256 re-match after scratch and durable storage were
conflated) is the in-repo lesson that stores must not be conflated.

## 5. Goal mapping

Dependency order: **100 first** (security precedes content), then **101 before 102 and 103**
(both need the contract, validators, and index); 102 and 103 are mutually independent.
Goals 101–103 amend lock-pinned files and MUST use the §4.2 amendment path. Goal files copy
these acceptance blocks verbatim; the bin names, test filters, and flags below are normative
surfaces.

### Goal 100 — `100-authority-lock-and-validator.md`

**Objective:** make invariant 9 mechanical — tamper-evident authority lock, fail-closed
validator, and below-the-model write protection (§4.2).

Deliverables: `agents/AUTHORITY.lock.json`; `validate-authority` bin + seeded-violation
tests; `agents/run-loop.sh` pre-iteration halt; CI guardrails-job wiring; orchestration.md
step 0 amendment; PreToolUse hook in repo `.claude/settings.json` (additive);
`agents/GOVERNANCE.md` rationales + `## Amendments (append-only, dated)` section.

```bash
cargo run -p pipeline --bin validate-authority   # exit 0 on the clean tree
cargo test -p pipeline validate_authority        # seeded: unlisted goal / hash mismatch -> exit 1; --check-path deny -> exit 2
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

### Goal 101 — `101-memory-contract-and-index.md`

**Objective:** land the memory file contract, the generated index, and the write-back
generalization (§4.1).

Deliverables: `MemoryFile` schemars contract in `crates/core`; `validate-memory` bin;
`memory-index` generator bin + CI drift gate; `docs/memory/` tree (subsystems + ops files
seeded, `_templates/MEMORY.template.md`); `agents/PROMPT.md` load-order amendment;
`agents/archetypes/_CHASSIS.md` + role files "SAF write-back" → "memory write-back (SAF or
domain memory file)"; `agents/skills/memory-authoring/SKILL.md` (generalizes `saf-authoring`).

```bash
cargo run -p pipeline --bin validate-memory      # all memory files (regimes + subsystems + ops) green
cargo run -p pipeline --bin memory-index && git diff --exit-code docs/memory/INDEX.md
cargo test -p pipeline memory_contract           # seeded: bad frontmatter, oversize (>max_kb), missing section, index drift
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

### Goal 102 — `102-saf-normalization.md`

**Objective:** normalize all 7 regimes to directory-form SAFs, resolving hyphen/underscore
drift (§1 gap 3, §4.1 locations).

Deliverables: 5 flat regimes moved to `docs/regimes/<x>/AUTHORITY.md`; hyphenated
`us-house`/`us-senate` paths reconciled with their underscore regime codes; duplicate legacy
`docs/regimes/us-house.md` folded into `docs/regimes/us_house/AUTHORITY.md`; `E1.lock.json`
SUPERSEDED (version bump + note, per its own policy quoted in §2) since it pins moved paths.
That policy's founder gate is superseded by `docs/decisions/automation-policy.md` (the
canonical autonomy policy per root `CLAUDE.md`), so the goal-102 agent supersedes the lock
without halting on that clause.

```bash
for r in australia_register br canada_ciec eu_fr_de_annual uk_commons_register us_house us_senate; \
  do cargo run -p pipeline --bin validate-survey -- "$r" || exit 1; done
cargo run -p pipeline --bin validate-memory && cargo run -p pipeline --bin memory-index \
  && git diff --exit-code docs/memory/INDEX.md
cargo test -p pipeline role_evals                # E1 lock correctly superseded, pins re-verified
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

### Goal 103 — `103-hygiene-loop.md`

**Objective:** wire the hygiene loop — consolidation standing item, journal schema +
rotation, staleness report, untrusted-content lint (§4.3).

Deliverables: consolidation standing work item + journal entry schema (`agents/LOOP.md` +
`agents/workflows/orchestration.md` step 7); rotation to `agents/journal/archive/YYYY-MM.md`;
`memory-staleness` report bin (report-only) + CI wiring; untrusted-block lint in
`validate-memory`; one consolidation pass on the current JOURNAL as the dry-run proof.

```bash
cargo run -p pipeline --bin memory-staleness     # report-only; exits 0 and prints the report
cargo run -p pipeline --bin validate-memory -- --check-delta origin/main   # this goal's consolidation
                                                 # pass is delta-only: no dated entry deleted/edited
cargo run -p pipeline --bin memory-index && git diff --exit-code docs/memory/INDEX.md
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

## 6. Rollout amendments

Every existing file the goals will amend. After goal 100 lands, every row marked ▲ is a
lock-pinned authority file: its amendment rides an `authority/*` branch and updates
`agents/AUTHORITY.lock.json` in the same commit (§4.2).

| File | Amendment | Goal |
|---|---|---|
| `agents/workflows/orchestration.md` ▲ | Step 0 INTEGRITY prose replaced by running `validate-authority` (quarantine-report duty stays, fed by the bin's output). | 100 |
| `agents/run-loop.sh` | Pre-iteration halt: run `validate-authority` before piping `agents/PROMPT.md`; non-zero exit stops the loop. | 100 |
| `agents/GOVERNANCE.md` ▲ | One-line rationale per rule; new `## Amendments (append-only, dated)` section. | 100 |
| `.claude/settings.json` (repo) | PreToolUse hook added (additive — must not clobber the user's global RTK hook). | 100 |
| `.github/workflows/ci.yml` guardrails job | + `validate-authority` step (100); + `validate-memory` and `memory-index` drift-gate steps (101); + `memory-staleness` warn-only step (103). Needs a Rust toolchain or prebuilt-bin cache in that job — implementer's choice. | 100/101/103 |
| `agents/PROMPT.md` ▲ | Step-1 load order: insert `docs/memory/INDEX.md` between `agents/goals/000-INDEX.md` and "tail of agents/JOURNAL.md". | 101 |
| `agents/archetypes/_CHASSIS.md` ▲ + `agents/roles/*.md` ▲ | Every "SAF write-back" (completed-state, step-6 RECORD wording, required-context footers) → "memory write-back (SAF or domain memory file)". | 101 |
| `agents/skills/saf-authoring/SKILL.md` | Generalized to `agents/skills/memory-authoring/SKILL.md`; the SAF is the regime instance of the general contract; same checklist shape (load first → task → append dated entry → validate → same-PR write-back). | 101 |
| `docs/regimes/_templates/AUTHORITY.template.md` | Gains the §4.1 memory-contract frontmatter fields (added to `RegimeSurvey`, not replacing it) + `[YYYY-MM-DD-nn]` quirks-log entry IDs. | 101 |
| `docs/regimes/{canada_ciec,australia_register,eu_fr_de_annual,uk_commons_register,us_senate}.md` | Moved to directory form `docs/regimes/<x>/AUTHORITY.md`. | 102 |
| `docs/regimes/us-house.md` + `docs/regimes/us-house/` | Legacy flat file + hyphenated dirs reconciled into `docs/regimes/us_house/`; `E1.lock.json` superseded (version bump + note). | 102 |
| `agents/LOOP.md` ▲ | Legacy-mode note: even when `orchestration.md` is absent, journal per the §4.1 entry schema. | 103 |
| `agents/workflows/orchestration.md` step 7 ▲ | Journal line schema becomes `date \| role \| goal \| outcome \| evidence pointers \| blockers`, ≤120 words. | 103 |
| `agents/PROMPT.md` ▲ | step-5 journal-line wording → §4.1 entry schema | 103 |

## 7. Out of scope

Verbatim from the approved plan:

- Business-domain memory files (contract reserves the enum value only).
- Embeddings/vector retrieval; external memory services.
- pg schema changes (e.g. FK for `regime_code`) — noted as adjacent cleanup, not folded in.
- OS-level sandboxing of the loop host.
- Building any of goals 100–103 in this session — the loop owns implementation.

Additionally:

- No changes to Gold/Silver/Bronze semantics — all root-CLAUDE.md data-plane invariants
  (supersede-never-update, Bronze immutability, contract-typed `details`, …) are untouched.
- No new external services — everything here is repo files + Rust bins + existing CI.

## 8. Risks & anti-patterns

Documented failure modes, each with its receipt and the design element that counters it.

| # | Failure mode (receipt) | Countered by |
|---|---|---|
| 1 | **Memory poisoning via untrusted content** — OWASP Agentic T1; AgentPoison: >80% attack success at <0.1% poison rate (arXiv 2407.12784); GitHub MCP exploit (Invariant Labs, https://invariantlabs.ai/blog/mcp-github-vulnerability). | §4.3 anti-poisoning: fenced `untrusted` blocks + `validate-memory` lint; §4.2 hook blocks Read of unlisted goal files (untrusted instructions never enter context). |
| 2 | **Context collapse from monolithic rewrite** — ACE: one rewrite step, 18,282 tokens → 122, accuracy below no-context baseline (arXiv 2510.04618). | §4.3 delta-only consolidation: validator rejects diffs that delete dated entries; monolithic rewrites banned; supersede-never-rewrite entry discipline (§4.1). |
| 3 | **Summary-of-summary decay** — Amp handoffs (https://ampcode.com/news/handoff). | §4.3 consolidation distills from PRIMARY journal entries, each distilled entry citing its source date/commit; rotated archives stay grep-able, so re-derivation always has the originals. |
| 4 | **Context rot — more loaded memory hurts** — focused ~300-token prompt beat a ~113k-token prompt containing the same information (https://www.trychroma.com/research/context-rot). | §4.1 tiny always-loaded index with bodies on demand (P2); `max_kb` size budgets (P1); ~120-word journal-entry cap. |
| 5 | **Stale-memory compounding** — Anthropic Project Vend (https://www.anthropic.com/research/project-vend-1). | §4.1 `last_verified` field (P6) + §4.3 `memory-staleness` CI report (>90 days, warn-not-gate → files re-verification work). |
| 6 | **Agent rewriting its own authority** — Amazon Q 1.84.0 poisoned-instruction incident, ~964k users (https://aws.amazon.com/security/security-bulletins/AWS-2025-015/); Copilot CVE-2025-53773; Cursor CurXecute CVE-2025-54135/54136. | §4.2 PreToolUse hook write-blocks the authority set, the lock, `.claude/settings*`, and hooks dir BELOW the model (exit 2, active under `--dangerously-skip-permissions`); amendment path is branch-named, goal-referenced, lock-updating. |
| 7 | **Prompt-enforced boundaries mistaken for boundaries** — Claude Code enforcement split: "Permission rules are enforced by Claude Code, not by the model" (https://code.claude.com/docs/en/permissions). Applies 1:1 to invariant 9 today (§1 gap 2). | §4.2 entirely: invariant 9 becomes lock + fail-closed bin at three run points + hook. Prose remains for legibility; the mechanism binds. |
| 8 | **Index bloat** — hierarchize past ~1k files; Claude Code caps its own index at 200 lines / 25KB. | §4.1 one line per file in `docs/memory/INDEX.md`; corpus is tens of files; P11 grep does the rest. Revisit shape only if the corpus approaches ~1k files (§3 P11). |

Residual risks, named honestly: (i) a fresh clone without the hook installed is unprotected
until `run-loop.sh`'s pre-flight runs — acceptable because CI (run point 2) still gates
merges; (ii) check (c) reads git history, so history rewrites could evade it — countered by
protected main (no force pushes, per the `agents/run-loop.sh` safety model); (iii) the
untrusted-text lint is heuristic — demarcation discipline is also written into the
`memory-authoring` skill, and consolidation reviews entries a second time.
