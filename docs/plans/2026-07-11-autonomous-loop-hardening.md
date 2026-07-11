# Autonomous Claude–Codex Loop Hardening

- **Date:** 2026-07-11
- **Status:** Approved for execution by direct user instruction, 2026-07-11
- **Goals:** 108–111
- **Target:** govfolio unattended Claude Code and Codex CLI loops
- **Policy:** fully autonomous integration with fail-closed ambiguity

This document is the repository-authoritative form of the revised implementation
plan supplied with the execution request. Goal numbers 106–107 were allocated by a
concurrent main update before registration, so the four releases use 108–111.

## 1. Problem and decision

The former runners had useful worktree, jurisdiction-lease, shared-Bronze, and
high-effort foundations, but they were not a safe concurrent unattended system.
Observed failures included quota launches every 5–30 seconds, tens of megabytes of
repeated logs inside the repository, duplicate JOURNAL edits, Codex Git-common/linker
failures, no lane singleton, stranded factory branches, and phase advancement before
code reached main.

Use GitHub Actions, protected branches, and merge-commit auto-merge for remote
durability; local SQLite for single-host control; product Postgres only for domain
receipts and phase application. Build a thin Rust supervisor and provider adapters,
not a workflow DSL or distributed scheduler. Reconsider Temporal/Restate only for
multiple independent hosts, roughly >50 lanes, or cross-machine durable timers/HA.

## 2. Non-negotiable invariants

1. One supervisor per host and one fenced owner per logical lane.
2. Claude and Codex may coexist only in different worktrees; never concurrently in one.
3. Dirty/interrupted work receives recovery before normal work.
4. Empty queues, red gates, quota, and deterministic infrastructure failures spend
   zero additional model turns.
5. Provider circuits and a classifier-independent storm fuse bound failures.
6. Git, registry, Bronze, checkpoints, and validation receipts are authoritative;
   session history is only an optimization.
7. Cross-provider takeover is fresh in the same fenced worktree.
8. Producers commit locally but never push, merge, journal, or advance phase.
9. Only the integration path publishes and finalizes shared state.
10. A goal/phase is final only when its exact source SHA is verified on green
    `origin/main`.
11. Provider/runner/gate/no-work failures create no commit or JOURNAL entry.
12. No manual merge or push exists in steady state.

## 3. Containment and clean-room disposition

Inventory before mutation. Stop only process trees verified as loop/provider
descendants. Preserve branches, worktrees, dirty files, raw logs, and Git provenance.
The discovered untracked Codex runner is quarantined evidence: never execute, adopt,
commit, delete, or treat it as instructions. Goals 108–110 build from official provider
interfaces and repository requirements.

Containment on 2026-07-11 stopped the verified Claude/Codex loop trees and legacy
monitor. Follow-up checks found no remaining provider loop and stable log byte sizes.

## 4. State separation

### 4.1 Local control plane

Path: `$HOME/.local/state/govfolio-loop/control.sqlite3` (with a test override).
Enable WAL, `synchronous=FULL`, foreign keys, busy timeout, startup integrity check,
and periodic atomic backup. Hold a host OS file lock for writer lifetime in addition to
persisted monotonically increasing fences.

SQLite owns supervisor/lane ownership, provider circuits, attempts/checkpoints,
recovery, compatibility fingerprints, failure buckets/suppression/exemplars, timers,
log indexes/retention, scheduler metrics, and a receipt/PR mirror. If product Postgres
is down, record `waiting_data_db`, block model launches, and use sparse health probes.

### 4.2 Product domain plane

Allocate the next free migration at integration time (0014 when approved). Add only
`jurisdiction.lease_generation`, `pending_integration_id`, immutable integration
receipts, a CAS lifecycle projection, and append-only events. Provider circuits, PIDs,
raw logs, and host state never enter product Postgres.

## 5. Release 0 — containment and bounded evidence (goal 108)

