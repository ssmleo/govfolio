# 040 — public website

## Objective
Next.js SSR consuming /v1 only: politician profile + timeline, record page with provenance (official link + archived copy + verification badge + supersession history), jurisdiction scorecard pages, search, sitemap.xml, CDN/ETag caching.

## Context (read first)
- design §6.4, §7.3 · packages/contracts client

## Acceptance criteria
```bash
pnpm --filter web test && pnpm e2e   # Playwright: profile, record provenance, search, sitemap
```

## Checklist
- [ ] profile  - [ ] record+provenance  - [ ] jurisdiction pages  - [ ] search  - [ ] sitemap  - [ ] cache headers
