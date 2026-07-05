//! [`JurisdictionAdapter`] for the `canada_ciec` regime: discover (role-scoped
//! windowed `/cards` sweeps, regime doc §2.3), fetch (details GET → Bronze by
//! sha256), parse (html5ever grammar A/B/C), normalize (Silver → Gold).
//!
//! LIVE-PATH SCOPE (regime doc §2.3–§2.4; recorded follow-ups): `discover`
//! emits every in-scope-role declaration id it finds in the `/cards` fragment
//! and defers (a) persistent high-water-mark windowing, (b) client-side
//! declaration-type filtering by the card badge, and (c) the clientId→roster
//! politician resolution to the runner binding. Out-of-scope declaration types
//! fail closed at `parse` (a new/unswept type is a rules change, invariant 6).
//! Conformance and e2e never touch the network.

use std::collections::HashSet;
use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use scraper::{Html, Selector};

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{
    FilingRef, JurisdictionAdapter, PolitenessCfg, RawDocRef, RegimeRef, RunCtx, StagingRow,
};

use crate::{normalize, parse};

const BASE: &str = "https://ciec-ccie.parl.gc.ca";
const CARDS_URL: &str = "https://ciec-ccie.parl.gc.ca/en/public-registry/cards";
const DETAILS_PATH: &str = "/en/public-registry/Details?declarationId=";

/// The three politician roles in v1 scope (regime doc §2.4 role legend).
const IN_SCOPE_ROLES: &[&str] = &[
    "cac94a19-d04e-e111-b8ea-00265535a320", // Members of Parliament
    "c8c94a19-d04e-e111-b8ea-00265535a320", // Ministers and Ministers of State
    "d2c94a19-d04e-e111-b8ea-00265535a320", // Parliamentary Secretaries
];

/// Discovery lookback when no persistent high-water mark is bound (regime doc
/// §2.3: real runs sweep `disclosureFrom = hwm − 30d`; the hwm is runner
/// machinery — this bounded window is the standalone default).
const DISCOVER_WINDOW_DAYS: i64 = 45;

/// Defensive page ceiling per role sweep (30 cards/page; a full registry is
/// ~280 pages — the windowed sweep stays far below this).
const MAX_PAGES: u32 = 400;

/// Conformance-mode declarationId threading (fixtures `MANIFEST.json`): the
/// details page does not print its own id on families A/B and conformance runs
/// with `pool: None`, so the pinned fixture sha256s map to their declarationIds
/// here. Production resolves from `raw_document.source_url`.
const CONFORMANCE_DECLARATIONS: &[(&str, &str)] = &[
    (
        "4531a973b004a2cbcaf68ebca9df849991614a15fe7fedf2270391bf6ff2a408",
        "30c94327-3108-f111-81a2-001dd8b72449",
    ),
    (
        "c3e9df01f2d1e5c3aa68f5096005ab3853c876e80ac4b31adfe8105be392b61a",
        "a4542986-719d-f011-819d-001dd8b72449",
    ),
    (
        "03061e491fc555f323cb8d928fc9de18a1a0b38a7750fba2f1ae82ee854dcd7a",
        "39e5bbfe-5a8e-f011-819c-001dd8b72449",
    ),
    (
        "e631c24d51957d11b9bf2b03806c7771e7c793ea133f3c68b0493fe1c74b1cb4",
        "e882485d-719d-f011-819d-001dd8b72449",
    ),
    (
        "c95a66fa36c59ee06390c5ea0e45fc231bd54136f1fc3eb1c1c67115f2681485",
        "877aeea7-e1b1-4348-bd5d-808c7758fb22",
    ),
    (
        "9eb65e0e239169232e4bef76a924c5d731183af499895d7630c869cbbfa60df2",
        "c7bf3da3-9669-f111-81a9-001dd8b72449",
    ),
    (
        "2b95fa9a9e1f133446317ff8c53ffe02543af539e47239e45935beaa2be2e762",
        "3f544e69-d268-f111-81a9-001dd8b72449",
    ),
];

