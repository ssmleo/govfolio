//! [`JurisdictionAdapter`] implementation for the `uk_commons_register`
//! regime: discover (two date-windowed sweeps of the documented public
//! interests API, regime doc §2.3), fetch (`GET /Interests/{id}` → Bronze by
//! sha256), parse (pure `serde_json`, §6 — no scraping, no LLM seam), and
//! normalize (Silver → Gold).
//!
//! Runner-binding follow-ups (recorded in the goal file, `us_senate`
//! precedent): the persistent published/updated high-water marks (this
//! adapter uses the fixed 30-day overlap window from "now"), §3.8 check 4
//! (re-emitting a `FilingRef` when `updatedDates` grew between discover and
//! fetch), the version-qualified filing publish + `uk_interest_update_unlinked`
//! review routing (§2.5/§3.7 — conformance doesn't publish), and the `From:`
//! header the regime doc pairs with the identified UA (the shared
//! [`pipeline::adapter::PoliteClient`] carries the contact in the UA itself).
//! Conformance and e2e never touch the network.

use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use serde::Deserialize;

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{
    FilingRef, JurisdictionAdapter, PolitenessCfg, RawDocRef, RegimeRef, RunCtx, StagingRow,
};

use crate::{normalize, parse};

const BASE: &str = "https://interests-api.parliament.uk";

/// `Take` is hard-capped at 20 by the API contract (regime doc §2.2 / E1).
const PAGE_TAKE: u64 = 20;

/// §2.3 sweep window: high-water mark − 30 days. The persistent HWM is
/// runner machinery (follow-up); until it lands the window anchors on "now",
/// which idempotent writes make free.
const DISCOVER_WINDOW_DAYS: i64 = 30;

/// §2.2: a `totalResults` change mid-walk means the window moved under us —
/// restart the sweep. Bounded so a hot window cannot loop forever.
const MAX_SWEEP_RESTARTS: usize = 3;

/// Conformance-mode interest-id threading (fixtures `MANIFEST.json`
/// `id_threading`): §3.8 check 1 needs the REQUESTED id and conformance runs
/// with `pool: None`, so the pinned fixture sha256s map to their interest ids
/// here. Production resolves from `raw_document.source_url`.
const CONFORMANCE_DOCUMENTS: &[(&str, u32)] = &[
    (
        "8b8613ade949e0b718eb0e7a9640d5d67a9d750ac2d62854edadc6a6ba7d5086",
        15475,
    ),
    (
        "402b3712abb121993f32491f257fc2cadc3cfe382803c2ad2e01bdfe1b105e73",
        15923,
    ),
    (
        "e461b7be25dc05be4319011409ca81c99a65014221947d20fae4dac84311dc60",
        15854,
    ),
    (
        "f15d0e13f9a93eb451d951a61062180ae09a5697ec117de906f1b8128183243a",
        15914,
    ),
    (
        "2f50d45ee0c0f70ec5abb1b092ff27551f5db37245508707109e66a0427a79b6",
        15915,
    ),
];

/// The UK House of Commons Register of Members' Financial Interests adapter
/// (categorical interests via the official Register of Interests API).
#[derive(Debug, Default)]
pub struct UkCommonsRegisterAdapter;

/// One `/Interests` list page (`{links[], skip, take, totalResults, items[]}`,
/// regime doc §2.1). Lenient at the JSON level: discovery only needs the id
/// and version; the strict drift gate is the parse stage (§6.1).
#[derive(Debug, Deserialize)]
struct ListPage {
    #[serde(rename = "totalResults")]
    total_results: u64,
    items: Vec<ListItem>,
}

/// The slice of a listed `PublishedInterest` discovery needs (§2.3 step 2).
#[derive(Debug, Deserialize)]
struct ListItem {
    id: u32,
    #[serde(rename = "updatedDates", default)]
    updated_dates: Vec<String>,
}

#[async_trait]
impl JurisdictionAdapter for UkCommonsRegisterAdapter {
    fn regime(&self) -> RegimeRef {
        RegimeRef {
            code: "uk_commons_register",
        }
    }

    fn politeness(&self) -> PolitenessCfg {
        // Regime doc §2.3 / tos_and_politeness: concurrency 1 (cfg default),
        // ≥2 s between requests, identified UA with a reachable contact —
        // served without challenge on this host (24/24 + 5/5 captures).
        PolitenessCfg::new(Duration::from_secs(2), "ssm.leo@outlook.com")
    }

