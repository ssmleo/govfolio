# skill: schema-contracts
Purpose: one definition, enforced everywhere
Load when: authoring details types, GoldCandidate changes, OpenAPI surface
Core checklist:
- schemars type -> snapshot JSON Schema committed -> mirror SQL CHECKs in validate() -> regen contracts, git diff --exit-code
Anti-patterns: hand-editing generated files; unsynced Rust/SQL rules
Write-back: deepen this file when the procedure teaches you something; same PR.
