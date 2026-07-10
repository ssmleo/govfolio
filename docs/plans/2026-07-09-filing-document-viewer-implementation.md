# Open the exact filing document — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a user open the archived original filing document for any politician's
disclosure, from the politician's own timeline, across every jurisdiction — serving OUR
OWN durable Bronze copy (not just linking to the government's URL), per the design at
`docs/plans/2026-07-09-filing-document-viewer-design.md`.

**Architecture:** One new public, tier-gated `GET /v1/filings/{id}/document` route in
`crates/api` that joins `filing -> raw_document`, reuses `RecordFilter::SQL_WHERE` so it
can never see more than `/v1/records/{id}` would, and reads the bytes straight off
`raw_document.storage_uri` (a `file://` path today — every real backfill uses
`pipeline::adapter::BronzeStore`, which already writes exactly this). A one-line pipeline
bugfix makes `raw_document.mime_type` actually correct for every jurisdiction's real byte
shape (PDF/HTML/JSON). The politician timeline groups by `filing_id` (data already on
every row) and adds one "View filing" link per group; Brazil's link additionally gets an
honest caption because its archived bytes are a synthesized per-candidate reconstruction,
not verbatim government bytes.

**Tech Stack:** Rust (axum, sqlx raw SQL, utoipa, tokio), TypeScript (Next.js server
components, Vitest, Playwright). No new external dependencies — `crates/api` already
depends on `pipeline`; only a `tokio` feature flag (`fs`) is added.

## Global Constraints

- The new endpoint MUST gate visibility identically to `GET /v1/records/{id}` — reuse
  `govfolio_core::query::RecordFilter::SQL_WHERE` via `auth.filter()`. A free-tier caller
  must get the same 404 for an embargoed filing that `/v1/records/{id}` would give for one
  of its records. Never build a bespoke visibility check for this route.
- No new DB migrations. `raw_document`/`filing` already carry every column needed
  (`storage_uri`, `sha256`, `mime_type`, `discovered_at`).
- The GCS/object-storage backend is explicitly OUT of scope this pass — it depends on the
  founder's already-tracked cloud-substrate halt (`agents/goals/000-INDEX.md` 020/081) and
  cannot be verified without live credentials. An unsupported `storage_uri` scheme (i.e.
  anything other than `file://`) must fail closed with `503 Unavailable`, never silently
  wrong output.
- Reuse `pipeline::adapter::BronzeStore`'s existing content-addressing conventions
  conceptually, but read bytes directly via the `storage_uri` column (it is already the
  exact absolute path written at ingest time — different adapters/backfills use different
  Bronze root directories, so re-deriving a path from a single assumed root would be
  wrong; the stored URI is always correct).
- Every Rust task: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace` must stay green.
- Every web task: `pnpm --filter web lint`, `pnpm --filter web typecheck`, `pnpm --filter web test` must stay green.
- Money/decimal, supersede-never-update, and every other existing invariant in `/CLAUDE.md`
  are unaffected by this feature and must not be touched.

---

### Task 1: Fix `mime_type` — sniff real content instead of hardcoding `application/pdf`

**Files:**
- Modify: `crates/pipeline/src/stages/ingest.rs:15-21` (the `sniff_mime` function + its test module at the bottom of the file)
- Modify: `crates/pipeline/src/run.rs:306-329` (`Runner::fetch_remote`)

**Interfaces:**
- Produces: `pub fn sniff_mime(bytes: &[u8]) -> &'static str` now returns one of
  `"application/pdf"`, `"text/html"`, `"application/json"`, `"application/octet-stream"`
  (previously only the first and last). Signature unchanged — no caller needs updating
  except the bugfix in `fetch_remote` below.

**Context:** `sniff_mime` today only tells PDF apart from everything else. The LIVE fetch
path (`fetch_remote`, used by every real/non-fixture adapter) doesn't even call it — it
hardcodes the literal string `"application/pdf"` regardless of what was actually fetched
(`run.rs:316`). This means every live-fetched UK filing (raw JSON) and US-Senate/Canada
filing (HTML report) is currently mislabeled in the `raw_document.mime_type` column. The
OFFLINE/fixture path (`fetch_bookkeeping`, `run.rs:334-378`) already does this correctly at
line 347 (`ingest::sniff_mime(bytes)`) — `fetch_remote` is the one that's wrong.

- [ ] **Step 1: Write the failing tests** — add to the `#[cfg(test)] mod tests` block at
  the bottom of `crates/pipeline/src/stages/ingest.rs` (keep the existing
  `sniffs_pdf_magic_and_falls_back_to_octet_stream` test as-is; add these two new tests
  below it):

```rust
    #[test]
    fn sniffs_html_by_doctype_or_tag() {
        assert_eq!(sniff_mime(b"<!DOCTYPE html><html></html>"), "text/html");
        assert_eq!(sniff_mime(b"<html><body>report</body></html>"), "text/html");
        assert_eq!(sniff_mime(b"  <!DOCTYPE html>\n<html></html>"), "text/html");
    }

    #[test]
    fn sniffs_json_objects_and_arrays_but_not_other_leading_brackets() {
        assert_eq!(sniff_mime(br#"{"a":1}"#), "application/json");
        assert_eq!(sniff_mime(b"[1,2,3]"), "application/json");
        assert_eq!(sniff_mime(b"<?xml version=\"1.0\"?>"), "application/octet-stream");
    }
```

- [ ] **Step 2: Run to verify red**

Run: `cargo test -p pipeline stages::ingest::tests -- --nocapture`
Expected: FAIL — both new tests fail because `sniff_mime` doesn't detect HTML/JSON yet
(they currently return `"application/octet-stream"` for everything non-PDF).

- [ ] **Step 3: Extend `sniff_mime`** — replace the existing function
  (`crates/pipeline/src/stages/ingest.rs:12-21`) with:

```rust
/// Best-effort mime sniff for the `raw_document.mime_type` column; the byte
/// content is the authority, not the URL suffix. Covers every live adapter's
/// real byte shape: PDF (Australia/US-House), HTML (US-Senate/Canada report
/// pages), JSON (UK's API response; Brazil's synthesized per-candidate join).
#[must_use]
pub fn sniff_mime(bytes: &[u8]) -> &'static str {
    let trimmed = leading_ascii_whitespace_trimmed(bytes);
    if trimmed.starts_with(b"%PDF-") {
        "application/pdf"
    } else if starts_with_html(trimmed) {
        "text/html"
    } else if looks_like_json(trimmed) {
        "application/json"
    } else {
        "application/octet-stream"
    }
}

fn leading_ascii_whitespace_trimmed(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(bytes.len());
    &bytes[start..]
}

fn starts_with_html(bytes: &[u8]) -> bool {
    const PREFIXES: [&[u8]; 4] = [b"<!DOCTYPE", b"<!doctype", b"<html", b"<HTML"];
    PREFIXES.iter().any(|prefix| bytes.starts_with(prefix))
}

fn looks_like_json(bytes: &[u8]) -> bool {
    matches!(bytes.first(), Some(b'{') | Some(b'['))
        && std::str::from_utf8(bytes).is_ok()
}
```

- [ ] **Step 4: Run to verify green**

Run: `cargo test -p pipeline stages::ingest::tests -- --nocapture`
Expected: PASS — all three tests (existing PDF/octet-stream one plus the two new ones).

- [ ] **Step 5: Fix the live-path bug** — in `crates/pipeline/src/run.rs`, replace the
  `fetch_remote` method (lines 306-329) with:

```rust
    async fn fetch_remote(
        &self,
        filing_ref: &FilingRef,
        run_id: &str,
    ) -> anyhow::Result<(RawDocRef, String)> {
        let doc = self.adapter.fetch(filing_ref, &self.ctx).await?;
        let bytes = self.ctx.bronze.get(&doc)?;
        let raw_document_id = ingest::ensure_raw_document(
            &self.pool,
            &doc,
            &self.storage_uri(&doc),
            ingest::sniff_mime(&bytes),
            Some(&filing_ref.url),
            self.ctx.clock.now(),
            Some(run_id),
        )
        .await?;
        finish_ok(
            &self.pool,
            run_id,
            json!({ "sha256": doc.sha256, "raw_document_id": raw_document_id }),
        )
        .await?;
        Ok((doc, raw_document_id))
    }
```

  (The only changes: a new `let bytes = self.ctx.bronze.get(&doc)?;` line, and
  `ingest::sniff_mime(&bytes)` replacing the hardcoded `"application/pdf"` literal —
  mirrors exactly what `fetch_bookkeeping` already does a few lines below it.)

- [ ] **Step 6: Verify the whole crate still builds and passes**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -p pipeline`
Expected: all green. (No existing test asserts the wrong `"application/pdf"` value for a
non-PDF live fetch, so this is not expected to break anything — if it does, that test was
relying on the bug and should be corrected to expect the sniffed value instead.)

- [ ] **Step 7: Commit**

```bash
git add crates/pipeline/src/stages/ingest.rs crates/pipeline/src/run.rs
git commit -m "fix(pipeline): sniff real mime type on the live fetch path instead of hardcoding application/pdf"
```

---

### Task 2: Bronze document reader — resolve `storage_uri` to bytes

**Files:**
- Create: `crates/api/src/bronze.rs`
- Modify: `crates/api/src/lib.rs` (add `pub mod bronze;` near the other `pub mod` lines at the top)
- Modify: `crates/api/Cargo.toml` — add `"fs"` to the `tokio` feature list (line 33):
  `tokio = { version = "1.52.3", features = ["rt-multi-thread", "macros", "net", "fs"] }`

**Interfaces:**
- Produces: `pub async fn read_document(storage_uri: &str) -> Result<Vec<u8>, ApiError>` —
  Task 3's route handler calls this directly with the `storage_uri` column value.

**Context:** `raw_document.storage_uri` is always a `file://<absolute path>` string today
(every adapter writes through `pipeline::adapter::BronzeStore`, which sets it to
`format!("file://{}", path.display())` — see `crates/pipeline/src/run.rs:580-582`).
Different backfills use different Bronze root directories (e.g.
`target/bronze-backfill-real` for US-House, `target/bronze-backfill-real-br` for Brazil —
`crates/worker/src/bin/backfill-real.rs:153`, `backfill-real-br.rs:450-452`), so the stored
URI's exact path — not a re-derived one — is the only correct address to read. Object
storage (`gs://`) is intentionally NOT implemented; that scheme must fail closed with
`ApiError::Unavailable` (503), never attempt a network call or panic.

- [ ] **Step 1: Write the failing tests** — create `crates/api/src/bronze.rs` with just
  the test module first:

```rust
//! Reads a `raw_document`'s archived bytes from wherever `storage_uri` says
//! they live (design §7.3: "our archived copy"). Dispatches on the URI's
//! scheme: local filesystem today (`file://` — the only scheme any adapter
//! currently writes, via `pipeline::adapter::BronzeStore`); object storage
//! (`gs://`) is a documented not-yet-implemented gap, not silently wrong — it
//! fails closed with `503` until the cloud-substrate halt clears
//! (`agents/goals/000-INDEX.md` 020/081).

use crate::error::ApiError;

/// Reads the archived bytes a `raw_document.storage_uri` points at.
///
/// # Errors
/// [`ApiError::Unavailable`] for a scheme this build cannot read yet;
/// [`ApiError::Internal`] on an I/O failure reading a local file.
pub async fn read_document(storage_uri: &str) -> Result<Vec<u8>, ApiError> {
    todo!()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reads_bytes_from_a_file_scheme_uri() {
        let path = std::env::temp_dir()
            .join(format!("govfolio-bronze-test-{}.bin", ulid::Ulid::new()));
        tokio::fs::write(&path, b"hello bronze").await.unwrap();
        let uri = format!("file://{}", path.display());

        let bytes = read_document(&uri).await.unwrap();

        assert_eq!(bytes, b"hello bronze");
        tokio::fs::remove_file(&path).await.unwrap();
    }

    #[tokio::test]
    async fn unsupported_scheme_fails_closed_with_503_not_a_panic() {
        let err = read_document("gs://bucket/object").await.unwrap_err();
        assert!(matches!(err, ApiError::Unavailable { .. }));
    }

    #[tokio::test]
    async fn missing_local_file_is_an_internal_error_not_a_panic() {
        let err = read_document("file:///definitely/not/a/real/path/xyz-govfolio-test")
            .await
            .unwrap_err();
        assert!(matches!(err, ApiError::Internal(_)));
    }
}
```

Add `pub mod bronze;` to `crates/api/src/lib.rs` (alongside the existing `pub mod auth;`
etc. at the top of the file), and add `"fs"` to the `tokio` feature list in
`crates/api/Cargo.toml:33`.

- [ ] **Step 2: Run to verify red**

Run: `cargo test -p api bronze:: -- --nocapture`
Expected: FAIL (compiles, panics on `todo!()`) for the first test; the crate must compile
first — if it doesn't (missing `tokio::fs`/`ulid` availability), fix the Cargo.toml feature
addition before proceeding.

- [ ] **Step 3: Implement** — replace the `todo!()` function body:

```rust
pub async fn read_document(storage_uri: &str) -> Result<Vec<u8>, ApiError> {
    let Some(path) = storage_uri.strip_prefix("file://") else {
        return Err(ApiError::Unavailable {
            code: "storage_backend_unavailable",
            message: format!(
                "no storage backend implemented yet for {storage_uri} \
                 (object storage arrives once the cloud-substrate halt clears)"
            ),
        });
    };
    tokio::fs::read(path)
        .await
        .map_err(|e| ApiError::from(anyhow::anyhow!("reading bronze document at {path}: {e}")))
}
```

- [ ] **Step 4: Run to verify green**

Run: `cargo test -p api bronze:: -- --nocapture`
Expected: PASS — all three tests.

- [ ] **Step 5: Full crate check**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test -p api`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/api/src/bronze.rs crates/api/src/lib.rs crates/api/Cargo.toml
git commit -m "feat(api): read archived Bronze document bytes from storage_uri"
```

---

### Task 3: `GET /v1/filings/{id}/document` route

**Files:**
- Create: `crates/api/src/routes/filings.rs`
- Modify: `crates/api/src/routes/mod.rs` (add `pub mod filings;`)
- Modify: `crates/api/src/lib.rs` (wire the route into the router + `ApiDoc` paths/tags)
- Create: `crates/api/tests/filings.rs`

**Interfaces:**
- Consumes: `bronze::read_document` (Task 2), `govfolio_core::query::RecordFilter`,
  `crate::auth::AuthContext::filter()` (all pre-existing except Task 2's new function).
- Produces: the route `GET /v1/filings/{id}/document`, registered in the OpenAPI contract —
  Task 4/5 (frontend) will link to
  `${apiBaseUrl()}/v1/filings/{filing_id}/document`.

**Context:** This mirrors the join already done in `fetch_record_detail`
(`crates/api/src/routes/records.rs:256-264`) but keyed by filing id directly, and folds the
SAME `RecordFilter::SQL_WHERE` visibility gate into the query as an `exists` check — a
filing's document is never more visible than its own records are (this is the mechanism
that prevents a free-tier caller from bypassing the 24h embargo, goal 050). Every existing
GET response already gets a strong ETag + 304 support for free from the `etag` middleware
in `crates/api/src/lib.rs` (it hashes any 200 body generically) — do not add custom
Cache-Control/ETag handling in this route.

- [ ] **Step 1: Write the failing integration tests** — create
  `crates/api/tests/filings.rs`:

```rust
//! `GET /v1/filings/{id}/document`: serves the archived Bronze copy, gated by
//! the SAME freshness bound as `/v1/records/{id}` (goal 050 — a filing's
//! document must never be more visible than its own records).
//!
//! DB-gated like the other sqlx suites: `--ignored` + postgres on `DATABASE_URL`.
#![allow(clippy::unwrap_used)]

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use http_body_util::BodyExt as _;
use tower::ServiceExt as _;

use api::ApiConfig;

fn test_app(pool: &sqlx::PgPool) -> Router {
    api::app(pool.clone(), ApiConfig::new())
}

struct Seeded {
    filing_id: String,
    document_path: std::path::PathBuf,
}

/// One filing + raw_document + disclosure_record, `discovered_at` controlled
/// by `age` (mirrors `crates/api/tests/tiers.rs::seed_two_ages`'s shape, but
/// returns the filing id and writes a REAL temp file so the document route
/// can actually read bytes back).
async fn seed_filing(pool: &sqlx::PgPool, age: Duration, bytes: &[u8], mime: &str) -> Seeded {
    govfolio_core::db::migrate(pool).await.unwrap();
    let politician_id = ulid::Ulid::new().to_string();
    let regime_id = ulid::Ulid::new().to_string();
    let raw_id = ulid::Ulid::new().to_string();
    let filing_id = ulid::Ulid::new().to_string();
    let record_id = ulid::Ulid::new().to_string();

    sqlx::query("insert into jurisdiction (id, name, level) values ('us', 'United States', 'national')")
        .execute(pool)
        .await
        .unwrap();
    sqlx::query(
        "insert into disclosure_regime \
           (id, jurisdiction_id, body, regime_type, value_precision, effective_from) \
         values ($1, 'us', 'US House', 'transaction_report', 'banded', '2012-01-01')",
    )
    .bind(&regime_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query("insert into politician (id, canonical_name) values ($1, 'Test Person')")
        .bind(&politician_id)
        .execute(pool)
        .await
        .unwrap();

    let document_path =
        std::env::temp_dir().join(format!("govfolio-filing-doc-test-{raw_id}.bin"));
    tokio::fs::write(&document_path, bytes).await.unwrap();
    let storage_uri = format!("file://{}", document_path.display());

    sqlx::query(
        "insert into raw_document (id, storage_uri, sha256, mime_type, fetched_at) \
         values ($1, $2, $3, $4, now())",
    )
    .bind(&raw_id)
    .bind(&storage_uri)
    .bind(format!("{raw_id}-sha256"))
    .bind(mime)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into filing \
           (id, regime_id, politician_id, raw_document_id, external_id, filing_type, discovered_at) \
         values ($1, $2, $3, $4, 'ext-1', 'ptr', $5)",
    )
    .bind(&filing_id)
    .bind(&regime_id)
    .bind(&politician_id)
    .bind(&raw_id)
    .bind(Utc::now() - age)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into disclosure_record \
           (id, filing_id, politician_id, regime_id, asset_description_raw, record_type, \
            asset_class, side, transaction_date, extracted_by, fingerprint) \
         values ($1, $2, $3, $4, 'test asset', 'transaction', 'equity', 'buy', '2026-06-01', \
                 'filings-test', $5)",
    )
    .bind(&record_id)
    .bind(&filing_id)
    .bind(&politician_id)
    .bind(&regime_id)
    .bind(format!("fp-{raw_id}"))
    .execute(pool)
    .await
    .unwrap();

    Seeded { filing_id, document_path }
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn serves_the_archived_bytes_with_the_sniffed_content_type(pool: sqlx::PgPool) {
    let seeded = seed_filing(&pool, Duration::hours(25), b"%PDF-1.7 test", "application/pdf").await;
    let app = test_app(&pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/filings/{}/document", seeded.filing_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "application/pdf"
    );
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&bytes[..], b"%PDF-1.7 test");
    tokio::fs::remove_file(&seeded.document_path).await.unwrap();
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn unknown_filing_is_404(pool: sqlx::PgPool) {
    govfolio_core::db::migrate(&pool).await.unwrap();
    let app = test_app(&pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/filings/01UNKNOWNFILINGID000000000/document")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[sqlx::test(migrations = false)]
#[ignore = "needs postgres"]
async fn embargoed_filing_document_404s_for_anonymous_free_tier_same_as_the_record_would(
    pool: sqlx::PgPool,
) {
    // Discovered 1 minute ago: invisible to the free tier (goal 050's 24h
    // delay) — this is the exact scenario a naive filing-id-only lookup
    // would leak.
    let seeded = seed_filing(&pool, Duration::minutes(1), b"fresh bytes", "text/html").await;
    let app = test_app(&pool);

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/v1/filings/{}/document", seeded.filing_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        response.status(),
        StatusCode::NOT_FOUND,
        "a not-yet-visible filing's document must 404 exactly like its record would"
    );
    tokio::fs::remove_file(&seeded.document_path).await.unwrap();
}
```

- [ ] **Step 2: Run to verify red**

Run: `docker compose up -d && DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p api --test filings -- --ignored --nocapture`
Expected: FAIL to compile (`routes::filings` doesn't exist yet).

- [ ] **Step 3: Implement the route** — create `crates/api/src/routes/filings.rs`:

```rust
//! Filing resources (design §6.1's originally-planned `/filings/{id} (+
//! raw-doc link)`, scoped to just the document sub-resource — filing
//! metadata already rides on `/v1/records/{id}`'s `provenance.filing`).
//! Serves OUR OWN archived copy of the original document (design §7.3:
//! "official-source link + our archived copy") rather than the government's
//! own URL, which can rot, change, or (Brazil) point at a nationwide bulk
//! file instead of anything politician-specific.

use axum::extract::{Extension, Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse as _, Response};
use const_format::concatcp;

use govfolio_core::query::RecordFilter;

use crate::AppState;
use crate::auth::AuthContext;
use crate::bronze;
use crate::error::{ApiError, ErrorBody};

/// The SAME visibility gate as `/v1/records/{id}` (`RecordFilter::SQL_WHERE`
/// binds `$1..=$11`; the filing id is `$12`): a filing's document is only
/// servable when at least one of its OWN disclosure records is visible under
/// the caller's tier. This is what stops a free-tier caller from bypassing
/// the 24h embargo (goal 050) by guessing a filing id directly.
const DOCUMENT_SQL: &str = concatcp!(
    "select d.storage_uri, d.mime_type \
     from filing f join raw_document d on d.id = f.raw_document_id \
     where f.id = $12 and exists ( \
       select 1 from disclosure_record where filing_id = f.id and ",
    RecordFilter::SQL_WHERE,
    ")"
);

/// Serves the archived original document for one filing.
///
/// # Errors
/// `404` for an unknown filing, or one not yet visible under the caller's
/// tier (the same freshness bound as every other record-serving route);
/// `503` if the document's storage backend is not implemented in this build;
/// `500` on backend failure.
#[utoipa::path(
    get,
    path = "/v1/filings/{id}/document",
    tag = "filings",
    params(("id" = String, Path, description = "Filing ULID")),
    responses(
        (status = 200, description = "The archived document bytes; Content-Type reflects the sniffed mime type"),
        (status = 404, description = "Unknown filing, or not yet visible under the caller's tier", body = ErrorBody),
        (status = 503, description = "Storage backend not available for this document", body = ErrorBody),
        (status = 500, description = "Internal error", body = ErrorBody),
    ),
)]
pub async fn get_filing_document(
    State(state): State<AppState>,
    Extension(auth): Extension<AuthContext>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let row: Option<(String, String)> = auth
        .filter()
        .bind_query_as(sqlx::query_as(DOCUMENT_SQL))?
        .bind(&id)
        .fetch_optional(&state.pool)
        .await?;
    let Some((storage_uri, mime_type)) = row else {
        return Err(ApiError::NotFound {
            message: format!("filing {id} not found"),
        });
    };
    let bytes = bronze::read_document(&storage_uri).await?;
    let content_type = HeaderValue::from_str(&mime_type)
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let mut response = (StatusCode::OK, bytes).into_response();
    response.headers_mut().insert(header::CONTENT_TYPE, content_type);
    Ok(response)
}
```

  If this exact `#[utoipa::path]` attribute fails to compile (utoipa's macro can be picky
  about responses with no `body =` schema for a binary payload), the fallback is to trim
  the `responses(...)` entries down to bare `(status = 200, description = "...")` /
  `(status = 404, description = "...", body = ErrorBody)` etc. until it compiles — the
  acceptance bar is a successful build and a clean contract regen (Step 6 below), not this
  literal macro invocation.

  Add `pub mod filings;` to `crates/api/src/routes/mod.rs` (alongside the other `pub mod`
  lines).

  In `crates/api/src/lib.rs`, add the route to the main public `Router::new()...` chain
  (anywhere among the other plain, non-admin `.route(...)` calls, e.g. right after the
  `/v1/records/{id}` line — this must NOT be behind `admin_gate`, it is public data like
  every other record-serving route):

