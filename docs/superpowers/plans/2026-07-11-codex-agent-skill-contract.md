# Codex Agent Skill Contract Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every governed Codex dispatch resolve, validate, and load the exact role and step skills available in the current Git worktree before task work begins.

**Architecture:** Keep `agents/skills` canonical. A strict manifest and lock describe skills, packs, triggers, dependencies, and role assignments; a Node standard-library tool validates the contract, generates Codex bridge skills and custom-agent shims, and deterministically resolves each dispatch envelope. The tracked runner invokes the validator inside each actual lane worktree before every Codex session and fails before model spend on any mismatch.

**Tech Stack:** Node.js 24 standard library, JSON/JSON Schema, generated Markdown/YAML/TOML, Bash, Git, GitHub Actions.

## Global Constraints

- `agents/skills` remains the only canonical procedure catalog; generated bridges contain no copied procedure body.
- Vendored files under `agents/skills/imported` remain byte-for-byte unchanged.
- `.codex/config.toml`, Codex logs, local settings, build artifacts, and `AGENTS.md.txt` remain untracked and untouched.
- Governed IDs are namespaced as `skill:*`, `pack:*`, and `trigger:*`; a skill ID such as `skill:rust-tdd` maps exactly to Codex name `govfolio-rust-tdd`.
- ACTIVE slot accounting preserves the six-slot ceiling; packs cost one slot and expand to leaf skills only during envelope resolution.
- Trigger activation is explicit by trigger ID. The resolver never infers triggers from prose.
- A planned or unavailable skill that becomes required returns `BLOCKED(skill-contract)`; it is never silently omitted.
- Tree locks are never refreshed by render or check. Only the explicit `refresh-lock` command changes hashes.
- Tree hashes use Git clean-filtered blob hashes, so Windows CRLF checkout policy cannot create Linux CI drift.
- `agents.max_depth` is exactly `2`; `agents.max_threads` is exactly `6`.
- All child dispatches, including nested implementers, fixers, explorers, and reviewers, use the deterministic resolver and a `GOVFOLIO_DISPATCH_V1` envelope.
- The validator never reads an unindexed goal body; goal requirements are inspected only after the orchestrator selects a `000-INDEX.md`-trusted goal.
- No authenticated Codex call is required in CI.
- The completed branch must pass focused contract checks and the repository's full acceptance block before it is merged locally to `main`.

---

### Task 1: Build the strict contract and resolver core

**Files:**

- Create: `agents/skill-routing.schema.json`
- Create: `scripts/agents/codex-contract-lib.mjs`
- Create: `scripts/agents/codex-contract.test.mjs`

**Interfaces:**

- Consumes: canonical role Markdown, canonical skill directories, Git clean filters, and exact `**Required skills:**` Markdown fields.
- Produces: pure manifest validation, deterministic tree hashing, pack/dependency expansion, task-field parsing, envelope construction, generated-file rendering, and sorted diagnostics for all later CLIs.

The library exports these exact functions:

```js
export function findRepoRoot(startDirectory) {}
export function parseManifest(text, sourcePath) {}
export function validateManifest(manifest, repoRoot) {}
export function hashSkillTree(repoRoot, canonicalDirectory) {}
export function parseRequiredSkills(markdown, sourcePath, heading) {}
export function expandRequirements(manifest, requirementIds) {}
export function resolveDispatch(manifest, options) {}
export function renderBridgeSkill(manifest, skill) {}
export function renderOpenAiMetadata(skill) {}
export function renderCodexAgent(manifest, role) {}
export function renderRoleSkillBlock(manifest, role) {}
export function formatEnvelope(resolution) {}
export function formatDiagnostics(diagnostics) {}
```

Every diagnostic has this stable shape and is sorted by `code`, `path`, `role`, then `skill`:

```js
{
  code: "UNKNOWN_SKILL_ID",
  message: "role rust-builder references unknown skill:missing",
  path: "agents/skill-routing.json",
  role: "rust-builder",
  skill: "skill:missing",
  expected: null,
  actual: "skill:missing",
  repair: "edit agents/skill-routing.json and rerun the contract checks"
}
```

- [ ] **Step 1: Write failing core tests**

Use `node:test`, `node:assert/strict`, `mkdtemp`, and `t.after()` cleanup. Build each fixture repository programmatically. The first tests must cover:

