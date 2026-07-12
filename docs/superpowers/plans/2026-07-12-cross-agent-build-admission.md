# Cross-Agent Build Admission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Route governed Codex, Claude, autonomous, and interactive Cargo work through
the existing fenced host supervisor with fair capacity, reproducible evidence, and
evidence-preserving stale-worktree handling.

**Architecture:** The authority-pinned Markdown policy is hashed into the supervisor's
durable control store. Clients request supervised Cargo execution through authenticated
host-local JSON Lines IPC; the supervisor owns queuing, resource budgets, process groups,
evidence, cancellation, and recovery. Stale worktrees remain in place and may operate
under a restricted historical contract without mutating their policy files.

**Tech stack:** Rust 2024, Tokio, SQLx/SQLite, serde JSON Lines, Windows named pipes,
Unix domain sockets, PowerShell, Git authority/contract validators.

## Global constraints

- Start from a fresh `origin/main` worktree; preserve all existing worktrees and branches.
- Goal 114 is the only executable authority for this work.
- The supervisor remains the only durable control-store writer and integration authority.
- `CLAUDE.md` remains unchanged.
- No reset, deletion, clean, rebase, Bronze movement, production access, or external spend.
- New Rust behavior follows test-first red/green/refactor and adds no production
  `unwrap()`/`expect()`.
- Policy rollout is `advisory` → `shadow` → `enforced`; never claim enforcement early.

---

## Release 0 — Authority registration and design correction

- [x] Register `agents/goals/114-build-resource-admission.md` in the authoritative queue.
- [x] Regenerate `agents/AUTHORITY.lock.json` with an explicit goal-114 supersession note.
- [x] Bring design commit `a103d55` forward without merging unrelated performance commits.
- [x] Amend the design for authenticated IPC, queue fairness, fixed job budgets, policy
  hashing, stale-worktree preservation, all-session fail-closed behavior, paired cold
  measurements, baseline/candidate tree hashes, and `scripts/dev/cargo-agent.ps1`.
- [x] Add this implementation plan to the PR-0 tree.
- [x] Verify authority, authority tests, Markdown hygiene, and clean diff.

Acceptance:

```text
cargo run -p pipeline --bin validate-authority
cargo test -p pipeline validate_authority
git diff --check
```

## Release 1 — Canonical policy and discovery

- [x] Create `docs/decisions/build-performance-policy.md` with exact front matter:

  ```yaml
  ---
  policy_id: govfolio-build-performance
  schema_version: 1
  status: advisory
  ---
  ```

- [x] Define managed command classification, budgets, deadlines, evidence outcomes,
  retry rules, queue behavior, stale-policy refresh, historical-contract restrictions,
  and the advisory process-inspection fallback in that one file.
- [x] Add pointer-only discovery text to root `AGENTS.md` and
  `.claude/rules/build-performance.md`; do not modify `.claude/agents/*.md`.
- [x] Extend `validate-authority`'s pinned set and tests for the canonical policy, then
  regenerate the authority lock through an `authority/*` branch.
- [x] Extend Codex contract tests to prove both providers point to the same canonical
  regular tracked file and that no policy body is duplicated.
- [x] Change `scripts/dev/cargo-agent.ps1` to invoke the supervisor Cargo client while
  preserving Cargo's exit code; retain the documented limitation that sccache cannot
  share compilation across distinct absolute target paths.

Acceptance:

```text
cargo test -p pipeline validate_authority
node --test scripts/agents/codex-contract.test.mjs
cargo run -p pipeline --bin validate-authority
powershell -NoProfile -File scripts/dev/cargo-agent.test.ps1
```

## Release 2 — Admission store, scheduler, IPC, and Cargo runner

### Public types and commands

```rust
struct BuildPolicySnapshot {
    schema_version: u32,
    policy_sha256: String,
    status: BuildPolicyStatus,
    source_commit: String,
    loaded_at: DateTime<Utc>,
}

enum BuildPolicyStatus {
    Advisory,
    Shadow,
    Enforced,
}
```

```text
govfolio-loop serve-builds
govfolio-loop build-policy
govfolio-loop cargo [--class focused|exclusive] [--category <category>] \
  [--policy-sha <sha256>] -- <cargo arguments>
govfolio-loop status
govfolio-loop recover-build <request-id>
```