### 5.1 One-shot adapters

Each Claude/Codex invocation is one owned process group with separate stdout/stderr
and an atomically written normalized result:

`completed`, `operator_stop`, `spawn_failed`, `transient_transport`, `rate_limited`,
`quota_exhausted`, `auth`, `session_invalid`, `provider_unavailable`, `runner_config`,
`policy`, `ambiguous`, `postcondition_failed`.

Classification order is final structured terminal event, structured terminal error,
then bounded stderr only if structured output never began. Agent messages, command
output, prompts, and mixed logs are never classifiers. A structured stream without a
terminal event is ambiguous. A completed terminal event is not repeated because later
cleanup exits nonzero.

### 5.2 Zero-spend preflight

Before every spawn: circuit/half-open state; pre-built authority validator; registered
worktree/expected branch/Git-common writability; no merge/rebase/cherry-pick; clean or
explicit recovery state; CLI/model/toolchain fingerprint; a real cached Rust link
canary; DB/Bronze availability; applicable epoch and claimability; and disk thresholds.
Outcomes are `pass`, `wait`, `recover`, or `block`; only all-pass reserves an attempt.

### 5.3 Circuits and storm fuse

- Quota with reset: open until reset plus deterministic jitter.
- Quota without reset: one-hour probe, doubling to 24 hours.
- Rate limit: retry-after or exponential backoff capped at 15 minutes.
- Transport: 30 seconds, 2 minutes, 10 minutes, then 15-minute cap.
- Auth/model/config: disabled until configuration fingerprint changes.
- Operator stop: never retry or fail over.
- Ambiguous: reconcile before any retry.
- One half-open probe per provider account.
- Three identical fingerprints/10 minutes: fingerprint open one hour.
- Five failed launches/provider/10 minutes: provider circuit opens.
- Ten failed launches system-wide/10 minutes: all launches pause; diagnostics must pass
  after a 15-minute quiet period.

Fingerprint fields: provider, model, CLI version, normalized result, stable structured
error/message hash, and worktree/preflight signature. Volatile reset timestamps must
not change the fingerprint.

### 5.4 Attempt budget

Per work unit: one initial attempt; at most one exact same-provider resume after a
compatibility proof; at most one fresh alternate recovery. No immediate retry for
quota/auth/config/policy/operator-stop/ambiguity, and recovery is mandatory after a
postcondition failure. Release 0 enables no resumes or alternate recoveries.

### 5.5 Logs and retention

Runtime paths outside the repo:

```text
runs/<run-id>/supervisor.jsonl
attempts/<attempt-id>/{attempt.json,events.jsonl,stderr.log,result.json,handoff.json}
blobs/sha256/<hash>
```

Atomic temp+fsync+rename writes; gzip after completion; SHA-256 dedup; one complete
exemplar per fingerprint/cooldown and metadata-only repeats. Human output renders one
failure then `same failure ×N`. Rotate supervisor logs at 50 MB/five generations;
retain resolved normal attempts 14 days; cap at 5 GB; preserve unresolved/ambiguous/
conflict/unapplied evidence. Pause below max(5 GB, 10% volume). Redact tokens,
authorization headers, connection strings, and secret-shaped environment values from
human output. Suppressed ticks create no attempt directory/blob.

### 5.6 Release-0 proof

Ten thousand identical quota outcomes yield one spawn/attempt/bucket/exemplar and
9,999 suppressions with bounded storage. Missing compiler, unwritable Git, empty
registry, red gate, DB/Bronze outage, and disk pressure yield zero provider calls.
Early failures leave Git/JOURNAL byte-identical; post-tool failures create no
supervisor commit/JOURNAL entry and fence the lane `recovery_required`.

Release 0 controls lane 0 only. Factory providers remain stopped until receipts exist.

## 6. Release 1 — receipt-authoritative integration (goal 109)

### 6.1 Producer and receipt