```js
test("valid manifest preserves slots and expands packs", () => {});
test("duplicate skill and Codex names fail", () => {});
test("unknown and planned requirements fail closed", () => {});
test("path traversal and symlinked canonical skills fail", () => {});
test("Git-filtered tree hashes are stable across CRLF checkout bytes", () => {});
test("dependency closure is sorted and cycle-safe", () => {});
test("Required skills ignores fenced examples and selects the exact heading", () => {});
test("unknown triggers are rejected instead of inferred", () => {});
test("dispatch formatting is deterministic JSON", () => {});
```

- [ ] **Step 2: Run the tests and confirm RED**

Run:

```powershell
node --test scripts/agents/codex-contract.test.mjs
```

Expected: FAIL because `scripts/agents/codex-contract-lib.mjs` does not exist.

- [ ] **Step 3: Implement the core library and strict schema**

Use arrays in the schema wherever duplicates must remain observable. The top-level schema is exact and rejects unknown keys:

```json
{
  "schema_version": 1,
  "hash_algorithm": "sha256-git-blobs-v1",
  "codex": {
    "bridge_root": ".agents/skills",
    "agent_root": ".codex/agents",
    "max_depth": 2,
    "max_threads": 6,
    "effort": "xhigh"
  },
  "dispatch": {
    "envelope_version": 1,
    "required_skills_field": "Required skills",
    "failure_prefix": "BLOCKED(skill-contract)"
  },
  "skills": [],
  "packs": [],
  "triggers": [],
  "roles": []
}
```

`hashSkillTree()` must enumerate regular files, reject symlinks and case-colliding paths, normalize relative paths to `/`, sort by UTF-8 byte order, and obtain each content ID with:

```js
execFileSync("git", [
  "hash-object",
  "--filters",
  `--path=${repositoryRelativePath}`,
  absolutePath
], { cwd: repoRoot, encoding: "utf8" }).trim();
```

Hash length-framed relative paths and the resulting 40-character Git blob IDs with SHA-256. This includes untracked resources without depending on checkout line endings.

`resolveDispatch()` accepts this exact options object:

```js
{
  role: "rust-builder",
  triggers: ["trigger:completion-review"],
  sectionFile: "docs/superpowers/plans/example.md",
  sectionHeading: "Task 4: Rust implementation",
  sourceContext: "docs/regimes/us_house/AUTHORITY.md"
}
```

It expands ACTIVE slots, explicit triggers, the nearest exact `**Required skills:**` field, packs, and transitive dependencies. It returns a hard diagnostic for planned/unavailable skills and emits no partial envelope.

- [ ] **Step 4: Run the core tests and confirm GREEN**

Run:

```powershell
node --test scripts/agents/codex-contract.test.mjs
```

Expected: all core tests PASS with no warnings.

- [ ] **Step 5: Commit the core**

```powershell
git add agents/skill-routing.schema.json scripts/agents/codex-contract-lib.mjs scripts/agents/codex-contract.test.mjs
git commit -m "feat(agents): add Codex skill contract core"
```

---

### Task 2: Add the manifest, lock refresh, generated projection, and CLIs

**Files:**

- Create: `agents/skill-routing.json`
- Create: `scripts/agents/refresh-codex-skill-lock.mjs`
- Create: `scripts/agents/render-codex-contract.mjs`
- Create: `scripts/agents/validate-codex-contract.mjs`
- Create: `scripts/agents/resolve-codex-dispatch.mjs`
- Modify: `scripts/agents/codex-contract.test.mjs`
- Generate: `.agents/skills/govfolio-*/SKILL.md`
- Generate: `.agents/skills/govfolio-*/agents/openai.yaml`
- Generate: `.codex/agents/*.toml`
- Modify generated blocks: `agents/roles/*.md`

**Interfaces:**

- Consumes: Task 1 pure functions and all current governed role allocations.
- Produces: a reviewed lock manifest, 32 available bridge skills plus explicit planned entries, eleven custom Codex agents, normalized role Slot-5 blocks, a no-write validator, and a deterministic envelope CLI.

The CLIs have exact behavior:

