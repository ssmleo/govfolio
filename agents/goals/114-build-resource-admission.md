# Goal 114 — cross-agent build resource admission

Status: `[ ]` registered 2026-07-12 from the founder-approved cross-agent build
admission plan. Execute only through the gated releases below.

## Objective

Make build-performance policy discoverable across Codex and Claude, then route governed
Cargo capacity, evidence, retries, and time budgets through the existing fenced host
supervisor without discarding stale worktrees or creating a second coordinator.

## Invariants

- Preserve every existing worktree, branch, dirty file, commit, and evidence artifact.
- The supervisor remains the sole durable control-store writer and integration authority.
- Stale policy blocks or restricts execution; it never authorizes reset, deletion, cleanup,
  rebase, Bronze movement, production access, or external spend.
- All new Rust behavior is test-first and keeps the workspace `unwrap`/`expect` lint green.
- `CLAUDE.md` remains unchanged. Authority amendments supersede the lock explicitly.

## Gated releases

1. **PR 0 — authority and design:** register this goal, bring the reviewed design onto
   current main, correct it for IPC admission, fairness/job budgets, policy hashing,
   stale-worktree preservation, paired cold measurements, tree hashes, and the real
   `scripts/dev/cargo-agent.ps1` path.
2. **PR 1 — policy and discovery:** add the pinned canonical advisory policy, thin Codex
   and Claude pointers, contract tests, and the supervisor-aware PowerShell wrapper.
3. **PR 2 — admission runtime:** add fenced durable build state, fair scheduling, resource
   budgets, local authenticated control transport, supervised Cargo execution, status,
   timeout, cancellation, evidence, and recovery.
4. **PR 3 — provider enforcement:** route autonomous and interactive sessions through the
   supervisor, fail closed on missing/stale policy, and permit clean historical-contract
   worktrees to resume in place without policy-path mutation.
5. **PR 4 — evidence and rollout:** exercise the evidence schema once, add the bounded
   experiment harness, run cross-provider/Windows pilots, then promote advisory → shadow →
   enforced only when the recorded gates pass.

## Acceptance

- Each release's commands in
  `docs/superpowers/plans/2026-07-12-cross-agent-build-admission.md` pass on its exact tree.
- Final exact candidate: fmt, workspace Clippy/tests, ignored nextest profile, authority,
  Codex contract, and web lint/typecheck/test all green.
- Windows two-worktree proof covers focused concurrency, exclusive fairness, policy refresh
  byte preservation, stale-fence rejection, and evidence-preserving recovery.
- Append-only runtime/operations memory and JOURNAL write-back land with the implementation.

