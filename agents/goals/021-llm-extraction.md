# 021 — LLM extraction fallback

## Objective
Implement the extractor interface stubbed in plan Task 8: schema-constrained LLM extraction for low-confidence/scanned PDFs, sha-cached, confidence-scored, second-model cross-check for high-impact rows.

## Context (read first)
- design §5.3, §4.3 · packages/pipeline/src/adapter.ts

## Acceptance criteria
```bash
pnpm --filter pipeline test -- extraction
pnpm conformance --filter adapters/us_house   # scanned fixture case goes green
```

## Checklist
- [ ] extractor iface impl  - [ ] cache by sha+version  - [ ] confidence  - [ ] cross-check  - [ ] scanned fixture