```text
refresh-codex-skill-lock.mjs   recomputes only source.tree_sha256 and file_count
render-codex-contract.mjs      --write or --check; never changes hashes
validate-codex-contract.mjs    validates source, lock, projection, roles, prompts, and runner
resolve-codex-dispatch.mjs     emits one envelope or exits 1 with BLOCKED(skill-contract)
```

- [ ] **Step 1: Add failing projection and CLI tests**

Add exact cases:

```js
test("refresh-lock changes only lock fields", () => {});
test("render check mode performs no writes", () => {});
test("render write mode repairs marked generated drift", () => {});
test("render never deletes an unmarked custom agent or skill", () => {});
test("stale marked bridge and shim are rejected", () => {});
test("role generated blocks match manifest slots and trigger IDs", () => {});
test("resolver expands role, trigger, step, pack, and dependencies", () => {});
test("resolver blocks a triggered planned skill", () => {});
test("impeccable bridge preserves docs-only script restrictions", () => {});
```

- [ ] **Step 2: Run the expanded test file and confirm RED**

Run:

```powershell
node --test scripts/agents/codex-contract.test.mjs
```

Expected: FAIL because the CLIs and real manifest do not exist.

- [ ] **Step 3: Create the complete manifest**

Represent every current role skill, all out-of-tree workflow dependencies, packs, planned members, and deterministic triggers. The available source directories are exactly:

```text
agents/skills/{adversarial-verification,conformance-diffing,drift-detection,evidence-archiving,extraction-strategy,fixture-capture,human-gate-etiquette,plan-decomposition,polite-fetching,regime-research,rust-tdd,saf-authoring,schema-contracts}
agents/skills/imported/frontend-design@9d2f1ae18723
agents/skills/imported/impeccable@582f23eae3c9/skills/impeccable
agents/skills/imported/rust-async-patterns@5cc2549a50fc
agents/skills/imported/rust-best-practices@7df6a608dd71
agents/skills/imported/typescript-advanced-types@5cc2549a50fc
agents/skills/imported/superpowers@d884ae04edeb/{brainstorming,dispatching-parallel-agents,executing-plans,finishing-a-development-branch,receiving-code-review,requesting-code-review,subagent-driven-development,systematic-debugging,test-driven-development,using-git-worktrees,using-superpowers,verification-before-completion,writing-plans,writing-skills}
```

Represent `skill:typescript-react-reviewer` as planned/unavailable because its import failed closed for no license, and `skill:typescript-expert` as the planned member of partial `pack:ts-craft`. Define `pack:rust-craft`, `pack:ts-craft`, and `pack:impeccable` with `slot_cost: 1` and no more than three members.

Use the eleven role assignments already approved in `agents/roles/*.md`. Normalize situational triggers to these exact IDs:

```text
trigger:web-artifact-under-audit
trigger:parallel-work
trigger:novel-feature-without-spec
trigger:skill-authoring
trigger:verification-failed-twice
trigger:completion-review
trigger:review-feedback
```

Declare transitive workflow dependencies, including `subagent-driven-development` -> `test-driven-development`, `requesting-code-review`, `finishing-a-development-branch`, and `using-git-worktrees`; and `executing-plans` -> `finishing-a-development-branch`, `using-git-worktrees`, and `writing-plans`.

- [ ] **Step 4: Implement the four CLIs and generated marker rules**

Generated files begin with a stable marker. `render --write` may replace or remove only files/blocks carrying that marker. Bridge skills contain valid `name`/`description` frontmatter, canonical path/hash checks, and no procedure duplication. Every bridge metadata file contains:

```yaml
interface:
  display_name: "Govfolio: plan-decomposition"
  short_description: "Governed bridge; explicit dispatch only."
policy:
  allow_implicit_invocation: false
```

Generated TOMLs contain `name`, role-specific `description`, `model_reasoning_effort = "xhigh"`, and developer instructions that permit only contract-loading reads before validation, require `GOVFOLIO_DISPATCH_V1`, load `AGENTS.md`, the exact role file, manifest, bridge, and canonical instructions, and return `BLOCKED(skill-contract)` without mutation on mismatch.

The envelope is delimited JSON, not prose:

```text
--- GOVFOLIO_DISPATCH_V1 ---
{"contract_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","role":"rust-builder","source_context":"docs/regimes/us_house/AUTHORITY.md","triggers":["trigger:completion-review"],"skills":[{"id":"skill:rust-tdd","codex_name":"govfolio-rust-tdd","canonical_path":"agents/skills/rust-tdd","tree_sha256":"5ab8aa4551d06cdd885b511b9bf962745c3b18ed793c1132d0e0cae74ba3d79b"}]}
--- END GOVFOLIO_DISPATCH_V1 ---
```

The child receipt is exactly:

```text
SKILLS_LOADED role=rust-builder contract=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa skills=skill:conformance-diffing,skill:rust-tdd
```

- [ ] **Step 5: Refresh the reviewed lock, render, and validate**

Run in this order:

```powershell
node scripts/agents/refresh-codex-skill-lock.mjs
node scripts/agents/render-codex-contract.mjs --write
node scripts/agents/render-codex-contract.mjs --check
node scripts/agents/validate-codex-contract.mjs --repo-root .
node --test scripts/agents/codex-contract.test.mjs
```

Expected: every command exits 0; `--check` writes nothing; eleven Codex TOMLs and every available governed bridge exist.

- [ ] **Step 6: Commit the manifest and projection**

```powershell
git add agents/skill-routing.json agents/roles .agents/skills/govfolio-* .codex/agents scripts/agents
git commit -m "feat(agents): project governed skills into Codex"
```

---

### Task 3: Enforce deterministic skill envelopes throughout orchestration

**Files:**

- Create: `agents/workflows/skill-dispatch-contract.md`
- Create: `AGENTS.md`
- Modify: `agents/archetypes/_CHASSIS.md`
- Modify: `agents/GOVERNANCE.md`
- Modify: `agents/PROMPT.md`
- Modify: `agents/PROMPT-FACTORY-LANE.md`
- Modify: `agents/workflows/orchestration.md`
- Modify: `agents/workflows/factory-lane.md`
- Modify: `agents/workflows/source-exploration.md`
- Modify: `docs/plans/2026-07-04-govfolio-implementation.md`
- Modify: `docs/plans/2026-07-07-consensus-extraction.md`
- Modify: `docs/plans/2026-07-07-consensus-hardening.md`
- Modify: `docs/plans/2026-07-09-filing-document-viewer-implementation.md`
- Modify: `agents/goals/021-llm-extraction.md`
- Modify: `scripts/agents/codex-contract.test.mjs`

**Interfaces:**

- Consumes: the Task 2 resolver and generated native role names.
- Produces: one provider-aware dispatch path that uses the resolver for root and nested agents, never edits vendored templates, and never falls back from a missing Codex shim to in-session role inference.

- [ ] **Step 1: Add failing integration-policy tests**

Add checks for the exact contract markers in every integration file:

```js
test("root and factory prompts require resolver output and native Codex roles", () => {});
test("orchestration rejects missing envelope and receipt", () => {});
test("chassis propagates the contract to nested dispatch", () => {});
test("governance declares manifest authority and generated projections", () => {});
test("known plan and goal sub-skills use Required skills fields", () => {});
test("tracked AGENTS requires CLAUDE and the dispatch contract", () => {});
```

- [ ] **Step 2: Run focused tests and confirm RED**

Run:

```powershell
node --test scripts/agents/codex-contract.test.mjs
```

Expected: the new policy tests FAIL against the current Claude-only dispatch wording.

- [ ] **Step 3: Write the shared dispatch contract and compact root AGENTS file**

`agents/workflows/skill-dispatch-contract.md` defines:

1. select the governed role and trusted task/workflow section;
2. supply explicit trigger IDs;
3. run `resolve-codex-dispatch.mjs`;
4. prepend its unmodified envelope to the child prompt;
5. under Codex, dispatch the exact generated custom agent;
6. require the exact `SKILLS_LOADED` receipt before accepting task output;
7. return `BLOCKED(skill-contract)` and do no task work for any mismatch;
8. repeat the same process for every nested dispatch.

Root `AGENTS.md` stays compact: it requires a complete read of tracked `CLAUDE.md`, then applies the skill contract to custom and built-in Codex agents. It lists the only allowed pre-receipt actions: read `AGENTS.md`, the role file, manifest, bridge/canonical skills, and run the deterministic validator.

- [ ] **Step 4: Update chassis, governance, prompts, and workflows**