/// The Canada CIEC public-registry adapter (ten in-scope declaration types,
/// three politician roles — v1 scope, regime doc §3.2).
#[derive(Debug, Default)]
pub struct CanadaCiecAdapter;

#[async_trait]
impl JurisdictionAdapter for CanadaCiecAdapter {
    fn regime(&self) -> RegimeRef {
        RegimeRef {
            code: "canada_ciec",
        }
    }

    fn politeness(&self) -> PolitenessCfg {
        // Regime doc §2.3: concurrency 1 (cfg default), ≥2 s spacing (the SAF's
        // observed cadence was 2.2 s, kept here); identified UA + contact
        // (invariant 10 — the host served 48/48 requests without challenge).
        PolitenessCfg::new(Duration::from_millis(2200), "ssm.leo@outlook.com")
    }

    /// Role-scoped windowed `/cards` sweeps (regime doc §2.3). Emits one
    /// [`FilingRef`] per discovered declarationId (details EN URL). See the
    /// module header for the deferred hwm/type-filter/roster follow-ups.
    async fn discover(&self, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        let since = window_start(ctx.clock.now());
        let mut seen = HashSet::new();
        let mut refs = Vec::new();
        for role in IN_SCOPE_ROLES {
            for page in 1..=MAX_PAGES {
                let url = format!(
                    "{CARDS_URL}?page={page}&affiliationRole={role}\
                     &disclosureFrom={since}&sortBy=declarationDisclosureDate&sortDir=asc"
                );
                let body = ctx.http.get(&url).await?;
                anyhow::ensure!(
                    body.status().is_success(),
                    "cards sweep {url} -> {} — freeze + review (§2.3)",
                    body.status()
                );
                let html = body.text().await.context("reading cards fragment")?;
                let ids = declaration_ids(&html)?;
                if ids.is_empty() {
                    break; // walked past the window for this role
                }
                let mut added = false;
                for id in ids {
                    if seen.insert(id.clone()) {
                        added = true;
                        refs.push(FilingRef {
                            url: format!("{BASE}{DETAILS_PATH}{id}"),
                            external_id: id,
                        });
                    }
                }
                if !added {
                    break; // no new ids on this page — end of this role's window
                }
            }
        }
        Ok(refs)
    }

    /// Fetch: GET the EN details page once, store raw bytes as the Bronze
    /// document (invariant 2). Pages set no ETag/cookies (regime doc §2.1) —
    /// the runner never re-fetches a stored sha.
    async fn fetch(&self, r: &FilingRef, ctx: &RunCtx) -> anyhow::Result<RawDocRef> {
        let response = ctx.http.get(&r.url).await?;
        anyhow::ensure!(
            response.status().is_success(),
            "details GET {} -> {} — freeze + review (§2.3)",
            r.url,
            response.status()
        );
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("reading details body of {}", r.url))?;
        ctx.bronze.put(&bytes)
    }

    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        let bytes = ctx.bronze.get(d)?;
        let html = std::str::from_utf8(&bytes)
            .with_context(|| format!("bronze doc {} is not UTF-8 — freeze", d.sha256))?;
        let declaration_id = resolve_declaration_id(d, ctx).await?;
        let rows = parse::parse_document(html, &declaration_id)
            .with_context(|| format!("parsing details {}", d.sha256))?;
        rows.into_iter()
            .map(|scored| {
                Ok(StagingRow {
                    payload: serde_json::to_value(&scored.row)
                        .context("serializing staging payload")?,
                    confidence: scored.confidence,
                })
            })
            .collect()
    }

    async fn normalize(
        &self,
        rows: &[StagingRow],
        ctx: &RunCtx,
    ) -> anyhow::Result<Vec<GoldCandidate>> {
        normalize::normalize_rows(rows, ctx)
    }
}

/// `disclosureFrom` for the discovery window (`YYYY-MM-DD`, regime doc §2.1).
fn window_start(now: chrono::DateTime<chrono::Utc>) -> String {
    let since = now.date_naive() - chrono::Duration::days(DISCOVER_WINDOW_DAYS);
    since.format("%Y-%m-%d").to_string()
}

