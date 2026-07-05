//! [`JurisdictionAdapter`] implementation for the `us_senate` PTR regime:
//! discover (agreement dance + date-windowed `DataTables` POST, regime doc
//! §2.1–§2.3), fetch (view GET → Bronze by sha256), parse (html5ever table
//! parse + LLM seam stub), normalize (Silver → Gold).
//!
//! LIVE-PATH LIMITATION (regime doc §2.5, load-bearing): the eFD host runs an
//! Akamai bot manager that mechanically 403s non-browser TLS fingerprints on
//! HTML GET paths — plain `reqwest` must be assumed 403-bound for view-page
//! `fetch` even with a valid session (POST endpoints passed from non-browser
//! stacks in every probe). The protocol here is implemented faithfully;
//! the browser-grade fetch engine (`wreq`-style impersonation vs a
//! headless-browser sidecar) is a recorded follow-up work item — the flip is
//! SAF-first per §2.5, and no fingerprint-evasion escalation happens beyond
//! the documented client. Conformance and e2e never touch the network.

use std::sync::Mutex;
use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use scraper::{Html, Selector};
use serde::Deserialize;

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{
    FilingRef, JurisdictionAdapter, PolitenessCfg, RawDocRef, RegimeRef, RunCtx, StagingRow,
};

use crate::extractor::{Extractor as _, StubExtractor};
use crate::{normalize, parse};

const BASE: &str = "https://efdsearch.senate.gov";
const SEARCH_URL: &str = "https://efdsearch.senate.gov/search/";
const HOME_URL: &str = "https://efdsearch.senate.gov/search/home/";
const DATA_URL: &str = "https://efdsearch.senate.gov/search/report/data/";

/// `DataTables` page size (regime doc §2.3: length 100, ≥2 s between pages —
/// the spacing rides the politeness throttle).
const PAGE_LENGTH: u64 = 100;

/// Discovery window: the statutory 45-day maximum filing lag plus the §2.3
/// 7-day overlap (idempotent writes make the overlap free). The persistent
/// high-water mark is runner machinery (follow-up with the runner binding).
const DISCOVER_WINDOW_DAYS: i64 = 45 + 7;

/// Conformance-mode report-UUID threading (fixtures `MANIFEST.json`
/// `report_uuid_threading`): the view page never prints its own UUID and
/// conformance runs with `pool: None`, so the pinned fixture sha256s map to
/// their UUIDs here. Production resolves from `raw_document.source_url`.
const CONFORMANCE_REPORTS: &[(&str, &str)] = &[
    (
        "bd2d1df73361210360e1c4b4c0fdb72d1b646497352975a4b2dcb562aaaea80e",
        "4b69867f-0376-4526-93f2-cd556b1155c9",
    ),
    (
        "abbbdd79d5bc33ff07f880398cd9c6ee985df3b45d17ef22a83649cd2e5a6ef2",
        "4aa0094d-d9da-4a05-aa13-6d9f5d376105",
    ),
    (
        "9c53a91cce5db4e201889fb580df5e4d43db4df9157fefbece42e7a1019dd5e7",
        "727b4eb6-d8c7-4792-aa5b-c651c2d72f9c",
    ),
    (
        "b1a9c78d5b059909f5944a8dc5d5fd6b19851f3b43ad0c9d31faf9b27d9fb487",
        "a9754ff5-901a-4877-b7be-a647bd361c52",
    ),
];

/// The US Senate eFD PTR adapter (report type `11`, senators only — v1 scope).
#[derive(Debug, Default)]
pub struct UsSenateAdapter {
    /// `csrftoken` cookie value captured during the agreement dance — the
    /// `X-CSRFToken` header of every search POST (regime doc §2.2).
    csrf_token: Mutex<Option<String>>,
    /// LLM seam for documents the HTML path cannot handle (v1 stub, §6.3).
    extractor: StubExtractor,
}