```rust
        .route(
            "/v1/filings/{id}/document",
            get(routes::filings::get_filing_document),
        )
```

  Add `routes::filings::get_filing_document,` to the `paths(...)` list inside the
  `#[derive(OpenApi)] ... #[openapi(...)]` block on `ApiDoc`, and add a new tag entry to
  `tags(...)`:

```rust
        (name = "filings", description = "Archived original filing documents \
         (design §7.3: our own durable copy, not just a link to the \
         government's site). Same freshness gate as records."),
```

- [ ] **Step 4: Run to verify green**

Run: `docker compose up -d && DATABASE_URL=postgres://postgres:postgres@localhost:5433/govfolio cargo test -p api --test filings -- --ignored --nocapture`
Expected: PASS — all three tests.

- [ ] **Step 5: Full crate check**

Run: `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace`
Expected: all green.

- [ ] **Step 6: Regenerate the OpenAPI contract**

Run: `cargo run -p api --bin openapi > packages/contracts/openapi.json` (check the existing
`crates/api/src/bin/openapi.rs` for the exact invocation — it may write the file itself
rather than needing shell redirection; follow whatever the bin already does), then
`cd apps/web && pnpm generate` (or whatever `packages/contracts/package.json`'s generate
script is named — see its `package.json` description) to regenerate `src/api.d.ts`.
Expected: `packages/contracts/openapi.json` and `src/api.d.ts` both change to include the
new path/schema; commit them as generated files (never hand-edit).