/// declarationId threading (regime doc §4). Pool-backed runs resolve from the
/// recorded `raw_document.source_url`; conformance runs (`pool: None`) use the
/// pinned fixture table. Anything unresolvable fails closed — never guessed.
async fn resolve_declaration_id(doc: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<String> {
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
            .and_then(declaration_id_from_href)
            .with_context(|| {
                format!(
                    "declarationId unresolvable from raw_document.source_url for {} — fail closed",
                    doc.sha256
                )
            });
    }
    CONFORMANCE_DECLARATIONS
        .iter()
        .find(|(sha256, _)| *sha256 == doc.sha256)
        .map(|(_, id)| (*id).to_owned())
        .with_context(|| {
            format!(
                "no conformance declarationId for document {} — extend the MANIFEST-pinned table \
                 (never guess)",
                doc.sha256
            )
        })
}

/// All in-scope-shaped declarationId GUIDs linked from a `/cards` fragment,
/// deduplicated in document order.
fn declaration_ids(html: &str) -> anyhow::Result<Vec<String>> {
    let doc = Html::parse_fragment(html);
    let selector = Selector::parse("a[href*=\"declarationId=\"]")
        .map_err(|e| anyhow::anyhow!("cards link selector: {e}"))?;
    let mut seen = HashSet::new();
    let mut ids = Vec::new();
    for anchor in doc.select(&selector) {
        if let Some(id) = anchor
            .value()
            .attr("href")
            .and_then(declaration_id_from_href)
            && seen.insert(id.clone())
        {
            ids.push(id);
        }
    }
    Ok(ids)
}

/// `declarationId={guid}` GUID (lowercase) from a URL/href, shape-checked.
fn declaration_id_from_href(href: &str) -> Option<String> {
    let (_, rest) = href.split_once("declarationId=")?;
    let guid = rest.split(['&', '#']).next()?;
    is_guid(guid).then(|| guid.to_owned())
}

/// True for a lowercase 8-4-4-4-12 hex GUID.
fn is_guid(s: &str) -> bool {
    s.len() == 36
        && s.char_indices().all(|(at, c)| {
            if matches!(at, 8 | 13 | 18 | 23) {
                c == '-'
            } else {
                c.is_ascii_hexdigit() && !c.is_ascii_uppercase()
            }
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn politeness_is_concurrency_one_with_polite_spacing() {
        let cfg = CanadaCiecAdapter.politeness();
        assert_eq!(cfg.concurrency, 1, "invariant 10");
        assert!(cfg.min_interval >= Duration::from_secs(2));
        assert!(cfg.user_agent().contains("ssm.leo@outlook.com"));
    }

    #[test]
    fn window_start_formats_iso() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-07-05T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        assert_eq!(window_start(now), "2026-05-21");
    }

    #[test]
    fn declaration_ids_extract_from_card_hrefs_and_dedup() {
        let html = "\
            <a href=\"/en/public-registry/Details?declarationId=30c94327-3108-f111-81a2-001dd8b72449\">x</a>\
            <a href=\"/en/public-registry/Details?declarationId=30c94327-3108-f111-81a2-001dd8b72449\">dup</a>\
            <a href=\"/en/client?clientId=5b99c2bd-7b2a-f011-8195-001dd8b72449\">not a declaration</a>";
        let ids = declaration_ids(html).unwrap();
        assert_eq!(ids, vec!["30c94327-3108-f111-81a2-001dd8b72449".to_owned()]);
    }

    #[test]
    fn declaration_id_href_shape_checks() {
        assert_eq!(
            declaration_id_from_href(
                "https://ciec-ccie.parl.gc.ca/en/public-registry/Details?declarationId=c7bf3da3-9669-f111-81a9-001dd8b72449"
            ),
            Some("c7bf3da3-9669-f111-81a9-001dd8b72449".to_owned())
        );
        assert_eq!(
            declaration_id_from_href("/en/public-registry/Details?declarationId=NOPE"),
            None
        );
    }
}