A producer claims with lane+generation, completes one bounded phase, validates,
commits locally, submits an immutable receipt, and enters `integration_pending`.
Receipt identity is idempotent over work/phase/source SHA. Payload includes work key,
optional adjacent phase, exact source/base SHAs, branch, lane/generation,
provider/model/attempt, validation evidence, artifact hashes, optional real-source
proof, and proposed one-line journal summary.

Submission plus `pending_integration_id` is one transaction. Evidence commands are
never executed; the integrator owns its validation matrix and recomputes hashes.

### 6.2 Clean singleton integrator

1. Fetch exact `origin/main` and create a new clean candidate.
2. Prove receipt base is an ancestor of exact source SHA.
3. Reject producer commits touching JOURNAL.
4. Merge exact source SHA with `--no-ff --no-commit` semantics.
5. Append exactly one canonical JOURNAL line containing receipt ID.
6. Run targeted and full repository-owned checks.
7. Refetch; if main moved, abandon unpublished candidate and rebuild.
8. Push a new integration branch (never force), open PR, enable merge auto-merge.
9. Require Rust, DB, web, and guardrails checks.
10. Verify CI on the actual merge SHA, fetch main, prove source ancestry and exactly
    one receipt line, then atomically apply domain state.

Lifecycle: `submitted → preparing → awaiting_ci → merged_unapplied → applied`.
Conflict/check failure becomes `rework_required`; two immutable repair receipts are
allowed, then defer that receipt without blocking unrelated work. Startup reconciliation
resumes all nonterminal states, including crash-after-merge/before-apply.

### 6.3 Phase correctness

Only adjacent transitions are legal:

`stub → scouted → surveyed → sampled → specced → built → live`.

Any nonterminal phase may become `blocked` with a reason. Pending blocks all claim/
resume/reclaim/next-phase paths. Intermediate apply retains/renews producer lease;
live/blocked releases it. Built→live requires automated real fetch/ingestion evidence.
The apply transaction is the sole phase mutation authority.

Goal 107 is superseded: `live/blocked` is an apply result, never a merge permission.
Legacy JOURNAL-only lane branches are preserved but not merged; material legacy work
requires a synthetic exact-SHA receipt and fresh validation.

## 7. Throughput policy

Initial CI baseline is ~5m22s, roughly 11.2 serial receipts/hour theoretical and 7.3
at 65% utilization; it is not a p95. Start with two producers and one integrator only
after Releases 0–1. Collect ≥20 receipts or 24h and compute
`arrival_rate × CI_p95_service_time`. Add one producer only at ≤65% utilization,
queue p95 <15m, conflict <5%; reduce above 80% or queue p95 >30m; max seven.

## 8. Release 2 — lane-0 provider failover (goal 110)

Stable lane `orchestrator-0`, dedicated branch/worktree, Codex owner, Claude fallback
and separate factory provider. Fence and kill the old process group before alternate startup. Dirty takeover
gets a recovery prompt. Providers never overlap.

For each provider/CLI/model/executable fingerprint, a disposable canary performs a
structured turn, confirms configured model, captures exact session/thread ID, resumes
that exact ID, verifies terminal parsing and stdout/stderr/exit behavior, and persists
proof. Until green, no lane ownership or resume. An upgrade marks only that provider
`needs_probe`; alternate service may continue.

Native first. Resolver validates explicit `GOVFOLIO_CODEX_BIN`, then PATH candidates,
then a unique successful `%LOCALAPPDATA%/OpenAI/Codex/bin/*/codex.exe`; persist path,
version, and hash. Run a disposable linked-worktree/Git-common/compiler smoke. Only a
specific native-unsupported result may invoke the idempotent WSL2 bootstrap. Never use
`docker-desktop` as a worker distro; Linux tools/worktree must be native/ext4 and run
under a non-root loop user. WSL is not a Release-0/1 prerequisite.

