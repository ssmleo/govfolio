# 100 — authority-lock-and-validator

## Objective
Make invariant 9 mechanical — a tamper-evident authority lock, a fail-closed
`validate-authority` bin at every run point, and below-the-model write protection (design §4.2).

## Scope
In:
- Pre-existing unlisted files: `agents/goals/022-adversarial-review-loop.md` and
  `023-extraction-tier-labeling.md` (committed b2139b8, unreviewed import proposals) are
  quarantined FIRST — `git mv` into `agents/goals/_quarantine/` (outside the bijection glob,
  which scans `agents/goals/*.md` non-recursively) with a one-line provenance note appended to
  each; never execute them; 000-INDEX gets no rows for them. The bijection gate must go green
  deterministically on this tree.
- `agents/AUTHORITY.lock.json` — sha256 pins over the authority set
  (`agents/GOVERNANCE.md`, `PROMPT.md`, `LOOP.md`, `workflows/orchestration.md`,
  `roles/*.md`, `archetypes/*.md`, `EFFORT.md`, `EPOCHS.md`, `goals/000-INDEX.md`,
  root `/CLAUDE.md` — per design Amendment 1, it carries the invariants + the universal
  memory pointer; nested folder CLAUDE.md stubs are NOT pinned — low-stakes pointers,
  edited casually as folders evolve);
  schema `{version, superseded_note?, pinned}`. Goal files are NOT content-pinned
  (legitimately mutable — progress blocks, checklists) — covered by the bijection check.
  Regenerated only via `--write-lock`, through the amendment path.
- `validate-authority` bin in the pipeline factory family (`crates/pipeline/src/factory.rs`
  + bin shim, same pattern as `crates/pipeline/src/bin/validate_survey.rs`). Checks
  (design §4.2): (a) goals↔`000-INDEX.md` bijection — an unlisted `agents/goals/*.md`
  (template excepted) → exit 1 + a quarantine report with git provenance (`git log -- <file>`);
  (b) every pinned path's sha256 matches the lock, missing/extra authority file → exit 1;
  (c) `--ci` amendment discipline — a diff touching authority files MUST update the lock in
  the same commit AND the commit message MUST reference an INDEX-listed goal. Fail closed on
  ANY ambiguity. Flags: `--write-lock` regenerates the lock; `--check-path <p>` fast
  single-path mode for the hook (no repo walk).
- Seeded-violation fixtures + `#[test]`s: unlisted goal file, tampered role file,
  missing/stale lock, `--check-path` deny (exit 2).
- `agents/run-loop.sh`: run the PRE-BUILT bin before each `claude` invocation; nonzero exit
  halts the iteration, logs, does NOT launch the session.
- `agents/workflows/orchestration.md` step 0: replace the manual INTEGRITY listing with
  running the bin (prose stays as explanation; the bin is the gate; the quarantine-report
  duty stays, now fed by the bin's output).
- `.github/workflows/ci.yml` guardrails job: a `validate-authority` step (Rust toolchain or
  prebuilt-bin cache in that job — implementer's choice).
- PreToolUse hook (repo `.claude/settings.json`, additive — MUST NOT clobber the user's
  global RTK hook): a shell script calling the PRE-BUILT binary (never `cargo run` — hook
  latency); blocks Read of unlisted `agents/goals/*.md` and Write/Edit of the authority set +
  the lock + `.claude/settings*` + the `.claude/hooks/` dir; exit code 2. Amendment path per
  §4.2: the hook checks the branch name only (cheap, synchronous); the goal-reference and
  lock-updated-in-same-commit conditions are enforced by check (c) at the run points, per
  design §4.2.
- `agents/GOVERNANCE.md`: one-line rationale per rule + `## Amendments (append-only, dated)` section.

Out: memory contract + index (goal 101), SAF normalization (102), hygiene loop (103),
OS-level sandboxing, remote branch-protection config.

> Bootstrap: the hook is inactive until this goal wires it, so wire `.claude/settings.json`
> LAST — the lock, the bin, and the hook config land on this ordinary goal branch without
> being blocked by the mechanism they install. Goals 101–103 amend pinned files and MUST use
> the §4.2 amendment path.

## Context (read first)
- docs/plans/2026-07-10-memory-authority-substrate-design.md §4.2, §5, §6, §8
- CLAUDE.md invariant 9 (goal-queue integrity)
- agents/workflows/orchestration.md · agents/GOVERNANCE.md · agents/run-loop.sh · agents/PROMPT.md
- crates/pipeline/src/factory.rs + crates/pipeline/src/bin/validate_survey.rs (bin pattern)
- docs/regimes/us-house/reference/E1.lock.json (pin + supersede precedent)
- docs/decisions/automation-policy.md (halt semantics)
- .claude/settings.json (additive hook wiring)

## Acceptance criteria (all must pass)
```bash
cargo run -p pipeline --bin validate-authority   # exit 0 on the clean tree
cargo test -p pipeline validate_authority        # seeded: unlisted goal / hash mismatch -> exit 1; --check-path deny -> exit 2
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
```

## Checklist
- [ ] quarantine pre-existing unlisted 022/023 into agents/goals/_quarantine/ (provenance b2139b8); bijection green on clean tree
- [ ] `AUTHORITY.lock.json` schema + `--write-lock` generation over the pinned authority set
- [ ] Bijection check (a): goals↔000-INDEX; unlisted goal → exit 1 + quarantine report w/ git provenance
- [ ] Hash-match check (b): pinned paths vs lock; missing/extra authority file → exit 1
- [ ] `--ci` amendment-discipline check (c) + `--check-path <p>` fast single-path mode
- [ ] Seeded-violation fixtures + `#[test]`s (unlisted goal, tampered role, missing/stale lock, --check-path deny) proven red
- [ ] `agents/run-loop.sh` pre-iteration halt wired (pre-built bin; nonzero → no session)
- [ ] `agents/workflows/orchestration.md` step 0 replaced by running the bin (quarantine duty retained)
- [ ] `.github/workflows/ci.yml` guardrails job gains a `validate-authority` step
- [ ] `agents/GOVERNANCE.md` per-rule rationales + `## Amendments (append-only, dated)` section
- [ ] PreToolUse hook script + repo `.claude/settings.json` additive wiring (LAST — see Bootstrap)
- [ ] Full acceptance block green; memory/SAF write-back + JOURNAL line; committed; checklist + 000-INDEX row ticked

## BLOCKED (human)
(empty)
