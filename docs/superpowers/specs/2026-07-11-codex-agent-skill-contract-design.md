# Codex Agent Skill Contract Design

Date: 2026-07-11
Status: Approved for specification by the user on 2026-07-11

## Problem

Govfolio's orchestration policy assigns each role a bounded set of ACTIVE and
SITUATIONAL skills under `agents/skills`, but Codex discovers repository skills
under `.agents/skills`. The current native Codex role shims and runner are also
untracked, so factory worktrees created from Git do not receive them. As a
result, a Codex session can be told to use a governed skill without that skill
being present in its native catalog, and there is no mechanical check before a
model call is made.

The failure is wider than discovery:

- role files use human-readable skill names rather than an exact resolver;
- Codex prompts still prefer Claude-specific agent shims;
- the default Codex nesting depth permits a root child but not a specialist's
  implementer or reviewer child;
- no validator checks role parity, skill paths, generated shims, or a clean
  worktree;
- the current fallback silently adopts a role in-session instead of failing
  when the native role or skill contract is unavailable.

## Goal

Before any governed Codex subagent performs task work, guarantee that its role
baseline and every task- or step-specific skill resolve to exact, checked-in
instructions in that worktree. A missing, ambiguous, stale, or undeclared skill
must stop dispatch before the model call whenever possible, and otherwise make
the child return a structured skill-contract failure before task actions.

## Non-goals

- Reorganizing the canonical `agents/skills` catalog.
- Editing vendored imported skill bodies or changing their pinned provenance.
- Removing unrelated repository, user, plugin, admin, or system skills.
- Committing machine-specific MCP configuration from `.codex/config.toml`.
- Making authenticated Codex calls part of CI.
- Changing the role allocator's six-slot governance policy.

## Chosen Approach

Keep `agents/skills` canonical and generate a tracked Codex projection.

Moving the canonical catalog into `.agents/skills` would provide native
discovery but would require a broad path migration and would disturb vendored
layouts. Git symlinks would preserve one source of truth but are unreliable in
Windows checkouts and worktrees. Generated bridge skills are portable, small,
and mechanically comparable with their source manifest.

## Architecture

### 1. Machine-readable routing manifest

Add `agents/skill-routing.json` as the machine-readable enforcement companion
to the human role files and governance policy. It contains:

- a versioned contract and the required Codex nesting/thread limits;
- every governed skill ID;
- a unique Codex-facing name prefixed with `govfolio-`;
- the repository-relative canonical skill-directory path;
- a deterministic tree hash covering the complete canonical skill directory;
- a concise Codex discovery description;
- every role's ACTIVE skill IDs;
- every SITUATIONAL skill ID and its trigger text.

The manifest is authoritative for machine dispatch. The existing role files
remain the reviewed human policy surface. Validation requires the manifest and
the role `Skills/Tools` sections to describe the same assignments, including
packs and situational triggers. A change to either side without the other fails.

For a task, the required set is:

1. all ACTIVE skills assigned to the selected role;
2. SITUATIONAL skills whose declared trigger fires; and
3. extra skill IDs explicitly declared by the selected goal, plan task, or
   workflow step.

Unrecognized extra skill IDs are errors. A dispatcher may not silently replace
or omit a required skill.

Goals, plan tasks, and workflow steps declare additions with the exact Markdown
field `**Required skills:** <comma-separated skill IDs>`. Absence of the field
means no additions beyond the role baseline and triggered situational skills.

### 2. Generated Codex projection

Generate one bridge directory per governed skill at
`.agents/skills/govfolio-<id>/`. Each bridge contains:

- a Codex-valid `SKILL.md` with `name` and `description` frontmatter;
- the canonical repository-relative path and expected tree hash;
- a hard instruction to read the canonical `SKILL.md` completely before task
  actions and resolve all relative resources from the canonical directory;
- `agents/openai.yaml` with implicit invocation disabled, so governed skills
  are loaded by the dispatch contract rather than accidental description
  matching.

The bridges contain no duplicated procedure text. Generated output is committed
so fresh clones and lane worktrees receive it. A renderer supports `--write` and
`--check`; CI and runtime use `--check`.

Generate the eleven `.codex/agents/*.toml` role shims from the same manifest and
the matching `agents/roles/*.md` files. Each shim retains the governed role name
and `xhigh` effort, loads `AGENTS.md` and the role file, and requires a dispatch
envelope. The checked-in shims are the native Codex role layer; Codex dispatch
must not fall back to an inferred in-session role when a named shim is missing.

### 3. Dispatch envelope

Every child dispatch, including generic implementer, fixer, explorer, and
reviewer dispatches created by nested workflows, carries a compact envelope:

```text
GOVFOLIO_ROLE: <role>
GOVFOLIO_REQUIRED_SKILLS:
- <skill-id> | $<codex-name> | <canonical-path> | <tree-sha256>
GOVFOLIO_SOURCE_CONTEXT: <SAF path or none>
```

The orchestration workflow, factory-lane workflow, root prompts, and common
agent chassis all require this envelope. Imported prompt templates remain
unchanged; controllers prepend the project envelope when using them.

On receipt, a child may perform only contract-loading reads until it has:

