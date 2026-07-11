# Loop provider compatibility and native-host rollout

Ordinary supervisor tests and ticks never call a live model and never install WSL.

## Ownership after Release 2

- `orchestrator-0` is Codex-primary in its dedicated branch/worktree.
- Claude runs only in a separate factory worktree. Providers never share a worktree.
- Lane ownership requires a green exact executable/CLI/model/config compatibility proof.
- Cross-provider takeover is fresh. Exact resume is same-provider only after proof.

## Native Codex resolution

Resolution order is authoritative `GOVFOLIO_CODEX_BIN`, first successful PATH candidate,
then exactly one successful `%LOCALAPPDATA%/OpenAI/Codex/bin/*/codex.exe`. The persisted
identity contains canonical path, CLI version, and executable SHA-256.

Permission denial, timeout, missing tools, ambiguity, Git-common failure, and linker
failure never authorize WSL. Only the OS bad-executable-format result does. The native
smoke uses a disposable detached worktree outside the repo, proves the same writable
Git common directory, runs exact Codex `--version`, links/runs real Rust, then cleans up.

## Live compatibility canary

A live canary is explicit and potentially billable. It is never a test or scheduler
tick. Run one per provider/CLI/model/executable fingerprint in a disposable worktree.

The canary pins one tracked `agents/skills/**/SKILL.md` SHA-256 and a random challenge,
without revealing the hash to the model. Green requires both a completed structured
load/read event referencing that exact path and a `.govfolio-loop/` marker containing
the independently recomputed hash/challenge. Agent prose is not evidence. The fresh
turn must report the configured model and capture a session/thread ID; one exact-ID
resume then verifies the same model/session and terminal behavior. Proof is stored as a
content-addressed blob plus an exact compatibility record.

Run Codex's canary before assigning `orchestrator-0`. Run Claude's canary only in its
separate factory worktree. A fingerprint change returns only that provider to
`needs_probe`.

Use explicit models and the pre-built supervisor from the exact merged `main` checkout:

```powershell
$env:GOVFOLIO_CODEX_MODEL = '<approved-codex-model>'
$env:GOVFOLIO_CLAUDE_MODEL = '<approved-claude-model>'
govfolio-loop probe-native-codex
govfolio-loop canary codex agents/skills/rust-tdd/SKILL.md
govfolio-loop canary claude agents/skills/rust-tdd/SKILL.md
```

The `run` command refuses to acquire a lane unless its exact provider identity has a
current green proof. Codex's persisted identity includes the resolved executable hash;
a proof for a different binary, CLI version, model, or configuration cannot authorize
the lane. Immediately before every provider spawn, the Rust supervisor also runs the
checked-in Codex skill renderer in check mode and validates the role/skill dispatch
contract inside that lane's actual worktree. Any drift blocks before model spend.

## Bounded takeover

`orchestrator-0` starts with `GOVFOLIO_LOOP_PROVIDER=codex` and a Claude fallback.
Set the fallback explicitly when desired:

```powershell
$env:GOVFOLIO_LOOP_FALLBACK_PROVIDER = 'claude'
```

Only a clean transient transport or provider-unavailable result may spend the single
fresh alternate-provider recovery. The original provider process has already exited
and the recovery retains the same monotonically increasing lane fence and work key.
It never resumes the other provider's session. A second failure, stale fence, missing
proof, dirty worktree, session ambiguity, quota, authentication, runner policy, or
operator stop does not spend another model turn; the lane is fenced for recovery.

After fencing, inspect `govfolio-loop status`, the bounded attempt evidence, Git state,
and receipt/lease state. Stop only the verified supervisor process tree, repair from
Git/registry/Bronze/receipt authority, rerun both exact canaries if a fingerprint
changed, and restart the pre-built supervisor. Never delete evidence or reset an
uninspected worktree.

## WSL status and optional bootstrap

Status is read-only and succeeds even when WSL is absent:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/bootstrap-loop-wsl.ps1 status
```

Verification rejects WSL1, root workers, non-ext4 lane roots, missing Linux-native
tools, Windows interop shims, and `docker-desktop` worker selection:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/bootstrap-loop-wsl.ps1 verify
```

Installation requires a fresh supervisor-produced `govfolio.native-unsupported/v1`
proof and an explicit switch:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/dev/bootstrap-loop-wsl.ps1 install `
  -NativeUnsupportedProof C:\path\to\native-unsupported.json `
  -ConfirmInstall
```

Never select a `docker-desktop` distro or place lane worktrees below `/mnt/c`.
