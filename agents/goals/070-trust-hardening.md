# 070 — trust hardening

## Objective
Monthly sampling-audit job + precision report, public corrections log page, per-regime redaction pass (pre-publication), adapter drift detection with fail-closed freeze.

## Context (read first)
- design §7.4–7.5, §5.6

## Acceptance criteria
```bash
pnpm --filter pipeline test -- redaction drift && pnpm --filter web test -- corrections
```

## Checklist
- [ ] sampler  - [ ] corrections page  - [ ] redaction rules/regime  - [ ] drift freeze
