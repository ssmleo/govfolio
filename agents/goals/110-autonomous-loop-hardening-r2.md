# 110 — autonomous-loop-hardening-r2

## Objective

Ship Release 2: provider-neutral logical lane 0 with exact compatibility canaries and
fresh Claude↔Codex recovery, while provisioning WSL only if a native smoke proves it
necessary.

## Scope

User amendment (2026-07-11): Codex owns `orchestrator-0` after its compatibility and
skill-load proofs; Claude is the separately proven fallback/factory provider. This
supersedes the provider preference below without weakening any fencing requirement.

In:

- Stable `orchestrator-0` lane/worktree/branch with monotonic fence.
- Codex owns lane 0 and Claude is fallback; old process group must be dead/fenced before
  takeover, and providers never overlap in the lane worktree.
- Disposable canary per provider/CLI/model/executable fingerprint: structured turn,
  exact session/thread capture, exact resume, terminal/exit/stdout/stderr verification.
- Every live Claude and Codex canary mechanically proves loading one tracked
  repository-approved skill through a structured load event plus a hash/challenge
  marker artifact; agent prose is not evidence.
- Fresh recovery by default; exact-session resume only for a proven fingerprint;
  cross-provider recovery is always fresh.
- Native Codex resolver order: explicit env, successful PATH candidate, then one
  successful `%LOCALAPPDATA%/OpenAI/Codex/bin/*/codex.exe` candidate; persist path,
  version, and executable hash.
- Disposable native linked-worktree/Git-common/GCC smoke before WSL.
- Idempotent `status|install|verify` WSL2 bootstrap only on native unsupported;
  reject `docker-desktop`, Windows interop shims, non-WSL2, root operation, and
  non-ext4 lane worktrees.

## Acceptance

```bash
cargo test -p loop-supervisor compatibility
cargo test -p loop-supervisor failover
cargo test -p loop-supervisor native_resolver
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/bootstrap-loop-wsl.ps1 status
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

No live provider turn is part of ordinary tests. The explicit canary command is a
bounded rollout action and records its structured evidence/fingerprint. WSL install is
not allowed unless the native smoke returns the specific unsupported capability.

## Checklist

- [x] Exact canary and fingerprint state machine with mechanical skill proof
- [x] Native Codex resolver and disposable linked-worktree smoke
- [x] Fenced lane-0 selection/takeover/recovery tests
- [x] Idempotent fail-closed WSL bootstrap + fake-runner tests
- [x] Claude/Codex canary evidence recorded; native used if green
- [x] Full acceptance, memory write-back, committed and merged to main

Live rollout evidence (2026-07-11): native Codex linked-worktree smoke passed on
Windows without WSL; Codex compatibility proof
`sha256:9161dc75557a0cb5b1a03d48acd24bfe579ba783647f8a14b1dde086b5754ced`
proved `gpt-5.6-sol`; Claude compatibility proof
`sha256:b56183de7f91f1d739eefc3395635419cb7627567e3c5a0353a004ea145a3a9a`
proved `claude-opus-4-8[1m]`. Both proofs include structured loading of the tracked
`rust-tdd` skill, its approved SHA-256 marker, and exact session/thread resume.

## BLOCKED (human)

(empty)
