---
policy_id: govfolio-build-performance
schema_version: 1
status: advisory
---

# Build performance policy

This is the canonical cross-provider policy for compile, link, test-build, cache,
benchmark, and CI build-time work. It supplements repository authority and safety rules;
it never authorizes destructive cleanup, shared-target mutation, Bronze movement,
production access, or external spend.

## Status and enforcement

The current status is **advisory**. Until executable supervisor admission lands, an agent
must inspect supervisor status and live Cargo/Rust processes before an exclusive command.
A busy or ambiguous host makes recorded measurement `INCONCLUSIVE`; the agent halts that
experiment rather than claiming exclusivity. Process inspection is advisory and
race-prone, not a lock.

After executable admission lands, every governed autonomous or interactive session uses
the supervisor client. Missing supervisor, invalid identity/fence, or stale policy hash
fails before Cargo starts. The status advances through `shadow` and becomes `enforced`
only after the rollout gates below pass.

## Command classes

| Disposition | Commands |
|---|---|
| Passthrough | `--version`, `version`, `metadata`, `tree`, and `fmt` |
| `cargo-focused` | `check`, `build`, `test`, `clippy`, or `run` scoped to exactly one package and a private target |
| `cargo-exclusive` | workspace/all-package or unscoped commands, `test --no-run`, benchmarks, cold builds, epoch gates, and complete matrices |
| Denied | `cargo clean`, deletion/reset helpers, Bronze paths, and shared mutable targets |

Unknown compilation-capable commands fail closed to `cargo-exclusive`. An explicit class
may upgrade inference but never downgrade it. User `-j`/`--jobs` cannot exceed the
supervisor's effective budget.

## Host capacity and fairness

Defaults for the current 16-logical-CPU Windows host:

- focused capacity 2, six jobs per holder;
- exclusive capacity 1, fourteen jobs;
- queue deadline 30 minutes;
- experiment deadline 60 minutes;
- supervisor heartbeat 30 seconds and progress report at least every 10 minutes;
- no-progress deadline 15 minutes;
- focused memory gate 4 GiB per resulting holder; exclusive memory gate 8 GiB; and
- free target-volume gate greater of 20 GiB or 10%.

`cargo-exclusive` conflicts with every build holder. Requests are FIFO within class. Once
an exclusive request waits, later focused requests cannot pass it: current focused holders
finish, then the oldest exclusive request runs. Queue waiting is durable supervisor state,
not an agent retry loop.

Every worktree keeps a private target directory. Cargo registry data may be shared; mutable
targets may not. Bronze remains in its dedicated durable root outside all build targets.
Local sccache may reuse compatible compiler results, but it does not share Cargo build
artifacts across different absolute target paths.

## Policy refresh and stale worktrees

Every admission records the active canonical-policy hash. A normal policy update lets an
in-flight admitted command finish under its acquired hash, but queued and later commands
must acknowledge the active hash.

A mismatch blocks only new admission. It never resets, deletes, cleans, rebases, abandons,
or edits the requesting worktree. Clean stale worktrees may resume in place under a
restricted `historical_contract` only when governed policy files equal their trusted
merge-base versions or active versions and changed paths exclude authority, policy,
goal-queue, deployment, production, and integration-control surfaces. Historical lanes
continue only their already-owned item and may not create a new claim or external-spend
action. Dirty stale worktrees remain preserved and recovery-fenced until reviewed changes
are committed.

## Performance experiment workflow

Before the first recorded command, declare the lever/workload, baseline and candidate
commit/tree hashes, acceptance threshold, regression veto, private target strategy,
expected duration/evidence format, toolchain, target triple, lockfile, and profile.

Then:

1. Obtain the applicable admission or use the honest advisory preflight.
2. Run one exploratory baseline/candidate pair in deterministic AB or BA order.
3. Stop `NO-GO` when the pair has no useful signal.
4. Stop `INCONCLUSIVE` for interference, invalid evidence, timeout, or exhausted retry.
5. Only a promising pair expands to three alternating baseline and candidate samples.
6. Release admission on completion, cancellation, timeout, or failure.

Required workload vocabulary is `cold build`, `warm no-op`, `representative edit rebuild`,
and `cargo test --workspace --no-run` or an explicitly scoped equivalent. Baseline and
candidate use separate private targets. Cold targets must be proven absent before samples.

## Time, retry, and churn budgets

- Exploratory phase deadline: 30 minutes.
- Whole experiment deadline: 60 minutes unless extended by an immutable checkpoint before
  expiry.
- A command progresses when it completes work, emits compiler output, or records measurable
  CPU/I/O activity; a quiet active linker is not automatically stale.
- One retry is allowed only for whitelisted transport, registry-fetch, or Windows sharing
  failures.
- Compiler errors, test failures, Clippy findings, policy failures, and cancellation have
  zero retries.
- Automatic complete matrix rerun budget: zero.
- A complete rerun checkpoint states why evidence is insufficient, commands repeated, and
  estimated wall cost.
- Review requirements are fixed before execution. An auditor may reject evidence but may
  not silently expand the contract.

## Evidence contract

Audit-ready evidence starts with the first recorded command and includes:

- baseline and candidate commits/tree hashes plus dirty-state checks;
- redacted exact command artifact and command hash;
- Cargo, rustc, linker, toolchain, target triple, lockfile, and profile hashes/config;
- target path and cold-freshness proof;
- start, end, Cargo, and host wall times;
- exit status and stderr;
- rebuilt workspace packages;
- concurrent Cargo/Rust observations throughout the sample;
- raw sample values and medians; and
- hashes of retained evidence artifacts.

Reports separate measured facts, calculations, estimates, and hypotheses. Every lever is
`GO`, `NO-GO`, or `INCONCLUSIVE`. Rejected and inconclusive fingerprints remain recorded
so another agent does not repeat them without new evidence.

One task contains at most one cold baseline/candidate pair or three paired warm/edit
samples plus required resets. Additional workloads or edit classes are separate tasks.
The auditor checkpoint occurs after exploratory evidence and before confidence sampling.

## Rollout gates

1. Land canonical policy and discovery shims as `advisory`.
2. Land supervisor admission and run `shadow` for at least 24 hours or 20 managed commands,
   including Codex, Claude, interactive, and stale-worktree cases.
3. Promote to `enforced` only with zero lost/reset worktrees, zero stale-fence mutations,
   zero unexplained Cargo children, queue p95 below 15 minutes, green Windows recovery and
   cross-provider tests, and one accepted exploratory experiment without rerun.
4. Promotion changes this status/change log and the authority hash only.

## Change log

- 2026-07-12 — schema 1 created as advisory policy under goal 114.
