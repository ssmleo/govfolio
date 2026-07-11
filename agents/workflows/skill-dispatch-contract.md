# Skill dispatch contract

This contract applies to every root and nested specialist dispatch. The authoritative
role, skill, pack, trigger, bridge, and native Codex-agent mapping is
`agents/skill-routing.json`; prose never substitutes for resolver output.

## Deterministic dispatch

1. Select one governed role and one trusted goal, plan, or workflow section. Goal
   sections may be selected only from goals listed in `agents/goals/000-INDEX.md`.
2. Supply every applicable trigger by its explicit `trigger:*` ID. Never infer or
   synthesize a trigger ID.
3. Run `node scripts/agents/resolve-codex-dispatch.mjs --repo-root . --role <role>`
   with `--trigger <trigger-id>` for each trigger, `--section-file <path>` and
   `--section-heading <exact-heading>` together when dispatching a section, and
   `--source-context <SAF-path>` when source-scoped.
4. Prepend the resolver's unmodified `GOVFOLIO_DISPATCH_V1` envelope to the child
   prompt. The model must not assemble, edit, reorder, or repair an envelope.
5. Under Codex, dispatch the exact generated custom agent at
   `.codex/agents/<role>.toml`. A missing Codex shim is a hard failure; never infer the
   governed role in-session. Under Claude Code, retain the native
   `.claude/agents/<role>` shim. Imported templates remain unchanged; prepend the
   envelope to their task prompt.
6. Before accepting task output, require this exact receipt, with values copied from
   the envelope and skill IDs kept in envelope order:

   `SKILLS_LOADED role=<role> contract=<contract_sha256> skills=<comma-separated envelope skill IDs>`

7. A missing envelope or receipt is a hard failure: do no task work. The same applies to
   a malformed envelope, missing role shim, source/hash mismatch, validator failure, or
   mismatched receipt. Return `BLOCKED(skill-contract)` and reject any output produced
   without the valid receipt.
8. Repeat steps 1-7 for every nested dispatch. A parent receipt never authorizes a
   child, and a parent must not hand-edit or reuse its own envelope for the child.

## Pre-receipt boundary

Before emitting `SKILLS_LOADED`, a child may only read `AGENTS.md`, tracked `CLAUDE.md`
completely, its exact governed role file, `agents/skill-routing.json`, and the bridge and canonical skill files named
by the envelope, and may run the deterministic contract validator. Planning, source
research, task-file reads outside the selected section, shell exploration, and mutation
are task work and remain forbidden until the receipt is emitted.
