# Goal 113 — supervisor pre-receipt tool denylist hardening

Status: `[ ]` not started. Founder-surfaced 2026-07-12 from the adversarial review of the
goal-112 PR (#9, `codex/root-dispatch-runtime`, merged at main `8387b23`).

## Source finding (Medium, defense-in-depth)

`classify_pre_receipt_tool` in `crates/supervisor/src/supervisor.rs` marks a pre-receipt
tool call `Allowed` when it (a) contains a recognized read-verb substring, (b) references
only allowlisted paths, and (c) contains no forbidden substring. The forbidden list omits
common destructive/exfil verbs — notably `rm `, `del `, `rmdir`, output redirection
(`>`, `>>`), and network tools (`curl`, `wget`). A crafted pre-receipt command such as
`readFile('agents/skills/one/SKILL.md'); rm -rf …` therefore classifies as `Allowed` and is
not early-rejected.

**Blast radius is bounded** — this is hardening, not an active receipt bypass:
- The exact standalone-line `SKILLS_LOADED` receipt is still required for a `Completed`
  verdict (`apply_root_receipt_postcondition`), so a decoy cannot fake success.
- Any repo mutation (dirty tree / HEAD change / JOURNAL edit) is independently caught by
  `apply_postconditions`.
- The provider runs under its own sandbox.

## Invariants
- Fail closed. The pre-receipt classifier must never widen what reaches a `Completed`
  verdict; it may only reject earlier.
- Behavior-preserving for legitimate allowlisted reads (no regression in the goal-112
  receipt/allowlist tests).
- No `unwrap()`/`expect()`/`unsafe` in new non-test code (clippy-denied).

## Tasks
1. Harden `classify_pre_receipt_tool` to a **default-deny** posture: a pre-receipt tool is
   `Allowed` only when it is an explicit recognized read-verb over allowlisted-paths-only
   with no other command content; anything else — destructive verbs (`rm`, `del`, `rmdir`),
   shell redirection (`>`, `>>`, `|` to a writer), network fetchers (`curl`, `wget`), or any
   unrecognized shape — classifies `ForbiddenTool`.
2. Add unit tests in `crates/supervisor` for each newly denied case, including the
   "read-then-destructive" compound command from the finding, proving it is `ForbiddenTool`
   pre-receipt and never reaches `Completed`.
3. Keep the goal-112 positive cases (legitimate allowlisted read → `Allowed`; missing/late/
   mismatched receipt; decoy tool; recovery ordering) green.

## Acceptance
- `cargo test -p loop-supervisor` green, including the new denial tests (named test proves
  the compound "read + rm" command is `ForbiddenTool`).
- `cargo clippy --all-targets -- -D warnings` and `cargo fmt --check` clean.
- No `unwrap`/`expect`/`unsafe` added outside tests.
- Journal write-back appended in the same PR.

## Files (expected)
- `crates/supervisor/src/supervisor.rs` (`classify_pre_receipt_tool` + tests)
- `agents/JOURNAL.md` (write-back)
