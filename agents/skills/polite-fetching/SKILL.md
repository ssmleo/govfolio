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
Write-back: deepen this file when the procedure teaches you something; same PR.
