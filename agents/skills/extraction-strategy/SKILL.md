# skill: extraction-strategy
Purpose: choose deterministic vs LLM per document class
Load when: SPEC/BUILD parse decisions
Core checklist:
- text layer present? deterministic first -> measure confidence -> LLM constrained to schema as fallback -> record choice + why in SAF
Anti-patterns: LLM-first on parseable docs; unrecorded strategy flips
Learnings (dated):
- 2026-07-04 (us_house PTR): "text layer present?" is not one question — test DATA
  cells and LABELS separately. Generated PDFs can style labels (small caps) with fonts
  whose reduced glyphs have no ToUnicode: labels extract lossy-but-deterministically
  while data survives verbatim. Anchor grammar on data tokens (date pairs, $-bands)
  and surviving label capitals, never on full label text. Verify with TWO independent
  extractors before blaming the crate; check content-stream order vs layout order —
  layout mode can interleave wrapped cells that content order keeps contiguous.
Write-back: deepen this file when the procedure teaches you something; same PR.