If neither `run` nor `serve-builds` is active, managed commands exit 75 without starting
Cargo. Explicit class can upgrade inferred classification but cannot downgrade it.

### Command classification

- Passthrough: `--version`, `version`, `metadata`, `tree`, and `fmt`.
- Focused: `check`, `build`, `test`, `clippy`, or `run` with exactly one explicit package
  and a private target.
- Exclusive: workspace/all-package commands, commands without package scope,
  `test --no-run`, benchmarks, cold builds, complete matrices, and `epoch-gate`.
- Denied: `cargo clean`, deletion/reset helpers, Bronze paths, and shared mutable targets.
- Unknown compilation-capable commands fail closed to exclusive.
- User `-j`/`--jobs` cannot exceed the supervisor's effective job budget.

### Durable state

- [ ] Add `0002_build_admission.sql` with `build_policy_snapshot`, `build_request`,
  `build_request_event`, `build_evidence`, queue sequence, and state indexes.
- [ ] Persist supervisor/lane fences, owner identity, policy hash, class, category,
  worktree, target, command hash, effective jobs, timestamps, PID identity, deadline,
  outcome, and evidence hash. Exact commands live only in protected runtime evidence.
- [ ] Implement fenced states:

  ```text
  queued -> running
         -> completed | failed | cancelled | timed_out | inconclusive
         -> recovery_required
  ```

### Protocol and scheduling

- [ ] Add authenticated JSON Lines IPC over
  `\\.\pipe\govfolio-loop-<state-root-hash>` on Windows and
  `<state-root>/control.sock` on Unix.
- [ ] Generate a per-user control token in the runtime directory and reject wrong protocol
  version, token, owner identity, policy hash, or fence before request creation.
- [ ] Stream queue heartbeat, stdout, stderr, admission identity, and terminal result.
- [ ] Implement FIFO within class and an exclusive barrier: once exclusive waits, no newer
  focused request starts; current focused holders finish first.
- [ ] Cancel queued/running requests on client disconnect; stale fences cannot start,
  heartbeat, release, or recover.

Defaults for the current 16-logical-CPU host:

```text
focused capacity: 2
focused jobs: 6 each
exclusive jobs: 14
queue deadline: 30 minutes
experiment deadline: 60 minutes
heartbeat: 30 seconds
progress report: 10 minutes
no-progress deadline: 15 minutes
```

- [ ] Validate `GOVFOLIO_CARGO_*` overrides and reject configurations that leave fewer
  than two CPUs for the host.
- [ ] Require 4 GiB available memory per resulting focused holder, 8 GiB for exclusive,
  and free target-volume space greater of 20 GiB or 10%.
- [ ] Treat compiler output or CPU/I/O deltas as measurable progress so a quiet active
  linker is not falsely timed out.

### Execution and recovery

- [ ] Extend the existing process-group runner for raw Cargo stdout/stderr and real exit
  codes; the supervisor, not the agent, launches admitted Cargo.
- [ ] Retry once only for whitelisted transport/registry-fetch/Windows sharing failures.
  Compiler, test, Clippy, policy, and cancellation failures get zero retries.
- [ ] On restart, cancel old queued requests, mark ambiguous running requests
  `recovery_required`, and block conflicts until the recorded PID/start identity is dead.
- [ ] `recover-build` requires the current supervisor fence and proof of dead process;
  recovery never changes the worktree.
- [ ] Extend status with active policy, queue position, holder, class, target, age, and
  deadline, without exposing commands or secrets.

Acceptance:

```text
cargo test -p loop-supervisor build_policy
cargo test -p loop-supervisor build_protocol
cargo test -p loop-supervisor build_scheduler
cargo test -p loop-supervisor build_process
cargo test -p loop-supervisor build_recovery
cargo clippy -p loop-supervisor --all-targets -- -D warnings
```

## Release 3 — Cross-provider enforcement and historical worktrees

- [ ] Prepend a supervisor-owned Cargo shim to autonomous provider `PATH`; inject the
  active policy hash and control endpoint.
