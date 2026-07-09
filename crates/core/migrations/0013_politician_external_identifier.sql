-- 0013_politician_external_identifier: universal, nullable durable per-filer
-- identifier (design: docs/decisions/politician-identity-resolution-design.md
-- §3.1). Expand-only. Populated only where a regime's raw source data has a
-- durable national/official id (today: `br`'s CPF/voter-registration
-- number); NULL everywhere else — zero behavior change for every regime that
-- doesn't populate it. Not unique-constrained: the column mixes id
-- namespaces across regimes (a br CPF and a future uk MNIS id are both plain
-- `text`), and `pipeline::stages::roster::resolve_hits` already scopes
-- matching to one regime via `mandate.body`/`district`.

alter table politician add column external_identifier text;
