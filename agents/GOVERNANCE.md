# Agent governance

Rules carry stated rationales (constitution-with-reasons, design §4.2 P20): reasons
generalize better than bare rules when a fresh session meets an unforeseen case.

## Agent-creation protocol (AUTOMATED — allocator, no founder gate)
Founder decision 2026-07-04: skill selection is automated via the deterministic allocator
in docs/decisions/automation-policy.md. A new agent self-allocates skills from its output
contract using the artifact->skill map, applies the 6-slot ceiling and pack rule, and
commits. The auditor spot-checks ceiling/pack compliance (mechanical, non-blocking).
The former founder gate is LIFTED and recorded here as automated.
Rationale: a rule approved identically six times is a proven pattern — coding it removes
the human bottleneck without changing the decision.

## Chassis rule
Every role file follows the six-slot chassis (agents/archetypes/_CHASSIS.md) and names its
archetype. New archetypes and archetype changes are founder-gated like skill changes.
Rationale: uniform slots let the orchestrator parse any role at dispatch; an agent's
structure is a function of its failure mode, so structure changes are prompt changes.

## Skill rules
- One procedure per skill (agents/skills/<name>/SKILL.md): purpose, when to load, core
  checklist, anti-patterns. Deepened via write-back, never forked per role.
- Roles load ONLY the skills listed in their Skills: section, plus /CLAUDE.md and the
  relevant SAF. Skill sprawl is a bug: if a role needs >6 skills, split the role.
Rationale: one canonical procedure accumulates learnings in one place; per-role forks
drift apart and rot, and unbounded loading dilutes the context that actually binds.

## Orchestrator constraints
The orchestrator (agents/roles/orchestrator.md) selects, dispatches, verifies, records.
It never writes production code, never self-certifies, never approves proposals, never
unblocks human lanes. Its full workflow: agents/workflows/orchestration.md.
Rationale: the verifier must not be the doer — self-certification is how unverified work
reaches main; separation keeps every claim externally checkable.

## A1 — Standing vs situational skills; packs (approved 2026-07-04)
Standing skills load every iteration; ceiling 6 slots. Situational skills are
founder-gated allocations loaded ONLY when their role-file trigger fires; they never
co-load by default and do not count against the ceiling. A pack (<=3 same-source,
same-domain skills) occupies one slot and is gated as a unit.
Rationale: context is the scarce resource — a hard ceiling forces curation, and
trigger-gated loading pays context cost only when the situation earns it.

## A2 — Doer split along the language boundary (approved 2026-07-04)
The doer archetype instantiates per language boundary: rust-builder (data plane) and
web-builder (presentation edge). extraction-strategy is held exclusively by spec-writer;
builders read the decided strategy from the SAF.
Rationale: the language boundary IS the invariant boundary (data semantics vs pixels);
splitting there keeps each doer's skill set small and its failure modes disjoint.

## A3 — Imports (approved 2026-07-04)
Bespoke skills outrank imported ones on any conflict. Imports enter ONLY by vendor-copy
at a pinned sha into agents/skills/imported/<lib>@<shortsha>/ with license and per-file
verdicts in docs/decisions/skill-imports.md. Live plugin/marketplace installs are
forbidden: an auto-update is an unreviewed prompt change. Some imports merge into
bespoke skills or chassis text after audit instead of occupying slots
(verification-before-completion -> chassis DoD; test-driven-development -> rust-tdd).
Rationale: anything loaded into an agent session is executable authority — imports get
supply-chain treatment (pin, review, vendor) exactly like a dependency lockfile.

## Effort policy
agents/EFFORT.md and .claude/agents shim frontmatter are prompt-changing artifacts:
edits are founder-gated like role edits. Shims must remain thin (no behavior beyond
loading the governed role file).
Rationale: effort levels and shim text silently reshape every downstream decision; a
prompt change without review is an unaudited behavior change.

## Authority lock (goal 100, design §4.2)
The authority set (this file, PROMPT.md, LOOP.md, workflows/orchestration.md, roles/*.md,
archetypes/*.md, EFFORT.md, EPOCHS.md, goals/000-INDEX.md, root /CLAUDE.md) is
sha256-pinned in agents/AUTHORITY.lock.json and enforced fail-closed by the
pre-built `validate-authority --ci` gate at four run points: run-loop.sh
pre-flight, CI guardrails, orchestration step 0, and the PreToolUse hook
(.claude/hooks/authority-guard.sh — blocks below the model, active even under
--dangerously-skip-permissions). Amendments to pinned files ride an authority/* branch,
update the lock in the same commit (`--write-lock --note "<what changed and why>"`), and
reference an INDEX-listed goal in the commit message. Goal files are not content-pinned;
the goals<->000-INDEX bijection covers them (invariant 9).
Rationale: prompt-only rules are not boundaries — the model can be talked out of prose,
not out of an exit code (design §8 rows 6-7); tamper-evidence makes every authority edit
auditable and every unauthorized one loud.

## Amendments (append-only, dated)
Corrections supersede; entries are never edited or deleted.
- 2026-07-11 (goal 100): added per-rule rationales, the Authority-lock rule, and this
  section; installed agents/AUTHORITY.lock.json + validate-authority (run-loop pre-flight,
  CI guardrails step, orchestration step 0 gate, PreToolUse hook). Reason: invariant 9
  was prompt-enforced only — made mechanical per design
  docs/plans/2026-07-10-memory-authority-substrate-design.md §4.2.