/// One `DataTables` response page (regime doc §2.2 contract).
#[derive(Debug, Deserialize)]
struct SearchPage {
    #[serde(rename = "recordsFiltered")]
    records_filtered: u64,
    data: Vec<Vec<serde_json::Value>>,
}

#[async_trait]
impl JurisdictionAdapter for UsSenateAdapter {
    fn regime(&self) -> RegimeRef {
        RegimeRef { code: "us_senate" }
    }

    fn politeness(&self) -> PolitenessCfg {
        // Regime doc §2.3/§2.5: concurrency 1 (cfg default), ≥2 s between
        // requests. NOTE on identification: eFD's bot manager 403s identified
        // UA strings — the production fetch engine must carry the contact via
        // the standard `From:` header on a stock-UA browser-grade client
        // (documented politeness deviation with rationale, §2.5). The
        // PolitenessCfg contact keeps invariant 10's identified-UA default
        // for every other surface.
        PolitenessCfg::new(Duration::from_secs(2), "ssm.leo@outlook.com")
    }

    /// Discovery (regime doc §2.1–§2.3): agreement dance if needed, then the
    /// date-windowed `DataTables` POST, paged until `recordsFiltered` is
    /// exhausted. Discovery is POST-only — no conditional-GET semantics exist
    /// (§2.3); the date window IS the cheap incremental check.
    async fn discover(&self, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        let mut csrf = self.ensure_session(ctx).await?;
        let since = window_start(ctx.clock.now());
        let mut refs = Vec::new();
        let mut start: u64 = 0;
        let mut draw: u64 = 1;
        let mut redanced = false;
        loop {
            let body = self.post_search(ctx, &csrf, &since, draw, start).await?;
            let page: SearchPage = match serde_json::from_str(&body) {
                Ok(page) => page,
                Err(_) if !redanced => {
                    // §2.1 re-dance rule: a non-JSON answer means the session
                    // died (302 back to the agreement page). Dance once more;
                    // a second failure is a blocking incident (fail closed).
                    redanced = true;
                    csrf = self.dance(ctx).await?;
                    continue;
                }
                Err(e) => anyhow::bail!(
                    "search POST answered non-DataTables JSON after a fresh agreement dance — \
                     blocking incident, freeze + review (§2.1): {e}"
                ),
            };
            for row in &page.data {
                refs.push(filing_ref_from_row(row)?);
            }
            start += PAGE_LENGTH;
            draw += 1;
            if start >= page.records_filtered {
                break;
            }
        }
        Ok(refs)
    }

    /// Fetch: GET the view page once, store raw bytes as the Bronze document
    /// (invariant 2). Report pages are immutable (§2.3/§7 pinning evidence) —
    /// the runner never re-fetches a stored sha. See the module header for
    /// the §2.5 TLS-fingerprint limitation on this path.
    async fn fetch(&self, r: &FilingRef, ctx: &RunCtx) -> anyhow::Result<RawDocRef> {
        let response = ctx.http.get(&r.url).await?;
        anyhow::ensure!(
            response.status().is_success(),
            "view GET {} -> {} (a 403 here is the documented §2.5 bot-manager gate on \
             non-browser TLS fingerprints — freeze, do not retry; browser-grade fetch \
             engine is the recorded follow-up work item)",
            r.url,
            response.status()
        );
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("reading view body of {}", r.url))?;
        ctx.bronze.put(&bytes)
    }

    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        let bytes = ctx.bronze.get(d)?;
        let report_uuid = resolve_report_uuid(d, ctx).await?;
        let Ok(html) = std::str::from_utf8(&bytes) else {
            // Non-UTF-8 bytes (e.g. a paper GIF): LLM seam (§6.3a).
            return self.extractor.extract(d, ctx).await;
        };
        match parse::parse_document(html, &report_uuid) {
            Ok(rows) => rows
                .into_iter()
                .map(|scored| {
                    Ok(StagingRow {
                        payload: serde_json::to_value(&scored.row)
                            .context("serializing staging payload")?,
                        confidence: scored.confidence,
                    })
                })
                .collect(),
            Err(cause) => {
                // Deterministic reject (§3.7) or paper wrapper page (§3.8):
                // LLM seam (§6.3b). The v1 stub freezes; the reject reason
                // rides along for the review trail.
                self.extractor.extract(d, ctx).await.with_context(|| {
                    format!("deterministic parse rejected {}: {cause:#}", d.sha256)
                })
            }
        }
    }

    async fn normalize(
        &self,
        rows: &[StagingRow],
        ctx: &RunCtx,
    ) -> anyhow::Result<Vec<GoldCandidate>> {
        normalize::normalize_rows(rows, ctx)
    }
}