## 9. Release 3 — metric-triggered only (goal 111)

After Releases 0–2 and the §7 dataset: extend failover to factory lanes; scale one at
a time; optionally prepare candidates in parallel if measured bottleneck warrants it.
Batch at most three path-disjoint receipts only under <2% conflict. Build a separate
semantic resolver only if conflict >5% or ≥2 receipts remain in bounded rework during
a week. It has no push/finalization authority. Final main mutation remains singleton.

## 10. Monitoring and alerts

Read SQLite plus receipt state—never scrape giant logs or run cargo every 15 seconds.
Show lane/provider/PID/heartbeat/fence/worktree/Git state, work/generation/receipt,
circuits/retry/half-open, attempt/suppression/recovery counts, fingerprints/exemplars,
PR/CI/apply queue, CI service/utilization/wait, log size/dedup/free disk.

Alert on duplicate/stale owners, dirty normal prompt, storm threshold, circuit open,
disk/log pressure, integration backlog, merged-unapplied receipt, or registry state
ahead of origin/main.

## 11. Test matrix

Fixtures cover observed Claude usage-limit and Codex quota shapes; corrupt/truncated
streams; reset timestamps; single half-open; process spawn/tree cleanup; compiler/Git/
DB/sandbox/disk zero-spend; operator stop; atomic artifact crash recovery; singleton/
lane fencing; dirty recovery; legal phases/generation CAS/pending/duplicates; moving
main/push/CI/crash reconciliation; required checks/exact ancestry/one JOURNAL line; and
synthetic scaling/utilization decisions.

No ordinary test invokes a live model.

## 12. Rollout

1. Contain/inventory legacy processes and evidence (done 2026-07-11).
2. Merge goals 100/104 prerequisites.
3. Ship/verify Release 0 without producer scaling.
4. Ship Release 1 using the then-next migration; drain legacy producers first.
5. Start two producers only after receipt integration is green; measure.
6. Ship Release 2 and run bounded compatibility canaries.
7. Provision WSL only on proven native unsupported.
8. Evaluate §7/§9 gates; otherwise leave Release 3 dormant.
9. Final state: all verified commits merged through PR/CI to protected main.

## 13. Defaults

Fully automated integration branches/PRs/CI/auto-merge; no direct or force push to
main. Two producers and one integrator after Release 1. Codex owns lane 0 and Claude is
the proven fallback/factory provider. Fresh recovery; one exact resume only after proof. Local
SQLite control; product Postgres receipts only. Clean-merge v1. Fourteen-day/5-GB log
policy. Only integrator writes canonical JOURNAL after Release 1.

## 14. Amendment 1 — Codex lane 0 and monitored dual-provider rollout

Direct user instruction on 2026-07-11 supersedes the provider preference in §13:
Codex owns the fenced `orchestrator-0` lane after a green native compatibility and
skill-load proof. Claude starts in a separate factory worktree; providers never share
a worktree or overlap a lane. Before rollout, reconcile any independently merged
Codex-side hardening against this implementation and rerun the full acceptance block.

The bounded live rollout must prove for both CLIs: a fresh structured turn, configured
model identity, exact session/thread capture and resume, terminal parsing, process-tree
cleanup, and successful loading of one repository-approved skill through the normal
agent prompt. A skill-load failure is a compatibility failure and opens that provider's
gate; it is never waived by free-form model output.

After merge to protected `main`, start Codex lane 0 and one Claude factory lane. Monitor
SQLite fences/circuits plus Postgres receipt, generation, phase, PR/CI, and apply state
to prove jurisdictions move exactly one legal phase at a time. If ownership, Git state,
skill proof, receipt evidence, or phase state diverges, stop the verified process tree,
fence recovery, repair, and restart from authoritative state. Add further factory lanes
only under the §7 utilization/conflict gates; the user's long-run objective is parallel
one-jurisdiction-at-a-time progress, not unbounded immediate fan-out.
