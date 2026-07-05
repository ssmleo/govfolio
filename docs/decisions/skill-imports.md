# Skill imports — decisions & audit log

## superpowers @ d884ae04edebef577e82ff7c4e143debd0bbec99 (obra/superpowers) — vendored 2026-07-04
Method: shallow clone, pinned sha, vendor-copy (no plugin marketplace — pinning over auto-update).
Screen: automated red-flag scan (exec/network/secret patterns) + spot-read.
FULL line-audit remains on goal 019 (auditor pass + founder sample) before any merge into bespoke skills.

| skill | lines | red-flag scan |
|---|---|---|
| brainstorming | 159 | clean |
| dispatching-parallel-agents | 185 | clean |
| executing-plans | 70 | clean |
| finishing-a-development-branch | 241 | clean |
| receiving-code-review | 213 | clean |
| requesting-code-review | 103 | clean |
| subagent-driven-development | 418 | clean |
| systematic-debugging | 296 | clean |
| test-driven-development | 371 | clean |
| using-git-worktrees | 202 | clean |
| using-superpowers | 62 | clean |
| verification-before-completion | 139 | clean |
| writing-plans | 174 | clean |
| writing-skills | 689 | 2 hits = doc links to agentskills.io spec (classified benign at screen; 019 line-audit anyway) |

## 019 Phase A — remaining imports located, pinned, vendored 2026-07-05 (scout)
Same method: shallow clone (identified UA), pinned HEAD sha, vendor-copy to
`agents/skills/imported/<name>@<sha12>/` with upstream LICENSE. Status of ALL rows below:
**AUDIT PENDING** — quarantined, nothing ACTIVE; Phase B line-audit + founder sample gate activation.

| import | source repo @ sha | upstream path | files | lines | red-flag screen | license |
|---|---|---|---|---|---|---|
| impeccable (pack, plugin v3.9.1) | pbakaus/impeccable @ 582f23eae3c9ef4db71366e944b0555d65b7aacc | plugin/ | 100 | 50482 | 244 hits — CODE-BEARING: 86 exec(, 26 fetch(, 6 child_process, 2 spawn(, 44 .env, 61 https://, misc secret/credential strings, concentrated in skills/impeccable/scripts/*.mjs (detector/live-server tooling). NOT classifiable benign at screen; scripts MUST be fully line-audited (or excluded) in Phase B before any activation | Apache-2.0 |
| rust-best-practices | apollographql/skills @ 7df6a608dd71f937e664b19183fb60d101bb13a1 | skills/rust-best-practices/ | 11 | 2430 | 56 hits = 54 doc links, 1 tokio spawn( example, 1 `validate_credentials` doc example — benign at screen | MIT |
| rust-async-patterns | wshobson/agents @ 5cc2549a50fc672230efd0a0307e2fd27ffba792 | plugins/systems-programming/skills/rust-async-patterns/ | 3 | 518 | 9 hits = 8 tokio::spawn examples + 1 doc link — benign at screen | MIT |
| typescript-advanced-types | wshobson/agents @ 5cc2549a50fc672230efd0a0307e2fd27ffba792 | plugins/javascript-typescript/skills/typescript-advanced-types/ | 3 | 722 | 7 hits = `password` fields in form-validation type examples — benign at screen | MIT |
| frontend-design | anthropics/skills @ 9d2f1ae187231d8199c64b5b762e1bdf2244733d | skills/frontend-design/ | 2 | 55 | 1 hit = doc link — benign at screen | Apache-2.0 (skill-local LICENSE.txt) |

### Skipped (fail closed)
- **typescript-react-reviewer — SKIPPED(no-license).** Canonical source located:
  dotneet/claude-code-marketplace @ 07fa7eac95c2323f73e5a8a961b70bb9e207f1d0,
  review-tool/skills/typescript-react-reviewer/. Repo contains NO license file anywhere
  (checked root, subdirs, README). Default copyright — cannot vendor-copy. Options:
  upstream adds a license; or author a bespoke reviewer skill (A3: bespoke > imported).
- **typescript-expert — SKIPPED(ambiguous).** Multiple plausible origins, none canonical:
  davila7/claude-code-templates (aitmpl.com), martinholovsky/claude-skills-generator
  (generator-produced), sickn33/antigravity-awesome-skills (aggregator — excluded on
  aggregator grounds). wshobson/agents and apollographql/skills do NOT carry it.
  Needs founder pick or bespoke authoring (A3).

### Supply-chain cross-check vs untracked `.agents/skills/` (skills-CLI installs, unpinned)
- rust-best-practices: `.agents` copy is byte-identical to fresh apollographql/skills @ 7df6a608 — MATCH.
- rust-async-patterns: `.agents` copy is byte-identical to fresh wshobson/agents @ 5cc2549a — MATCH.
- tdd-rust (`.agents`, source rtk-ai/rtk): not a 019 target; not vendored. Bespoke rust-tdd remains authoritative (A3).