- [ ] Require interactive clients to use the same supervisor command. Missing server,
  policy hash, or valid identity fails before Cargo starts.
- [ ] On policy mismatch, return `policy_refresh_required`, display the active hash and
  bounded canonical policy, create no request, and change no worktree bytes.
- [ ] Allow retry in the same session with the new policy hash. Running admitted commands
  finish under the acquired hash; old queued/new requests must refresh.
- [ ] Implement `historical_contract` mode only when every governed policy file is clean
  and equals its trusted merge-base blob or active blob, and application changes exclude
  authority, policy, queue, deployment, production, and integration-control paths.
- [ ] Restrict historical lanes to their already-owned work item, with no new claim,
  production action, external spend, or authority mutation.
- [ ] Preserve dirty stale worktrees as `recovery_required`; never reset them. Existing
  `recover-lane` resumes after reviewed changes are committed on the same branch.
- [ ] Add historical receipt fields: merge-base SHA, active policy hash, source SHA, and
  changed-path manifest.
- [ ] Reject historical receipts that touch governed paths; always integrate and validate
  application code from fresh current main.
- [ ] Pause admissions for unknown govfolio Cargo/Rust processes. If one contaminates a
  supervised measurement, cancel only the supervised command and mark `INCONCLUSIVE`.

Acceptance scenarios:

- Current Codex, current Claude, and interactive clients share one queue.
- Policy refresh leaves stale worktree HEAD, index, tracked, and untracked hashes identical.
- Trusted historical policy blobs pass; locally modified policy blobs fail closed.
- Dirty implementation remains present and fenced.
- Historical application changes integrate without replacing current policy.
- Foreign Cargo invalidates measurement without being killed.
- Windows cancellation leaves no supervisor-owned Cargo/rustc child.

## Release 4 — Evidence pilot, experiment harness, and rollout

```text
govfolio-loop experiment-start <manifest.json>
govfolio-loop experiment-review <review.json>
```

- [ ] Define schema version, experiment ID/lever, baseline and candidate commit/tree
  hashes, workload, acceptance threshold, regression veto, target strategy, duration,
  evidence format, policy hash, and toolchain/linker/profile/lockfile hashes.
- [ ] Exercise the schema once manually before implementing the harness; amend it once if
  evidence is insufficient, then freeze that version.
- [ ] Use separate private baseline/candidate targets; cold targets must be absent and
  Bronze paths are forbidden.
- [ ] Exploratory phase runs one deterministic AB or BA baseline/candidate pair.
- [ ] No useful signal is `NO-GO`; invalid environment, interference, timeout, or exhausted
  retry is `INCONCLUSIVE`.
- [ ] Confidence phase runs three alternating baseline and candidate samples and compares
  medians. Additional workloads are separate tasks.
- [ ] Require an auditor checkpoint after exploratory evidence. It may accept or reject the
  fixed contract but cannot expand it.
- [ ] Require a new immutable reason/command/cost checkpoint before a complete matrix rerun.
- [ ] Capture redacted exact command artifact/hash, tree/toolchain/profile/lock hashes,
  target freshness, timing, exits/stderr, rebuilt packages, raw samples, medians,
  interference, and retained artifact hashes.

Rollout:

1. Land policy/discovery as `advisory`.
2. Run `shadow` for at least 24 hours or 20 managed commands, including Codex, Claude,
   interactive, and stale-worktree refresh cases.
3. Promote to `enforced` only with zero lost/reset worktrees, zero stale-fence mutations,
   zero unexplained Cargo children, queue p95 below 15 minutes, Windows/cross-provider
   tests green, and one accepted exploratory experiment without rerun.
4. Promotion changes only canonical policy status/change log and its authority hash.

## Final verification

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo nextest run --workspace --run-ignored ignored-only --profile ci
cargo run -p pipeline --bin validate-authority
node --test scripts/agents/codex-contract.test.mjs
pnpm --filter web lint
pnpm --filter web typecheck
pnpm --filter web test
```

Run a Windows two-worktree proof for focused concurrency, exclusive fairness, byte-stable
policy refresh, stale-fence rejection, and recovery that never deletes, resets, cleans,
rebases, or abandons a worktree.
