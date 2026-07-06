# skill: polite-fetching
Purpose: durable, lawful scraping
Load when: any network fetch of a source
Core checklist:
- read ToS/robots -> conditional GETs -> min-interval + concurrency 1 -> identified UA with contact -> log politeness incidents to SAF
Anti-patterns: parallel hammering; ignoring 429s; anonymous UA
Learnings (dated):
- 2026-07-05 (us_senate eFD): some hosts (Akamai bot manager) mechanically 403 ANY
  non-stock UA string and any non-browser TLS fingerprint — an identified UA can be
  UNSERVABLE. Fallback that keeps invariant 10 honest: stock browser client + contact
  identification via the standard `From:` header on every request; record the deviation
  + probe matrix in the SAF. Diagnose with a probe LADDER (plain client → browser UA
  strings → real browser → real browser + custom UA → real browser + From header),
  changing ONE variable per request, never blind-retrying a 403. If the documented
  client gets blocked later: freeze + work item — no fingerprint-evasion arms race.
- 2026-07-05 (uk_commons_register): bot posture is PER-HOST, not per-organization —
  interests-api.parliament.uk served the identified UA 24/24 while
  publications.parliament.uk Cloudflare-403'd the same client the same hour. Run the
  probe ladder per host; prefer a documented public API host (published OpenAPI +
  contact address = designed-for-reuse signal) over any HTML route of the same org.
- 2026-07-05 (canada_ciec): robots.txt can be a catch-all HTML route — a 200 whose
  body is the site's home page is NOT a robots policy (and not permission either).
  Verify the body looks like robots grammar before treating any robots response as
  policy; absence means self-imposed limits govern (invariant 10), same as a 404.
- 2026-07-06 (br TSE DivulgaCandContas): a CDN-level redirect to a generic
  "indisponivel"/maintenance page, reproduced identically under both an identified
  UA and a stock browser UA, reads as a genuine outage rather than a bot-block
  (contrast the canada_ciec/us_senate cases where only non-browser UAs were
  blocked). Same host's robots.txt was also just this catch-all page (echoes the
  canada_ciec robots lesson) — don't treat it as policy. Corroborate official
  status via the org's other live subdomains when the primary portal is down;
  re-probe before committing to "unavailable" as a durable finding.
Write-back: deepen this file when the procedure teaches you something; same PR.