Preserve the chassis's six slots and add a pre-slot skill-contract precondition. Update orchestration step 4 to resolve the selected goal/plan/workflow section, explicit triggers, role, and source SAF before native dispatch. Codex missing-shim or missing-receipt errors are hard failures; Claude retains its `.claude/agents/{selected-role}` path. Imported templates receive a prepended envelope and remain unchanged.

Factory phase fan-out resolves a separate envelope for every producer, auditor, implementer, fixer, or reviewer. An invalid receipt is a failed verification under the existing two-failure lease semantics.

Add exact `**Required skills:**` fields next to the existing `REQUIRED SUB-SKILL` declarations in the four implementation plans and trusted goal 021. Do not scan or edit unindexed goal bodies.

- [ ] **Step 5: Run integration checks and confirm GREEN**

```powershell
node --test scripts/agents/codex-contract.test.mjs
node scripts/agents/render-codex-contract.mjs --check
node scripts/agents/validate-codex-contract.mjs --repo-root .
```

Expected: all tests and validation PASS, and `git diff -- agents/skills/imported` is empty.

- [ ] **Step 6: Commit orchestration enforcement**

```powershell
git add AGENTS.md agents/archetypes/_CHASSIS.md agents/GOVERNANCE.md agents/PROMPT.md agents/PROMPT-FACTORY-LANE.md agents/workflows docs/plans agents/goals/021-llm-extraction.md scripts/agents/codex-contract.test.mjs
git commit -m "feat(agents): enforce skill contracts on dispatch"
```

---

### Task 4: Adopt and harden the Codex runner and CI gate

**Files:**

- Create from the reviewed local candidate: `agents/run-loop-codex.sh`
- Create: `scripts/agents/check-codex-contract-clean-worktree.mjs`
- Modify: `scripts/agents/codex-contract.test.mjs`
- Modify: `package.json`
- Modify: `.github/workflows/ci.yml`

**Interfaces:**

- Consumes: tracked contract assets and Task 2 validator.
- Produces: one runner wrapper through which every raw `codex exec` passes, a no-auth preflight mode, a clean-worktree verifier, and a dependency-free CI job.

- [ ] **Step 1: Add failing runner and clean-worktree tests**

Add cases:

```js
test("runner has no raw Codex call outside the contract wrapper", () => {});
test("runner validates immediately before every Codex exec", () => {});
test("runner passes depth two and thread bound six", () => {});
test("runner preflight-only mode never invokes a stub Codex binary", () => {});
test("tracked machine Codex config is forbidden", () => {});
test("clean detached worktree validates without mutation", () => {});
```

- [ ] **Step 2: Run tests and confirm RED**

```powershell
node --test scripts/agents/codex-contract.test.mjs
```

Expected: FAIL because the tracked runner and clean-worktree verifier do not exist.

- [ ] **Step 3: Adopt the runner through a single contract wrapper**

Bring the reviewed `C:\projects\govfolio.io\agents\run-loop-codex.sh` candidate into this branch without bringing its logs or local configuration. Add `node` and tracked-contract existence checks. Add these exact Codex overrides:

```bash
--config 'agents.max_depth=2'
--config 'agents.max_threads=6'
```

All model calls route through one `codex_with_contract` function which runs renderer `--check` and repository validation inside the passed worktree immediately before invoking Codex. Add `GOVFOLIO_CODEX_PREFLIGHT_ONLY=1`; in this mode the runner validates the primary checkout and every configured lane worktree, prints success, and exits without calling Codex.

- [ ] **Step 4: Implement clean-worktree verification**

Use `execFile`, `mkdtemp`, and a `finally` block. Require the source worktree clean, create a detached temporary worktree, run render check and validation there, require empty `git status --porcelain --untracked-files=all`, verify tracked presence of `AGENTS.md`, runner, manifest, bridges, and all shims, assert `.codex/config.toml` is untracked, then remove only the verified temporary worktree.

- [ ] **Step 5: Add package scripts and the CI job**

Add these scripts:

```json
{
  "agent-contract:test": "node --test scripts/agents/codex-contract.test.mjs",
  "agent-contract:lock": "node scripts/agents/refresh-codex-skill-lock.mjs",
  "agent-contract:render": "node scripts/agents/render-codex-contract.mjs --write",
  "agent-contract:check": "node scripts/agents/render-codex-contract.mjs --check && node scripts/agents/validate-codex-contract.mjs --repo-root .",
  "agent-contract:clean": "node scripts/agents/check-codex-contract-clean-worktree.mjs"
}
```

