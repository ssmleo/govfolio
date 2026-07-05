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
- 2026-07-04: adapter details schemas — type lives in the adapter crate, snapshot committed
  under `crates/pipeline/schemas/details/<regime>.<record_type>.json` (pipeline `include_str!`s
  it into the conformance registry; no dep cycle because only the JSON crosses). Bootstrap
  order matters: `include_str!` needs the file BEFORE the generating test can compile — seed a
  `{}` placeholder, then `UPDATE_SNAPSHOT=1 cargo test -p <adapter> --test details_schema_snapshot`.
- 2026-07-04: OpenAPI surface reuses core wire types via a feature-gated derive — core
  grows `utoipa = { optional = true }` + `#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]`
  on the enums/ValueInterval; the derive honors serde attrs (`rename_all`, `rename = "self"`),
  so SQL CHECK tokens and money-as-strings stay single-sourced. `#[schema(pattern = ...)]`
  takes only string LITERALS (a `const` is "expected string literal").
- 2026-07-05: one struct, two doors with different strictness: a filter type reused as BOTH
  a query-string extractor (must tolerate foreign params like `cursor`) and a stored jsonb
  contract (must reject unknown keys) keeps serde lenient and makes the SCHEMA strict via
  `#[schemars(extend("additionalProperties" = false))]`; enforce at the write door by
  validating the raw JSON against the committed snapshot (jsonschema) BEFORE serde. Related
  utoipa: `ToSchema` + `IntoParams` derives coexist on one struct (feature-gated cfg_attr);
  hand-impl `PartialSchema`/`ToSchema` on newtypes to mirror a manual `JsonSchema` impl
  (string + pattern) so both documents stay one shape.
- 2026-07-04: contract-testing OpenAPI 3.1 responses with `jsonschema`: build the validation
  doc as `{"$schema": 2020-12, "allOf": [<response schema node>], "components": doc.components}`
  so internal `#/components/schemas/...` pointers resolve; then PROVE the validator has teeth
  against a garbage body — a silently unresolved ref validates everything. Deterministic emit
  for the committed openapi.json: recursive explicit key sort (never the map backing — see
  rust-tdd preserve_order learning) + trailing newline; verify by double-emit sha compare.
Write-back: deepen this file when the procedure teaches you something; same PR.
