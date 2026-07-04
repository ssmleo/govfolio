# skill: extraction-strategy
Purpose: choose deterministic vs LLM per document class
Load when: SPEC/BUILD parse decisions
Core checklist:
- text layer present? deterministic first -> measure confidence -> LLM constrained to schema as fallback -> record choice + why in SAF
Anti-patterns: LLM-first on parseable docs; unrecorded strategy flips
Write-back: deepen this file when the procedure teaches you something; same PR.
