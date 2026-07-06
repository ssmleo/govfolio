# skill: regime-research
Purpose: find and understand disclosure regimes
Load when: SCOUT/SURVEY phases
Core checklist:
- start: parliament/ethics-commissioner/official-gazette domains -> trace legal basis -> identify bodies, filers, record types -> note language(s)
Anti-patterns: assuming US-like structure; trusting aggregators as primary
Learnings (dated):
- 2026-07-05 (uk_commons_register): before studying any HTML register, probe for a
  dedicated open-data API — pattern `{service}-api.{parliament domain}/swagger/v1/
  swagger.json` (UK Parliament ships one per service: members-api, interests-api,
  whatson-api). The interests API's published OpenAPI contract replaced an entire
  HTML-anatomy study and carries the category scheme + typed money in-band.
- 2026-07-05 (canada_ciec): no API is not the end of machine-readability — a
  rebuilt registry (ASP.NET/Dynamics) exposed stable CRM GUIDs for every entity
  (types, roles, persons, declarations, per-ITEM disclosure divs) behind plain GET
  query params + a bare `data-cards-url` HTML-fragment endpoint for sweeps: join on
  GUIDs, display labels. Also read the site's own notices page early: an official
  "temporary website / phased transition" notice is a drift forecast — record it as
  the sentinel's top watch item in the SAF.
- 2026-07-06 (br scout): don't assume a country's "annual public-servant asset
  declaration" regime covers legislators — Brazil's CGU-run e-Patri explicitly
  scopes to Executive branch only (its own FAQ states Legislative/Judicial staff
  "não devem apresentar"); federal deputies/senators file a separate internal DBR
  with their own house instead, which reads as compliance-only (no public search UI
  found). The only confirmed PUBLIC federal asset disclosure is TSE's
  DivulgaCandContas, and it's candidacy-snapshot (per-election), not annual/in-office
  — verify cadence explicitly, don't assume "annual" just because a lead says so.
Write-back: deepen this file when the procedure teaches you something; same PR.