impl UsSenateAdapter {
    /// Cached csrftoken, or a fresh agreement dance (§2.1).
    async fn ensure_session(&self, ctx: &RunCtx) -> anyhow::Result<String> {
        let cached = self
            .csrf_token
            .lock()
            .map_err(|_| anyhow::anyhow!("csrf token lock poisoned"))?
            .clone();
        match cached {
            Some(token) => Ok(token),
            None => self.dance(ctx).await,
        }
    }

    /// The §2.1 session dance: GET `/search/` (302 → agreement page when the
    /// session lacks the flag), POST the one-checkbox agreement, keep the
    /// `csrftoken` cookie value for later `X-CSRFToken` headers. Acceptance
    /// is stored server-side against the opaque session cookie (the polite
    /// client's cookie store carries both).
    async fn dance(&self, ctx: &RunCtx) -> anyhow::Result<String> {
        let response = ctx.http.get(SEARCH_URL).await?;
        anyhow::ensure!(
            response.status().is_success(),
            "session dance GET {SEARCH_URL} -> {}",
            response.status()
        );
        let csrf_cookie = response
            .cookies()
            .find(|cookie| cookie.name() == "csrftoken")
            .map(|cookie| cookie.value().to_owned());
        let body = response
            .text()
            .await
            .context("reading session dance body")?;
        if body.contains("agreement_form") {
            let form_token = agreement_form_token(&body)?;
            let accepted = ctx
                .http
                .post_form(
                    HOME_URL,
                    &[
                        ("prohibition_agreement", "1"),
                        ("csrfmiddlewaretoken", &form_token),
                    ],
                    &[("Referer", HOME_URL)],
                )
                .await?;
            anyhow::ensure!(
                accepted.status().is_success(),
                "agreement POST {HOME_URL} -> {} — blocking incident, freeze + review (§2.1)",
                accepted.status()
            );
        }
        let token = csrf_cookie
            .context("no csrftoken cookie after the session dance — fail closed (§2.1)")?;
        *self
            .csrf_token
            .lock()
            .map_err(|_| anyhow::anyhow!("csrf token lock poisoned"))? = Some(token.clone());
        Ok(token)
    }

    /// One `DataTables` POST (§2.2 contract, captured verbatim from the
    /// search page's own JS): PTRs (`[11]`), senators (`[1]` — v1 scope),
    /// ordered by date submitted ascending.
    async fn post_search(
        &self,
        ctx: &RunCtx,
        csrf: &str,
        since: &str,
        draw: u64,
        start: u64,
    ) -> anyhow::Result<String> {
        let draw_s = draw.to_string();
        let start_s = start.to_string();
        let length_s = PAGE_LENGTH.to_string();
        let form: &[(&str, &str)] = &[
            ("draw", &draw_s),
            ("start", &start_s),
            ("length", &length_s),
            ("report_types", "[11]"),
            ("filer_types", "[1]"),
            ("submitted_start_date", since),
            ("submitted_end_date", ""),
            ("candidate_state", ""),
            ("senator_state", ""),
            ("office_id", ""),
            ("first_name", ""),
            ("last_name", ""),
            ("order[0][column]", "4"),
            ("order[0][dir]", "asc"),
            ("columns[0][data]", "0"),
            ("columns[1][data]", "1"),
            ("columns[2][data]", "2"),
            ("columns[3][data]", "3"),
            ("columns[4][data]", "4"),
        ];
        let response = ctx
            .http
            .post_form(
                DATA_URL,
                form,
                &[
                    ("X-CSRFToken", csrf),
                    ("Referer", SEARCH_URL),
                    ("X-Requested-With", "XMLHttpRequest"),
                ],
            )
            .await?;
        anyhow::ensure!(
            response.status().is_success(),
            "search POST {DATA_URL} -> {}",
            response.status()
        );
        response.text().await.context("reading search POST body")
    }
}