    /// Discovery (regime doc §2.3): two windowed sweeps of `GET /Interests`,
    /// (a) `PublishedFrom` for new interests, (b) `UpdatedFrom` for in-place
    /// updates (§3.7), both `SortOrder=PublishingDateDescending`, paged
    /// `Skip += 20` until `totalResults` is exhausted. NO conditional GETs
    /// exist on this host (no validators served, E24) — the date window IS
    /// the cheap incremental check.
    async fn discover(&self, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        let since = window_start(ctx.clock.now());
        let mut refs = Vec::new();
        for param in ["PublishedFrom", "UpdatedFrom"] {
            for item in self.sweep(ctx, param, &since).await? {
                let filing_ref = filing_ref_from_item(&item)?;
                // The two sweeps overlap by construction; publish-time dedup
                // is (regime_id, external_id), but don't emit duplicates.
                if !refs.contains(&filing_ref) {
                    refs.push(filing_ref);
                }
            }
        }
        Ok(refs)
    }

    /// Fetch: `GET /Interests/{id}` once per new `external_id`, raw response
    /// bytes → Bronze (invariant 2). One Bronze doc = one interest version
    /// (§2.3 step 3).
    async fn fetch(&self, r: &FilingRef, ctx: &RunCtx) -> anyhow::Result<RawDocRef> {
        let response = ctx.http.get(&r.url).await?;
        anyhow::ensure!(
            response.status().is_success(),
            "interest GET {} -> {} — freeze + review, no blind retry (invariant 6)",
            r.url,
            response.status()
        );
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("reading interest body of {}", r.url))?;
        ctx.bronze.put(&bytes)
    }

    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        let bytes = ctx.bronze.get(d)?;
        let requested_id = resolve_interest_id(d, ctx).await?;
        // §6.1: UTF-8 end to end (non-ASCII observed: `Gŵyl`, `£`). A
        // non-UTF-8 body from a JSON API is contract drift — freeze.
        let text = std::str::from_utf8(&bytes)
            .context("interest document is not UTF-8 — contract drift, freeze (§6.1)")?;
        let scored = parse::parse_document(text, requested_id)?;
        Ok(vec![StagingRow {
            payload: serde_json::to_value(&scored.row).context("serializing staging payload")?,
            confidence: scored.confidence,
        }])
    }

    async fn normalize(
        &self,
        rows: &[StagingRow],
        ctx: &RunCtx,
    ) -> anyhow::Result<Vec<GoldCandidate>> {
        normalize::normalize_rows(rows, ctx)
    }
}

impl UkCommonsRegisterAdapter {
    /// One windowed sweep, restarted (bounded) when `totalResults` moves
    /// mid-walk (§2.2 pagination contract).
    async fn sweep(&self, ctx: &RunCtx, param: &str, since: &str) -> anyhow::Result<Vec<ListItem>> {
        'restart: for _attempt in 0..=MAX_SWEEP_RESTARTS {
            let mut items = Vec::new();
            let mut skip: u64 = 0;
            let mut total: Option<u64> = None;
            loop {
                let url = format!(
                    "{BASE}/api/v1/Interests?SortOrder=PublishingDateDescending\
                     &{param}={since}&Take={PAGE_TAKE}&Skip={skip}"
                );
                let response = ctx.http.get(&url).await?;
                anyhow::ensure!(
                    response.status().is_success(),
                    "interest sweep GET {url} -> {}",
                    response.status()
                );
                let body = response
                    .text()
                    .await
                    .with_context(|| format!("reading sweep body of {url}"))?;
                let page: ListPage = serde_json::from_str(&body)
                    .with_context(|| format!("sweep page at {url} is not a list response"))?;
                if let Some(total) = total {
                    if total != page.total_results {
                        // The window moved under us; the re-walk is free
                        // (idempotent writes, §2.2).
                        continue 'restart;
                    }
                } else {
                    total = Some(page.total_results);
                }
                items.extend(page.items);
                skip += PAGE_TAKE;
                if skip >= page.total_results {
                    return Ok(items);
                }
            }
        }
        anyhow::bail!(
            "{param} sweep totalResults kept moving across {MAX_SWEEP_RESTARTS} restarts — \
             freeze + review (§2.2)"
        )
    }
}

/// `PublishedFrom`/`UpdatedFrom` value for the discovery window (`YYYY-MM-DD`).
fn window_start(now: chrono::DateTime<chrono::Utc>) -> String {
    let since = now.date_naive() - chrono::Duration::days(DISCOVER_WINDOW_DAYS);
    since.format("%Y-%m-%d").to_string()
}