- [ ] **Step 7: Commit**

```bash
git add crates/api/src/routes/filings.rs crates/api/src/routes/mod.rs crates/api/src/lib.rs \
        crates/api/tests/filings.rs packages/contracts/openapi.json packages/contracts/src/api.d.ts
git commit -m "feat(api): serve the archived original filing document at GET /v1/filings/{id}/document"
```

---

### Task 4: Politician timeline — group by filing, add "View filing" link

**Files:**
- Modify: `apps/web/src/components/RecordRows.tsx`
- Modify: `apps/web/src/components/RecordRows.test.tsx`
- Modify: `apps/web/src/lib/api.ts` (export `apiBaseUrl` is already exported — no change
  needed there; just import it in `RecordRows.tsx`)

**Interfaces:**
- Consumes: `apiBaseUrl()` (already exported from `@/lib/api`, `apps/web/src/lib/api.ts:178-180`).
- Produces: `RecordTable` (same public signature as today —
  `{ records: DisclosureRecord[]; caption: string }`) now also renders one filing-group
  header row per contiguous run of equal `filing_id`.

**Context:** The timeline's underlying query sorts `order by id` (insertion-ordered ULID —
confirmed at `crates/api/src/routes/records.rs:32`), NOT `event_date`, so one filing's rows
(published together in one pipeline run) are reliably adjacent in the array `RecordTable`
already receives — grouping by contiguous equal `filing_id` needs no re-sorting. Brazil's
two regime ids (`crates/adapters/br/src/seed.rs:82,102`:
`0BRAREG0000000000000000001` Câmara, `0BRAREG0000000000000000002` Senado) get an honest
caption because their archived document is a synthesized per-candidate reconstruction, not
verbatim government bytes (see the design doc for why).