/// `submitted_start_date` for the discovery window (`MM/DD/YYYY HH:MM:SS`,
/// §2.2 field format).
fn window_start(now: chrono::DateTime<chrono::Utc>) -> String {
    let since = now.date_naive() - chrono::Duration::days(DISCOVER_WINDOW_DAYS);
    since.format("%m/%d/%Y 00:00:00").to_string()
}

/// The hidden Django form token of the agreement page (§2.1 step 2 — the
/// 64-char masked token, distinct from the `csrftoken` cookie).
fn agreement_form_token(html: &str) -> anyhow::Result<String> {
    let doc = Html::parse_document(html);
    let sel = Selector::parse(r#"form#agreement_form input[name="csrfmiddlewaretoken"]"#)
        .map_err(|e| anyhow::anyhow!("agreement token selector: {e}"))?;
    let token = doc
        .select(&sel)
        .next()
        .and_then(|input| input.value().attr("value"))
        .context("agreement page carries no csrfmiddlewaretoken — fail closed (§2.1)")?;
    anyhow::ensure!(!token.is_empty(), "empty csrfmiddlewaretoken");
    Ok(token.to_owned())
}

/// One §2.2 listing row → [`FilingRef`]: cell 3 is
/// `<a href="/search/view/(ptr|paper)/<uuid>/" …>title</a>`; the UUID is the
/// only source-native id (`filing.external_id`). Names/title/date cells ride
/// the listing only — filer identity resolves from the document itself
/// (§2.4), so they are not carried here.
fn filing_ref_from_row(row: &[serde_json::Value]) -> anyhow::Result<FilingRef> {
    anyhow::ensure!(
        row.len() == 5,
        "listing row has {} cells, expected 5 (§2.2) — fail closed",
        row.len()
    );
    let link = row[3]
        .as_str()
        .context("listing link cell is not a string — fail closed")?;
    let href = link
        .split_once("href=\"")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once('"'))
        .map(|(href, _)| href)
        .with_context(|| format!("listing link cell {link:?} has no href"))?;
    let uuid = uuid_from_view_path(href)
        .with_context(|| format!("listing href {href:?} is not a view URL — fail closed"))?;
    Ok(FilingRef {
        external_id: uuid,
        url: format!("{BASE}{href}"),
    })
}

/// `…/search/view/(ptr|paper)/{uuid}/` → the UUID, shape-checked.
fn uuid_from_view_path(path: &str) -> Option<String> {
    let (_, rest) = path.split_once("/search/view/")?;
    let mut segments = rest.split('/');
    let kind = segments.next()?;
    if kind != "ptr" && kind != "paper" {
        return None;
    }
    let uuid = segments.next()?;
    let bytes = uuid.as_bytes();
    let shape_ok = bytes.len() == 36
        && uuid.char_indices().all(|(at, c)| {
            if matches!(at, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit() && !c.is_ascii_uppercase()
            }
        });
    shape_ok.then(|| uuid.to_owned())
}