1. loaded `AGENTS.md`, its role file, and `agents/skill-routing.json`;
2. confirmed its ACTIVE assignment is present in the envelope;
3. confirmed each envelope path and hash against the manifest; and
4. read every listed bridge and canonical `SKILL.md`.

Then it records a compact `SKILLS_LOADED` receipt in its report and proceeds.
If any check fails, it returns `BLOCKED(skill-contract): <reason>` and performs
no task mutation. Skills available from user or plugin scope never satisfy a
governed requirement unless the manifest explicitly pins the checked-in
project path.

### 4. Tracked runtime assets and worktree behavior

Adopt and track these currently local Codex orchestration assets after bringing
them under the contract:

- root `AGENTS.md`;
- `.codex/agents/*.toml`;
- `agents/run-loop-codex.sh`.

Do not commit the machine-specific `.codex/config.toml`. The tracked runner
passes portable overrides for `agents.max_depth = 2` and
`agents.max_threads = 6`, which allows root -> specialist -> worker/reviewer
while bounding fan-out. The runner validates the contract inside the actual
current worktree before every `codex exec`, not only in the primary checkout.

The existing `AGENTS.md.txt`, lane logs, local settings, build artifacts, and
machine MCP configuration remain untouched and untracked.

### 5. Validator and fail-closed behavior

Add a cross-platform Node validator using only standard-library modules. It
checks:

- manifest schema, unique IDs, and unique `govfolio-*` names;
- all canonical paths resolve inside the repository;
- deterministic skill-directory hashes;
- generated bridge and agent-shim drift;
- exact role-set parity across `agents/roles`, `.claude/agents`, and
  `.codex/agents`;
- ACTIVE, SITUATIONAL, pack, and trigger parity between role files and the
  manifest;
- the presence of `AGENTS.md` and every prompt/workflow dispatch requirement;
- the runner's explicit depth/thread settings and preflight call;
- absence of unresolved or duplicate governed skill names.

Errors identify the role, skill, expected path, actual path or hash, and the
repair command. Validation exits nonzero before the runner starts Codex. No
warning-only path exists for a governed requirement.

## Data Flow

```text
role policy + canonical skills
          |
          v
agents/skill-routing.json
          |
          +--> renderer --> .agents/skills/govfolio-* bridges
          |             --> .codex/agents/*.toml
          |
          +--> validator --> CI and runner preflight
          |
          v
orchestrator selects role + triggered step skills
          |
          v
dispatch envelope --> child contract reads --> SKILLS_LOADED --> task work
                                           \--> BLOCKED(skill-contract)
```

## Testing

Use Node's built-in test runner with temporary fixture repositories. Required
tests cover:

- a complete manifest and projection passes;
- a missing canonical skill fails;
- a path escaping the repository fails;
- a changed skill tree without a manifest hash update fails;
- a stale or hand-edited bridge fails;
- a missing or extra Codex role shim fails;
- a role/manifest ACTIVE or SITUATIONAL mismatch fails;
- duplicate Codex-facing names fail;
- missing dispatch-envelope language fails;
- missing runner preflight, `max_depth = 2`, or the thread bound fails;
- a clean Git worktree has every tracked runtime asset and passes validation.

CI adds a small agent-governance job that runs the renderer in check mode, the
validator tests, the repository validator, and `bash -n` on the Codex runner.
An optional authenticated smoke command may spawn each custom role with a
no-mutation prompt and verify its `SKILLS_LOADED` receipt, but it is not a CI
gate because it requires credentials, network access, and model spend.

## Rollout

1. Add the manifest, renderer, validator, and tests.
2. Generate and track bridge skills and native Codex role shims.
3. Update governance, chassis, workflows, and prompts to use envelopes.
4. Adopt and harden the Codex runner and root `AGENTS.md`.
5. Add CI checks and run all focused validation.
6. Verify the contract from a clean temporary worktree.
7. Run the optional authenticated role smoke if the local Codex executable is
   available and authorized.

The rollout is complete only when a clean worktree passes and intentionally
removing one required bridge makes the runner stop before `codex exec`.

## Risks and Mitigations

- **Manifest drift:** generated artifacts and role parity are CI failures.
- **Hash maintenance:** the renderer updates hashes and projections in one
  explicit command; runtime never rewrites them automatically.
- **Skill-context truncation:** envelopes include exact project paths and hashes,
  so loading does not depend on the skill being retained in the abbreviated
  initial catalog.
- **Windows portability:** generated files replace Git symlinks and use
  repository-relative paths.
- **Recursive fan-out:** nesting stops at depth two and open threads are capped.
- **Vendored prompt changes:** imported skill bodies stay byte-for-byte pinned;
  project workflows add the envelope externally.
- **Dirty primary checkout:** runtime validation executes inside each lane's
  actual worktree and CI repeats the check from a clean checkout.

## Acceptance Criteria

- Every governed role and referenced skill resolves exactly once from a clean
  worktree.
- Every native Codex agent loads its role ACTIVE skills and any triggered
  step-specific skills through an explicit envelope.
- A missing, stale, ambiguous, or undeclared required skill prevents task work.
- Specialist agents can dispatch one nested worker/reviewer layer but no deeper.
- No imported skill body or machine-specific Codex/MCP configuration is
  committed or rewritten.
- Focused tests, repository validation, runner syntax validation, and the clean
  worktree check all pass.
