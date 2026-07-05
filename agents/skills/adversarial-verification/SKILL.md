# skill: adversarial-verification
Purpose: verify like you want it to fail
Load when: AUDITOR passes
Core checklist:
- re-derive each claim from evidence only -> verdict per claim -> bounce with actionable notes -> never audit own production
Anti-patterns: rubber-stamping; vague bounces
Learnings (dated):
- 2026-07-04: auditing auto-resolved expected outputs — cheapest proof they weren't
  regenerated from parser output is commit ORDER: `git log --oneline -- <fixtures>`
  must show the fixture commit predating the implementation commit, with no later
  touch. Pair with re-hashing inputs against the SAF pins, then re-derive via a
  THIRD extraction path the authors didn't use (e.g. visual PDF render when they
  used two text extractors) so shared-tool blind spots can't align.
- 2026-07-05: serialization CONVENTIONS need a raw-text scan, not a parsed
  compare — JSON.parse/serde erase the `1.0`-float vs `1`-integer literal
  distinction, so a deep-compare can pass while the committed literal violates
  the MANIFEST convention. Scan the bytes for the literal form separately.
  Also: no python on host ≠ no third path — a hand-rolled tag tokenizer in
  node stdlib is disjoint from any spec-compliant DOM parser and doubles as
  an independent §3.7-style integrity re-computation.
Write-back: deepen this file when the procedure teaches you something; same PR.
