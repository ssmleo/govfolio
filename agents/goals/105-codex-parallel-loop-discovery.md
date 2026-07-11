# 105 — codex-parallel-loop-discovery

## Objective

Record the disposition of the untracked Codex parallel-loop stack discovered on
2026-07-11 without reading it as authority, adopting it, deleting its evidence, or
allowing its retry storm to continue.

## Findings and provenance

The root checkout contained an untracked Codex runner, four untracked lane logs, and
four `codex/lane/*` worktrees. Git reported no history for the untracked artifacts.
At containment time the Codex logs were growing rapidly, the tracked Claude runner
was also live, and neither stack had a singleton or provider circuit.

Invariant 9 therefore treats the untracked runner and logs as quarantined evidence,
not executable instructions. Their contents are not incorporated into the clean-room
implementation.

## Founder decision (2026-07-11)

The attached and approved `Autonomous Claude–Codex Loop Hardening` plan resolves the
former human halt:

- Stop only process trees verified as autonomous loop/provider descendants.
- Preserve all worktrees, branches, dirty files, and raw logs for reconciliation.
- Do not adopt, commit, execute, delete, or rewrite the untracked Codex runner.
- Build official-protocol Claude and Codex adapters cleanly under goals 108–110.
- Keep WSL optional; first test a real native Codex binary in a disposable worktree.

Containment stopped the verified retry-storm trees. A second process audit found no
remaining loop/provider process, and observed log sizes remained stable.

## Checklist

- [x] Discovery and zero-history provenance recorded
- [x] Verified loop process trees stopped without deleting evidence
- [x] Founder disposition recorded: clean-room rebuild, no adoption
- [x] Follow-up implementation registered as goals 108–110

## BLOCKED (human)

(empty)

