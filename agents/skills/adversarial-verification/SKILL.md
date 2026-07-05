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
Write-back: deepen this file when the procedure teaches you something; same PR.
