# Open the exact filing document, per politician, across all jurisdictions

## Context

Today, a user can only reach a filing's source in an indirect, two-click way: politician
page → click an asset description → land on `/r/[id]` → read `ProvenanceBlock`'s "Official
source" link. That link always points at the **government's own URL**, never at anything
we archived ourselves — even though Bronze already captures the real bytes
(`raw_document.storage_uri`, `sha256`, `mime_type`, `fetched_at`) and the design doc's own
§7.3 ("Trust surface") explicitly planned "official-source link **+ our archived copy**"
for every record page. The archived copy currently only shows as a `sha256` hash — an
integrity anchor, not something you can actually open.

Research surfaced four real problems this feature needs to solve, not just one:

1. **No entry point where the user actually is.** The politician timeline
   (`RecordRows.tsx`) has no source/document affordance at all — only the buried
   detail-page link.
2. **We only ever link to the gov's URL, which can rot or change**, and — critically —
   for Brazil the recorded `source_url` is the **same nationwide bulk ZIP for every
   candidate in a year**, not anything specific to that politician. Serving our own
   archived copy is the only way BR ever gets a politician-specific document at all.
3. **A live bug mislabels every fetched document as `application/pdf`** regardless of
   actual content (`crates/pipeline/src/run.rs:316`), so UK's raw JSON and US-Senate/
   Canada's HTML reports would be served/rendered wrong even once archived-copy serving
   exists.
4. **Brazil's archived bytes are a synthetic per-candidate reconstruction** (a join of two
   unrelated nationwide TSE files), not verbatim government-issued bytes — this needs
   honest labeling, not silent conflation with the other jurisdictions' genuine captures.

**Decisions made with the user during brainstorming:**
- Scope = serve our own durable archived Bronze copy (not just link out to the gov's site).
- Entry point = one grouped "View filing →" link per filing in the politician's timeline
  (not per-row, not detail-page-only) — confirmed the timeline sorts by insertion-ordered
  ULID (`records.rs:32`, `order by id`), not `event_date`, so one filing's rows reliably
  land adjacent to each other and grouping is safe.
- Execute directly in an interactive session (no goal-queue entry — see the companion
  implementation plan, `docs/plans/2026-07-09-filing-document-viewer-implementation.md`).

**Scope call surfaced during design (flagging explicitly, not burying it):** the prod/GCS
serving path depends on the founder's already-tracked cloud-substrate halt (ADC login,
`agents/goals/000-INDEX.md` 020/081) and can't be verified end-to-end in this sandboxed
session without live credentials — writing unverifiable signed-URL code would violate
verify-before-completion. This pass ships the **local backend fully working and verified**
(real BR + US-House data already exists locally per goals 082/092), designs the read path
so a GCS backend drops in later without touching the endpoint or frontend at all, and
makes an unsupported `storage_uri` scheme fail loudly rather than silently.

## Design

### 1. Storage read — `crates/api`
A small function resolving a `raw_document.storage_uri` to bytes. Dispatches purely on the
URI's scheme (no new config surface needed): `file://` (the only scheme any adapter
currently writes, via `pipeline::adapter::BronzeStore`) reads the literal path directly —
different backfills use different Bronze root directories, so the stored URI's exact path,
not a re-derived one against an assumed root, is the only correct address. `gs://` (object
storage, not yet implemented) fails closed with `503 Unavailable`.

### 2. New endpoint — `GET /v1/filings/{id}/document`
New `crates/api/src/routes/filings.rs`. Joins `filing → raw_document`, the same shape
already used in `fetch_record_detail` (`records.rs:256-264`).

**Must gate visibility identically to `GET /v1/records/{id}`** — reuse `RecordFilter`/
`auth.filter()` so a free-tier caller can't bypass the 24h embargo (goal 050) by hitting
this endpoint directly for a filing whose records aren't visible to them yet. This is a
real gap a naive filing-id-only lookup would introduce; the endpoint checks that at least
one `disclosure_record` referencing this filing is visible under the caller's tier/filter
before serving anything.

Response: `200` with streamed bytes + correct `Content-Type` (see mime fix below). ETag/304
support comes for free from the existing `etag` middleware (it hashes every 200 GET body
generically, `crates/api/src/etag.rs`) — no custom caching logic needed. Regenerate the
OpenAPI contract (`cargo run -p api --bin openapi`) since this is a new path — this is the
`/filings/{id} (+ raw-doc link)` the design doc §6.1 originally planned, scoped down to
just the document sub-resource since filing metadata is already served via `/v1/records/
{id}`'s `provenance.filing`.

