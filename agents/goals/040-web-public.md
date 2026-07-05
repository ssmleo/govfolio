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
- [x] profile  - [x] record+provenance  - [x] jurisdiction pages  - [x] search  - [x] sitemap  - [x] cache headers

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
- 2026-07-05 (leg B, web-builder): public site COMPLETE — pnpm workspace (corepack pnpm
  11.10.0), Next 16 App Router strict TS (`no-explicit-any` lint-denied), generated
  client `packages/contracts/src/api.d.ts` (openapi-typescript, committed; CI web job
  regen-drift-gated). Pages: `/` (search + latest records), `/p/[id]` (+ permanent
  `/p/[id]/from/[cursor]` timeline pages — cursor pagination in the PATH so every page
  is CDN-cacheable), `/r/[id]` (all fields, provenance card, badge, supersession both
  directions, confidence <1), `/jurisdictions` (+`/[id]` scorecard tables),
  `/search?q=`, `/sitemap.xml` (index → politicians/records/jurisdictions urlsets).
  ISR s-maxage+SWR on entity pages (empty `generateStaticParams` required to opt
  dynamic-param routes into ISR — explicit `cache:"no-store"` in the fetch wrapper
  silently forces full-dynamic; both learned the hard way, e2e-asserted now). Server
  fetch layer sends If-None-Match and serves 304s from a bounded etag+body cache.
  Money renders via Intl string path (never parseFloat; unit test proves exactness past
  2^53). Evidence: vitest 22/22, Playwright 11/11 (profile, record trust surface,
  search, sitemap, cache headers) against seeded local api.
- NOTE (leg A follow-up, non-blocking): "latest records" on `/` walks ascending-ULID
  pages to the tail (bounded); an `order=desc` param on `/v1/records` would make it
  O(1) when the corpus grows.

## BLOCKED (human lane — public copy)
Site ships with deliberately minimal, factual copy (hero line, footer, badge
explanations, empty states). Per automation-policy the PUBLIC claim-making /
methodology / legal copy lane is human-only: before launch a founder pass is needed on
(1) home hero + footer wording, (2) verification-badge explanations, (3) absence of any
legal disclaimer ("not investment advice", corrections policy) — none was added because
that text is the human lane. Artifacts to review: `apps/web/src/app/layout.tsx`,
`apps/web/src/app/page.tsx`, `apps/web/src/components/VerificationBadge.tsx`.
Recommendation: keep wording as-filed-neutral (design §7.5); nothing blocks the loop.