/// Report-UUID threading (regime doc §4: the page never prints it).
/// Pool-backed runs resolve from the recorded `raw_document.source_url`;
/// conformance runs (`pool: None`) use the pinned fixture table. Anything
/// unresolvable fails closed — never guessed.
async fn resolve_report_uuid(doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<String> {
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
            .and_then(uuid_from_view_path)
            .with_context(|| {
                format!(
                    "report_uuid unresolvable from raw_document.source_url for {} — \
                     fail closed (the page never prints it)",
                    doc.sha256
                )
            });
    }
    CONFORMANCE_REPORTS
        .iter()
        .find(|(sha256, _)| *sha256 == doc.sha256)
        .map(|(_, uuid)| (*uuid).to_owned())
        .with_context(|| {
            format!(
                "no conformance report_uuid for document {} — extend the MANIFEST-pinned \
                 table with the fixture (never guess)",
                doc.sha256
            )
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn politeness_is_concurrency_one_with_two_second_spacing() {
        let cfg = UsSenateAdapter::default().politeness();
        assert_eq!(cfg.concurrency, 1, "invariant 10");
        assert_eq!(cfg.min_interval, Duration::from_secs(2));
        assert!(cfg.user_agent().contains("ssm.leo@outlook.com"));
    }

    #[test]
    fn agreement_token_extracts_from_the_form() {
        let html = r#"<form id="agreement_form" action="" method="POST">
            <input type="hidden" name="csrfmiddlewaretoken" value="AbCdEf012345">
            <input type="checkbox" name="prohibition_agreement" value="1">
        </form>"#;
        assert_eq!(agreement_form_token(html).unwrap(), "AbCdEf012345");
        assert!(agreement_form_token("<p>no form</p>").is_err());
    }

    #[test]
    fn listing_rows_become_filing_refs() {
        // Shape verbatim from E3 (§2.2), including the dirty name cells.
        let row = vec![
            serde_json::json!("Gary C"),
            serde_json::json!("Peters"),
            serde_json::json!("Peters, Gary (Senator)"),
            serde_json::json!(
                "<a href=\"/search/view/ptr/4b69867f-0376-4526-93f2-cd556b1155c9/\" \
                 target=\"_blank\">Periodic Transaction Report for 06/12/2026</a>"
            ),
            serde_json::json!("06/12/2026"),
        ];
        let filing_ref = filing_ref_from_row(&row).unwrap();
        assert_eq!(
            filing_ref.external_id,
            "4b69867f-0376-4526-93f2-cd556b1155c9"
        );
        assert_eq!(
            filing_ref.url,
            "https://efdsearch.senate.gov/search/view/ptr/4b69867f-0376-4526-93f2-cd556b1155c9/"
        );
        // Paper rows carry /paper/ hrefs and still discover (LLM seam later).
        let mut paper = row.clone();
        paper[3] = serde_json::json!(
            "<a href=\"/search/view/paper/a0d25e8f-fe54-4328-a7ea-504da008742b/\" \
             target=\"_blank\">Periodic Transaction Report for 06/05/2026</a>"
        );
        assert_eq!(
            filing_ref_from_row(&paper).unwrap().external_id,
            "a0d25e8f-fe54-4328-a7ea-504da008742b"
        );
        // Anything else fails closed.
        let mut bad = row;
        bad[3] = serde_json::json!("<a href=\"/search/view/annual/x/\">nope</a>");
        assert!(filing_ref_from_row(&bad).is_err());
    }

    #[test]
    fn view_path_uuids_are_shape_checked() {
        assert_eq!(
            uuid_from_view_path("/search/view/ptr/4b69867f-0376-4526-93f2-cd556b1155c9/"),
            Some("4b69867f-0376-4526-93f2-cd556b1155c9".to_owned())
        );
        assert_eq!(
            uuid_from_view_path(
                "https://efdsearch.senate.gov/search/view/ptr/4b69867f-0376-4526-93f2-cd556b1155c9/"
            ),
            Some("4b69867f-0376-4526-93f2-cd556b1155c9".to_owned()),
            "absolute source_url form resolves too"
        );
        assert_eq!(
            uuid_from_view_path("/search/view/ptr/NOT-A-UUID/"),
            None,
            "shape check"
        );
        assert_eq!(uuid_from_view_path("/search/view/annual/x/"), None);
    }

    #[test]
    fn discovery_window_formats_per_the_search_contract() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-05T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(window_start(now), "05/14/2026 00:00:00");
    }
}
