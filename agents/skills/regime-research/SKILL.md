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
Write-back: deepen this file when the procedure teaches you something; same PR.