Add a dependency-free `agent-governance` GitHub Actions job using Node 24. It runs the Node tests, render check, validator, `bash -n agents/run-loop-codex.sh`, and clean-worktree verifier. It performs no package install, network action, or authenticated Codex call.

- [ ] **Step 6: Run runner and CI-equivalent checks**

```powershell
node --test scripts/agents/codex-contract.test.mjs
node scripts/agents/render-codex-contract.mjs --check
node scripts/agents/validate-codex-contract.mjs --repo-root .
bash -n agents/run-loop-codex.sh
node scripts/agents/check-codex-contract-clean-worktree.mjs
```

Expected: every command exits 0; the clean worktree remains clean; no `codex` process is started.

- [ ] **Step 7: Commit runner and CI enforcement**

```powershell
git add agents/run-loop-codex.sh scripts/agents package.json .github/workflows/ci.yml
git commit -m "ci(agents): gate Codex skill availability"
```

---

### Task 5: Prove failure behavior, complete repository verification, and merge

**Files:**

- Modify: `agents/JOURNAL.md`
- Modify if review finds a scoped defect: only files introduced or changed by Tasks 1-4

**Interfaces:**

- Consumes: the complete branch and current local `main`.
- Produces: adversarial failure evidence, full verification evidence, one journal write-back, reviewed commits, and a local merge commit on `main` containing no unrelated goal-100 history.

- [ ] **Step 1: Prove the fail-closed path**

In a disposable detached worktree created by the clean-worktree verifier, temporarily remove one generated required bridge and run preflight-only mode with a stub `codex` executable that records invocations.

Expected: validator exits 1 with `GENERATED_FILE_MISSING`; the stub invocation count remains zero. Restore by discarding the disposable worktree, not by changing this branch.

- [ ] **Step 2: Run the complete focused contract suite**

```powershell
pnpm agent-contract:test
node scripts/agents/render-codex-contract.mjs --check
node scripts/agents/validate-codex-contract.mjs --repo-root .
bash -n agents/run-loop-codex.sh
node scripts/agents/check-codex-contract-clean-worktree.mjs
git diff --exit-code -- agents/skills/imported
```

Expected: all commands exit 0 and imported skill bodies have no diff.

- [ ] **Step 3: Run the repository acceptance block**

```powershell
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
docker compose up -d
cargo test --workspace -- --ignored
pnpm --filter web lint
pnpm --filter web typecheck
pnpm --filter web test
pnpm e2e
pnpm --filter @govfolio/contracts generate
git diff --exit-code packages/contracts/
```

Expected: every command exits 0 with pristine output. Authenticated Codex smoke remains optional because the design explicitly excludes authenticated model calls from CI.

- [ ] **Step 4: Dispatch task and whole-branch reviews, then fix and re-verify findings**

Use governed `auditor` reviewers with the deterministic skill envelope. Treat every Critical or Important finding as blocking. Re-run the focused covering test after each fix and re-run the whole-branch contract suite after the final review wave.

- [ ] **Step 5: Append the durable write-back and commit final corrections**

Append one `agents/JOURNAL.md` line with date `2026-07-11`, item `codex-agent-skill-contract`, outcome, focused/full verification evidence, and blockers `none`. Stage only scoped files and commit any final review fixes plus the journal entry:

```powershell
git add agents/JOURNAL.md
git commit -m "docs(agents): record Codex skill contract rollout"
```

- [ ] **Step 6: Integrate the latest main and re-run merge-sensitive checks**

Merge current local `main` into `codex/codex-agent-skill-contract`. Resolve only scoped conflicts, preserving concurrent JOURNAL entries. Re-run the focused contract suite, `cargo fmt --check`, and `pnpm --filter web typecheck`.

- [ ] **Step 7: Merge the completed branch locally to main**

Create an isolated clean worktree for `main`; do not switch the user's active goal worktree. Merge with:

```powershell
git merge --no-ff codex/codex-agent-skill-contract -m "merge: enforce Codex agent skill contracts"
```

Run the focused contract check once from merged `main`, verify `git status --short` is empty, and leave remote pushing out of scope because the user requested a local commit and merge, not a push.
