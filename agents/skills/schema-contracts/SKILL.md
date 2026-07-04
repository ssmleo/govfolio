# skill: schema-contracts
Purpose: one definition, enforced everywhere
Load when: authoring details types, GoldCandidate changes, OpenAPI surface
Core checklist:
- schemars type -> snapshot JSON Schema committed -> mirror SQL CHECKs in validate() -> regen contracts, git diff --exit-code
Anti-patterns: hand-editing generated files; unsynced Rust/SQL rules
Learnings (dated):
- 2026-07-04: schemars 1.x derive honors `#[serde(try_from = "Raw")]` and demands `JsonSchema`
  on the raw type — leaking the internal name into the committed contract. To keep validating
  deserialization without polluting the schema: derive `JsonSchema`+`Serialize` on the real type,
  write a manual `Deserialize` that funnels through the constructor (`map_err(de::Error::custom)`).
- 2026-07-04: doc comments ARE contract surface — schemars embeds them as `description`, so a
  doc edit fails the snapshot test. Correct response: regenerate (UPDATE_SNAPSHOT=1) and commit
  the diff, never weaken the test.
Write-back: deepen this file when the procedure teaches you something; same PR.
