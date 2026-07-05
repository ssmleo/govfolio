# 040 — public website

## Objective
Next.js SSR consuming /v1 only: politician profile + timeline, record page with provenance (official link + archived copy + verification badge + supersession history), jurisdiction scorecard pages, search, sitemap.xml, CDN/ETag caching.

## Context (read first)
- design §6.4, §7.3 · generated TS client from packages/contracts/openapi.json (openapi-typescript); bootstrap pnpm workspace in this goal; add contract drift check to CI web job

## Acceptance criteria
```bash
pnpm --filter web test && pnpm e2e   # Playwright: profile, record provenance, search, sitemap
```

## Checklist
- [ ] profile  - [ ] record+provenance  - [ ] jurisdiction pages  - [ ] search  - [ ] sitemap  - [ ] cache headers

## Progress
- 2026-07-05 (leg A, rust-builder): /v1 READ surface the pages consume is complete —
  `GET /v1/politicians` (cursor + `q` ILIKE over name/alias), `GET /v1/politicians/{id}`
  (profile: mandates + record summary), `GET /v1/records/{id}` (provenance: filing /
  raw_document / regime + supersession chain both directions via recursive CTE),
  `GET /v1/jurisdictions` (regimes joined = scorecard source), `GET /v1/regimes`,
  `GET /v1/search` (typed envelope; plain ILIKE, Typesense deferred per design §6.4),
  strong ETag + If-None-Match→304 middleware on every GET. Contract-tested against the
  emitted OpenAPI (10 contract tests incl. an edit-resolution supersession seed + 304
  path); `packages/contracts/openapi.json` regenerated. Page checkboxes above stay for
  the web leg.