/// One listed interest → [`FilingRef`] (§2.5): `external_id = "{id}@{version}"`
/// where `version = updatedDates.length` — an in-place update changes the
/// version and arrives as a new filing.
fn filing_ref_from_item(item: &ListItem) -> anyhow::Result<FilingRef> {
    let version = u32::try_from(item.updated_dates.len()).context("updatedDates overflow")?;
    Ok(FilingRef {
        external_id: format!("{}@{version}", item.id),
        url: format!("{BASE}/api/v1/Interests/{}", item.id),
    })
}

/// Interest-id threading (§3.8 check 1). Pool-backed runs resolve from the
/// recorded `raw_document.source_url`; conformance runs (`pool: None`) use
/// the pinned fixture table. Anything unresolvable fails closed — never
/// guessed.
async fn resolve_interest_id(doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<u32> {
    if let Some(pool) = &ctx.pool {
        let source_url: Option<Option<String>> =
            sqlx::query_scalar("select source_url from raw_document where sha256 = $1")
                .bind(&doc.sha256)
                .fetch_optional(pool)
                .await
                .context("reading raw_document.source_url")?;
        return source_url
            .flatten()
            .as_deref()
            .and_then(id_from_interest_url)
            .with_context(|| {
                format!(
                    "interest id unresolvable from raw_document.source_url for {} — \
                     fail closed (§3.8 check 1)",
                    doc.sha256
                )
            });
    }
    CONFORMANCE_DOCUMENTS
        .iter()
        .find(|(sha256, _)| *sha256 == doc.sha256)
        .map(|(_, id)| *id)
        .with_context(|| {
            format!(
                "no conformance interest id for document {} — extend the MANIFEST-pinned \
                 table with the fixture (never guess)",
                doc.sha256
            )
        })
}

/// `…/api/v1/Interests/{id}` → the numeric id, shape-checked.
fn id_from_interest_url(url: &str) -> Option<u32> {
    let (_, rest) = url.split_once("/api/v1/Interests/")?;
    let id = rest.trim_end_matches('/');
    (!id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        .then(|| id.parse().ok())
        .flatten()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn politeness_is_concurrency_one_with_two_second_spacing() {
        let cfg = UkCommonsRegisterAdapter.politeness();
        assert_eq!(cfg.concurrency, 1, "invariant 10");
        assert_eq!(cfg.min_interval, Duration::from_secs(2));
        assert!(cfg.user_agent().contains("ssm.leo@outlook.com"));
    }

    #[test]
    fn listed_interests_become_version_qualified_filing_refs() {
        let original = ListItem {
            id: 15475,
            updated_dates: vec![],
        };
        let filing_ref = filing_ref_from_item(&original).unwrap();
        assert_eq!(filing_ref.external_id, "15475@0");
        assert_eq!(
            filing_ref.url,
            "https://interests-api.parliament.uk/api/v1/Interests/15475"
        );
        // An in-place update bumps the version and arrives as a NEW filing.
        let updated = ListItem {
            id: 2696,
            updated_dates: vec!["2024-07-26".to_owned(), "2026-06-18".to_owned()],
        };
        assert_eq!(
            filing_ref_from_item(&updated).unwrap().external_id,
            "2696@2"
        );
    }

    #[test]
    fn interest_url_ids_are_shape_checked() {
        assert_eq!(
            id_from_interest_url("https://interests-api.parliament.uk/api/v1/Interests/15475"),
            Some(15475)
        );
        assert_eq!(
            id_from_interest_url("https://interests-api.parliament.uk/api/v1/Interests/15475/"),
            Some(15475),
            "trailing slash tolerated"
        );
        assert_eq!(
            id_from_interest_url("https://interests-api.parliament.uk/api/v1/Interests/abc"),
            None
        );
        assert_eq!(id_from_interest_url("https://example.com/other"), None);
    }

    #[test]
    fn discovery_window_formats_as_iso_date() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-05T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(window_start(now), "2026-06-05");
    }

    #[test]
    fn conformance_table_pins_all_five_fixtures() {
        assert_eq!(CONFORMANCE_DOCUMENTS.len(), 5);
        for (sha256, id) in CONFORMANCE_DOCUMENTS {
            assert_eq!(sha256.len(), 64, "sha256 hex for interest {id}");
        }
    }
}