- [ ] **Step 1: Write the failing tests** — replace
  `apps/web/src/components/RecordRows.test.tsx` with (adds three new `it` blocks; keeps all
  four existing ones unchanged):

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { RecordTable } from "@/components/RecordRows";
import { makeRecord } from "@/test/fixtures";

describe("RecordTable", () => {
  it("renders money from decimal strings via the string-safe Intl path", () => {
    render(<RecordTable records={[makeRecord()]} caption="test records" />);
    expect(screen.getByText("$1,001 – $15,000")).toBeInTheDocument();
  });

  it("keeps giant declared values exact (no parseFloat in the render path)", () => {
    const record = makeRecord({
      value: {
        low: "90071992547409931.55",
        high: "90071992547409931.55",
        currency: "USD",
      },
    });
    render(<RecordTable records={[record]} caption="test records" />);
    expect(screen.getByText("$90,071,992,547,409,931.55")).toBeInTheDocument();
  });

  it("links each row to its record page and shows the verification state", () => {
    const record = makeRecord();
    render(<RecordTable records={[record]} caption="test records" />);
    expect(
      screen.getByRole("link", { name: record.asset_description_raw }),
    ).toHaveAttribute("href", `/r/${record.id}`);
    expect(screen.getByText("Unverified")).toBeInTheDocument();
  });

  it("renders an honest empty state", () => {
    render(<RecordTable records={[]} caption="test records" />);
    expect(screen.getByText(/No disclosure records yet/)).toBeInTheDocument();
  });

  it("shows one 'View filing' link per filing, not one per record row", () => {
    const sameFiling = [
      makeRecord({ id: "rec-1", filing_id: "filing-a" }),
      makeRecord({ id: "rec-2", filing_id: "filing-a" }),
    ];
    render(<RecordTable records={sameFiling} caption="test records" />);
    const links = screen.getAllByRole("link", { name: "View filing" });
    expect(links).toHaveLength(1);
    expect(links[0]).toHaveAttribute(
      "href",
      expect.stringContaining("/v1/filings/filing-a/document"),
    );
    expect(links[0]).toHaveAttribute("target", "_blank");
    expect(links[0]).toHaveAttribute("rel", "noopener noreferrer");
  });

  it("shows a separate 'View filing' link for each distinct filing", () => {
    const records = [
      makeRecord({ id: "rec-1", filing_id: "filing-a" }),
      makeRecord({ id: "rec-2", filing_id: "filing-b" }),
    ];
    render(<RecordTable records={records} caption="test records" />);
    expect(screen.getAllByRole("link", { name: "View filing" })).toHaveLength(2);
  });

  it("captions Brazil's filings as a bulk-source reconstruction, not a verbatim document", () => {
    const record = makeRecord({
      filing_id: "filing-br",
      regime_id: "0BRAREG0000000000000000001",
    });
    render(<RecordTable records={[record]} caption="test records" />);
    expect(
      screen.getByText(/reconstructed per-candidate from TSE's bulk disclosure files/),
    ).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run to verify red**

Run: `pnpm --filter web test -- RecordRows`
Expected: FAIL — the three new tests fail (no "View filing" link exists yet).

- [ ] **Step 3: Implement** — replace the whole contents of
  `apps/web/src/components/RecordRows.tsx`:

```tsx
import Link from "next/link";

import type { DisclosureRecord } from "@/lib/api";
import { apiBaseUrl } from "@/lib/api";
import { formatDate, formatValueInterval } from "@/lib/format";
import { VerificationBadge } from "@/components/VerificationBadge";

// One record as a ledger row: date · action · asset (as filed) · value · state.
// Neutral as-filed language throughout (design §7.5).
function actionLabel(record: DisclosureRecord): string {
  if (record.record_type === "transaction") {
    return record.side ?? "transaction";
  }
  return record.record_type.replaceAll("_", " ");
}

// Brazil's archived document is a per-candidate JOIN of two nationwide TSE
// bulk files (crates/adapters/br/src/adapter.rs) — real, but a
// reconstruction, not verbatim government-issued single-candidate bytes.
// These are BR's two regime ids (crates/adapters/br/src/seed.rs REGIME_ID /
// REGIME_ID_SENADO). This is a deliberate, narrow exception to "no
// per-jurisdiction branching" — the underlying data genuinely differs in
// kind here, not just in presentation.
const BULK_RECONSTRUCTED_REGIME_IDS = new Set([
  "0BRAREG0000000000000000001",
  "0BRAREG0000000000000000002",
]);

function filingDocumentUrl(filingId: string): string {
  return `${apiBaseUrl()}/v1/filings/${encodeURIComponent(filingId)}/document`;
}

function FilingGroupHeader({ record }: { record: DisclosureRecord }) {
  return (
    <tr className="filing-group-header">
      <td colSpan={5}>
        <a
          href={filingDocumentUrl(record.filing_id)}
          target="_blank"
          rel="noopener noreferrer"
        >
          View filing
        </a>
        {BULK_RECONSTRUCTED_REGIME_IDS.has(record.regime_id) ? (
          <span className="muted">
            {" "}
            (reconstructed per-candidate from TSE&apos;s bulk disclosure files)
          </span>
        ) : null}
      </td>
    </tr>
  );
}

export function RecordRow({ record }: { record: DisclosureRecord }) {
  return (
    <tr className="record-row">
      <td className="cell-date">
        {record.event_date ? formatDate(record.event_date) : "—"}
      </td>
      <td className="cell-action">{actionLabel(record)}</td>
      <td className="cell-asset">
        <Link href={`/r/${record.id}`}>{record.asset_description_raw}</Link>
      </td>
      <td className="cell-value">
        {record.value ? formatValueInterval(record.value) : "—"}
      </td>
      <td className="cell-state">
        <VerificationBadge state={record.verification_state} />
      </td>
    </tr>
  );
}

export function RecordTable({
  records,
  caption,
}: {
  records: DisclosureRecord[];
  caption: string;
}) {
  if (records.length === 0) {
    return <p className="empty">No disclosure records yet for this view.</p>;
  }
  let previousFilingId: string | null = null;
  return (
    <table className="records">
      <caption className="visually-hidden">{caption}</caption>
      <thead>
        <tr>
          <th scope="col">Date</th>
          <th scope="col">Action</th>
          <th scope="col">Asset as filed</th>
          <th scope="col">Declared value</th>
          <th scope="col">State</th>
        </tr>
      </thead>
      <tbody>
        {records.map((record) => {
          const isNewFilingGroup = record.filing_id !== previousFilingId;
          previousFilingId = record.filing_id;
          return (
            <>
              {isNewFilingGroup ? (
                <FilingGroupHeader key={`${record.filing_id}-header`} record={record} />
              ) : null}
              <RecordRow key={record.id} record={record} />
            </>
          );
        })}
      </tbody>
    </table>
  );
}
```

  Note: `records.map` returning a fragment per item needs each mapped element to carry its
  own `key` — React fragments used this way need an explicit `key` on the outer `<>`
  (use `<React.Fragment key={record.id}>...</React.Fragment>` instead of the shorthand
  `<>` if the shorthand form does not accept a `key` prop under this project's JSX
  transform/React version; verify against `pnpm --filter web typecheck` and adjust to
  whichever form compiles clean).

- [ ] **Step 4: Run to verify green**

Run: `pnpm --filter web test -- RecordRows`
Expected: PASS — all seven tests.

- [ ] **Step 5: Full web checks**

Run: `pnpm --filter web lint && pnpm --filter web typecheck && pnpm --filter web test`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add apps/web/src/components/RecordRows.tsx apps/web/src/components/RecordRows.test.tsx
git commit -m "feat(web): group the politician timeline by filing and add a View filing link"
```

---

### Task 5: Upgrade `ProvenanceBlock` and reviewer `BronzeDocument` to use the archived copy

**Files:**
- Modify: `apps/web/src/components/ProvenanceBlock.tsx`
- Modify: `apps/web/src/components/ProvenanceBlock.test.tsx`
- Modify: `apps/web/src/components/reviewer/BronzeDocument.tsx`
- Modify: `apps/web/src/components/reviewer/BronzeDocument.test.tsx`

**Interfaces:**
- Consumes: `apiBaseUrl()` (`@/lib/api`), `provenance.filing.id` (already present on every
  `Provenance` — `apps/web/src/test/fixtures.ts:41-70` shows the shape).

**Context:** `ProvenanceBlock`'s "Archived copy" row today shows only `fetched_at` +
`sha256` as inert text — upgrade it to also link to the new endpoint, alongside (not
replacing) the existing "Official source" gov-URL link. `BronzeDocument` (reviewer-only,
`components/reviewer/`) currently iframes the EXTERNAL gov URL — its own code comment says
"serving our archived GCS copy... is post-020-apply"; this task is that moment. Swap its
iframe `src` to the new endpoint, keeping the external-URL fallback link as a secondary
option (not removing the "Official source" concept, since the gov URL is still useful
context even once we serve our own copy).

- [ ] **Step 1: Write the failing tests** — replace
  `apps/web/src/components/ProvenanceBlock.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ProvenanceBlock } from "@/components/ProvenanceBlock";
import { makeProvenance } from "@/test/fixtures";

describe("ProvenanceBlock", () => {
  it("shows the official source link, sha256, and fetch time", () => {
    const provenance = makeProvenance();
    render(<ProvenanceBlock provenance={provenance} />);

    const sourceUrl = provenance.raw_document.source_url ?? "";
    expect(screen.getByRole("link", { name: sourceUrl })).toHaveAttribute(
      "href",
      sourceUrl,
    );
    expect(screen.getByTestId("sha256")).toHaveTextContent(
      `sha256:${provenance.raw_document.sha256}`,
    );
    expect(screen.getByText(/fetched/)).toHaveTextContent("Jul 5, 2026");
    expect(screen.getByText(/fetched/)).toHaveTextContent("UTC");
  });

  it("links the regime to its jurisdiction page", () => {
    render(<ProvenanceBlock provenance={makeProvenance()} />);
    expect(screen.getByRole("link", { name: "US House" })).toHaveAttribute(
      "href",
      "/jurisdictions/us",
    );
  });

  it("says so plainly when no source URL was recorded", () => {
    const provenance = makeProvenance();
    provenance.raw_document = { ...provenance.raw_document, source_url: null };
    render(<ProvenanceBlock provenance={provenance} />);
    expect(
      screen.getByText("Source URL not recorded for this document"),
    ).toBeInTheDocument();
  });

  it("links the archived copy to our own document endpoint", () => {
    const provenance = makeProvenance();
    render(<ProvenanceBlock provenance={provenance} />);
    const link = screen.getByRole("link", { name: "Open archived copy" });
    expect(link).toHaveAttribute(
      "href",
      expect.stringContaining(`/v1/filings/${provenance.filing.id}/document`),
    );
    expect(link).toHaveAttribute("target", "_blank");
    expect(link).toHaveAttribute("rel", "noopener noreferrer");
  });
});
```

  Replace `apps/web/src/components/reviewer/BronzeDocument.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { BronzeDocument } from "@/components/reviewer/BronzeDocument";
import { makeProvenance } from "@/test/fixtures";

describe("BronzeDocument (right half of the side-by-side)", () => {
  it("embeds OUR archived copy, with a fallback link and the archived sha256", () => {
    const provenance = makeProvenance();
    render(<BronzeDocument provenance={provenance} />);
    const expectedUrl = expect.stringContaining(
      `/v1/filings/${provenance.filing.id}/document`,
    );
    expect(screen.getByTitle("Archived source document")).toHaveAttribute(
      "src",
      expectedUrl,
    );
    expect(
      screen.getByRole("link", { name: "open the archived document directly" }),
    ).toHaveAttribute("href", expectedUrl);
    expect(screen.getByTestId("bronze-sha256")).toHaveTextContent(
      "sha256:94781947c3975677a2fa8f7839f6c0f074b3d3a2ff6019b3cfd8ee4942f6262e",
    );
  });
});
```

- [ ] **Step 2: Run to verify red**

Run: `pnpm --filter web test -- ProvenanceBlock BronzeDocument`
Expected: FAIL (new/changed assertions don't match current markup).

- [ ] **Step 3: Implement `ProvenanceBlock`** — replace
  `apps/web/src/components/ProvenanceBlock.tsx`:

```tsx
import Link from "next/link";

import type { Provenance } from "@/lib/api";
import { apiBaseUrl } from "@/lib/api";
import { formatDate, formatDateTime } from "@/lib/format";

// The trust surface (design §7.3): official-source link, our archived copy
// (sha256 + fetched_at + an actual link to it), the filing, and the regime
// it was filed under.
export function ProvenanceBlock({ provenance }: { provenance: Provenance }) {
  const { filing, raw_document, regime } = provenance;
  const archivedCopyUrl = `${apiBaseUrl()}/v1/filings/${encodeURIComponent(filing.id)}/document`;
  return (
    <section className="provenance" aria-label="Provenance">
      <h2>Provenance</h2>
      <dl className="provenance-grid">
        <dt>Official source</dt>
        <dd>
          {raw_document.source_url ? (
            <a href={raw_document.source_url} rel="noopener noreferrer">
              {raw_document.source_url}
            </a>
          ) : (
            <span className="muted">Source URL not recorded for this document</span>
          )}
        </dd>

        <dt>Archived copy</dt>
        <dd>
          fetched {formatDateTime(raw_document.fetched_at)}
          <span className="sha" data-testid="sha256">
            sha256:{raw_document.sha256}
          </span>
          {" · "}
          <a href={archivedCopyUrl} target="_blank" rel="noopener noreferrer">
            Open archived copy
          </a>
        </dd>

        <dt>Filing</dt>
        <dd>
          {filing.external_id ? `#${filing.external_id}` : filing.id}
          {filing.filed_date ? ` · filed ${formatDate(filing.filed_date)}` : null}
          {filing.published_at
            ? ` · published ${formatDateTime(filing.published_at)}`
            : null}
        </dd>

        <dt>Regime</dt>
        <dd>
          <Link href={`/jurisdictions/${encodeURIComponent(regime.jurisdiction_id)}`}>
            {regime.body}
          </Link>
          {" · "}
          {regime.regime_type.replaceAll("_", " ")}
          {regime.source_url ? (
            <>
              {" · "}
              <a href={regime.source_url} rel="noopener noreferrer">
                official disclosure site
              </a>
            </>
          ) : null}
        </dd>
      </dl>
    </section>
  );
}
```

- [ ] **Step 4: Implement `BronzeDocument`** — replace
  `apps/web/src/components/reviewer/BronzeDocument.tsx`:

```tsx
import type { Provenance } from "@/lib/api";
import { apiBaseUrl } from "@/lib/api";
import { formatDateTime } from "@/lib/format";

// Right half of the side-by-side (design §7.2): the Bronze document. Embeds
// OUR archived copy (design §7.3's original intent — the gov URL can rot,
// change, or, for Brazil, point at a nationwide bulk file instead of
// anything politician-specific).
export function BronzeDocument({ provenance }: { provenance: Provenance }) {
  const { raw_document, filing } = provenance;
  const archivedCopyUrl = `${apiBaseUrl()}/v1/filings/${encodeURIComponent(filing.id)}/document`;
  return (
    <section className="bronze-doc" aria-label="Source document">
      <h2>Source document</h2>
      <iframe
        className="doc-frame"
        src={archivedCopyUrl}
        title="Archived source document"
      />
      <p className="doc-fallback">
        If the document does not render,{" "}
        <a href={archivedCopyUrl} rel="noopener noreferrer">
          open the archived document directly
        </a>
        .
      </p>
      <p className="doc-integrity">
        Archived copy fetched {formatDateTime(raw_document.fetched_at)}
        <span className="sha" data-testid="bronze-sha256">
          sha256:{raw_document.sha256}
        </span>
      </p>
    </section>
  );
}
```

  Note: this drops the previous "no source_url recorded" branch, since the archived-copy
  link no longer depends on `source_url` at all (it is keyed by `filing.id`, always
  present). If a reviewer still needs to see the ORIGINAL gov URL for comparison, that is
  covered by `ProvenanceBlock` rendered alongside it in the same reviewer split view
  (`apps/web/src/app/(reviewer)/review/[id]/page.tsx`) — do not re-add gov-URL branching
  here.

- [ ] **Step 5: Run to verify green**

Run: `pnpm --filter web test -- ProvenanceBlock BronzeDocument`
Expected: PASS.

- [ ] **Step 6: Full web checks**

Run: `pnpm --filter web lint && pnpm --filter web typecheck && pnpm --filter web test`
Expected: all green. If any OTHER test file references the old `BronzeDocument` "Official
source document" title text or the old no-source-url branch, update it to match (grep
`apps/web` for `"Official source document"` and `open the official PDF directly` first).

- [ ] **Step 7: Commit**

```bash
git add apps/web/src/components/ProvenanceBlock.tsx apps/web/src/components/ProvenanceBlock.test.tsx \
        apps/web/src/components/reviewer/BronzeDocument.tsx apps/web/src/components/reviewer/BronzeDocument.test.tsx
git commit -m "feat(web): link/embed our own archived filing document instead of the government's URL"
```

---

### Task 6: End-to-end verification

**Files:**
- Modify: `apps/web/e2e/record.spec.ts`

**Interfaces:**
- Consumes: `seededRecords()`/`apiGet()` helpers already in `apps/web/e2e/api.ts` (used by
  the existing spec).

**Context:** Confirms the whole path works against a real running stack: politician
timeline → grouped "View filing" link → the Rust API actually serving bytes. This is the
one check that exercises Tasks 1-5 together, not any single layer in isolation.

- [ ] **Step 1: Add a new Playwright test** — append to `apps/web/e2e/record.spec.ts`:

```ts
test("record page's archived-copy link actually serves the document", async ({
  page,
  request,
}) => {
  const records = await seededRecords();
  const withValue = records.find((record) => record.value != null) ?? records[0];
  expect(withValue).toBeTruthy();
  if (!withValue) return;
  const detail = await apiGet<RecordDetail>(`/v1/records/${withValue.id}`);

  await page.goto(`/r/${detail.record.id}`);

  const archivedLink = page.getByRole("link", { name: "Open archived copy" });
  const href = await archivedLink.getAttribute("href");
  expect(href).toBeTruthy();
  if (!href) return;

  const response = await request.get(href);
  expect(response.ok()).toBeTruthy();
  expect(response.headers()["content-type"]).toBeTruthy();
});
```

- [ ] **Step 2: Run to verify green** (against the real local stack — this is not a unit
  test double)

Run: `pnpm --filter web e2e -- record.spec.ts`
Expected: PASS. This requires the local API + web dev servers and a seeded local Postgres
(per `docs/runbooks/dev-host-windows.md`) to actually be running — follow whatever bootstrap
the existing `record.spec.ts` tests already rely on (check `apps/web/playwright.config.ts`
for `webServer`/global setup before assuming anything needs to be started manually).

- [ ] **Step 3: Full repo verification** (everything, one more time, end to end)

Run:
```bash
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --workspace
pnpm --filter web lint && pnpm --filter web typecheck && pnpm --filter web test && pnpm --filter web e2e
```
Expected: all green.

- [ ] **Step 4: Manual spot-check across jurisdictions** — against the local dev DB (real
  backfilled BR + US-House data per goals 082/092): open at least one US-House politician
  and one Brazilian politician's page in a browser, click "View filing" on each, and
  confirm (a) the US-House one opens a real PDF, (b) the Brazilian one opens JSON and shows
  the "reconstructed per-candidate..." caption on the timeline. Note the result in the task
  report — this is the one step no automated test fully covers (visually confirming the
  browser renders each content type reasonably).

- [ ] **Step 5: Commit**

```bash
git add apps/web/e2e/record.spec.ts
git commit -m "test(e2e): verify the archived-copy filing document link actually serves bytes"
```
