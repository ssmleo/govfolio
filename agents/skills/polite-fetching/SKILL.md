# skill: polite-fetching
Purpose: durable, lawful scraping
Load when: any network fetch of a source
Core checklist:
- read ToS/robots -> conditional GETs -> min-interval + concurrency 1 -> identified UA with contact -> log politeness incidents to SAF
Anti-patterns: parallel hammering; ignoring 429s; anonymous UA
Write-back: deepen this file when the procedure teaches you something; same PR.