### 3. mime_type bugfix — `crates/pipeline`
`sniff_mime` (`stages/ingest.rs:15-21`) only distinguishes PDF-magic-bytes vs.
`application/octet-stream` today. Extend it to also detect HTML (`<!DOCTYPE`/`<html`
prefix) and JSON (leading `{`/`[` + valid UTF-8), covering every live adapter's real byte
shape (PDF: Australia/US-House; HTML: US-Senate/Canada; JSON: UK, and BR's synthetic
reconstruction). Fix `Runner::fetch_remote` (`run.rs:306-329`) to read the fetched bytes
back via the existing `BronzeStore::get` and call `sniff_mime` on them, instead of
hardcoding the literal `"application/pdf"` at line 316 — mirroring what the offline path
(`fetch_bookkeeping`, already calls `sniff_mime` correctly at line 347) already does.

### 4. Brazil disclaimer — frontend only, no adapter/data change
Can't retroactively relabel BR's already-ingested Bronze bytes (immutable — invariant 2),
and won't bake a note into future ones either (conflates data with presentation). Instead:
a small, explicitly-commented client-side check against BR's two known regime ids
(`crates/adapters/br/src/seed.rs`: `REGIME_ID`/`REGIME_ID_SENADO`) renders a muted caption
next to BR filings' "View filing" link — "reconstructed per-candidate from TSE's bulk
disclosure files" — honest about what the archived JSON actually is (a join of two
nationwide files, not a verbatim government single-candidate document). This is a
deliberate, narrow exception to "no per-jurisdiction branching in the frontend," justified
because the underlying data genuinely differs in kind for BR, not just in presentation.

### 5. Frontend — grouped entry point
`RecordRows.tsx`: group the already-fetched page's records by contiguous `filing_id` runs
and render one extra header row per group: a "View filing" link to `/v1/filings/
{filing_id}/document`, `target="_blank" rel="noopener noreferrer"` — a deliberate, noted
departure from this app's no-new-tab convention (losing timeline scroll position to view a
document is worse than a new tab). No list-endpoint schema change needed — `filing_id`
already rides on every `DisclosureRecord`.

Also upgrade the **existing** "Archived copy" row in `ProvenanceBlock.tsx` (today just
`fetched_at` + `sha256` text) to include an actual link to the new endpoint, alongside (not
replacing) the existing gov "Official source" link. Swap `BronzeDocument.tsx`'s
reviewer-only iframe `src` from the external gov URL to the new endpoint too — this is
literally the moment its own code comment says it's been waiting for ("serving our
archived GCS copy... is post-020-apply").

## Critical files
- `crates/pipeline/src/stages/ingest.rs` — `sniff_mime` extension
- `crates/pipeline/src/run.rs` — `fetch_remote` mime fix (~line 306-329)
- `crates/api/src/bronze.rs` — **new**, storage read (`read_document`)
- `crates/api/src/routes/filings.rs` — **new**, the document endpoint
- `crates/api/src/routes/records.rs` — pattern mirrored for the join + `RecordFilter`
  visibility gating (lines 244-268)
- `crates/api/src/lib.rs` — route registration + OpenAPI contract
- `packages/contracts/openapi.json` / `src/api.d.ts` — regenerated, never hand-edited
- `apps/web/src/components/RecordRows.tsx` — grouping + "View filing" link
- `apps/web/src/components/ProvenanceBlock.tsx` — archived-copy link upgrade
- `apps/web/src/components/reviewer/BronzeDocument.tsx` — swap iframe source
- No new migrations — `raw_document`/`filing` already carry everything needed.

## Verification
- `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
- New unit tests for extended `sniff_mime` (pdf/html/json/octet-stream fixtures) and the new
  `bronze::read_document` (local file read, unsupported scheme, missing file)
- New integration tests for `GET /v1/filings/{id}/document`: 200 + correct bytes + correct
  `Content-Type`; 404 unknown id; embargo test confirming a free-tier caller gets the same
  denial `/v1/records/{id}` would for a not-yet-visible filing
- `pnpm --filter web lint|typecheck|test && pnpm e2e` — new Playwright case: open a real
  politician's timeline, click a grouped "View filing" link, confirm it opens/downloads
  the document (extends `apps/web/e2e/record.spec.ts` conventions)
- Manual: against local dev DB (already has real backfilled BR + US-House data per goals
  082/092), click through politicians in at least 2 different jurisdictions and confirm
  the browser renders/downloads the correct file with the correct content type

See `docs/plans/2026-07-09-filing-document-viewer-implementation.md` for the task-by-task
implementation plan.
