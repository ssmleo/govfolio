//! [`JurisdictionAdapter`] for the `australia_register` regime: discover (parse
//! the HTML register index → per-member PDF links, regime doc §2.3), fetch
//! (member PDF → Bronze by sha256), parse (LLM-vision seam → Silver, offline
//! cache), normalize (Silver → Gold).
//!
//! LIVE-PATH SCOPE (regime doc §2.3, §6.5; recorded follow-ups): the host
//! `www.aph.gov.au` is fronted by an Azure Front Door WAF that 403-blocks every
//! non-browser client — the HTML index AND the media PDFs alike (regime doc
//! §2.3, E11). The polite production fetch therefore MUST run through a
//! browser-engine fetch seam (headless Chromium, the `us_senate` §2.5 precedent);
//! this leg implements the fetch PROTOCOL (polite GET + Bronze) but keeps the
//! plain `reqwest` transport, so a live fetch surfaces the WAF 403 as a
//! fail-closed error rather than evading it. Wiring the browser seam and the
//! live vision transcription is the follow-up. Conformance and e2e never touch
//! the network — `parse` reads the offline extraction cache.

use std::collections::HashSet;
use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;
use scraper::{Html, Selector};

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{
    FilingRef, JurisdictionAdapter, PolitenessCfg, RawDocRef, RegimeRef, RunCtx, StagingRow,
};

use crate::extractor::{Extractor as _, LlmExtractor};
use crate::normalize;

/// The register landing page (server-rendered alphabetical member table).
const INDEX_URL: &str = "https://www.aph.gov.au/Senators_and_Members/Members/Register";
/// Absolute-URL base for the site-relative `/-/media/...` PDF links.
const BASE: &str = "https://www.aph.gov.au";

/// The Australia House of Representatives Register of Members' Interests
/// adapter (per-member scanned PDFs; two record types in one compound doc —
/// regime doc §3.4). Senate register is a separate regime (§2.6).
#[derive(Debug, Default)]
pub struct AustraliaRegisterAdapter {
    /// LLM-vision seam (design §5.3): the whole-regime default (§6).
    extractor: LlmExtractor,
}

#[async_trait]
impl JurisdictionAdapter for AustraliaRegisterAdapter {
    fn regime(&self) -> RegimeRef {
        RegimeRef {
            code: "australia_register",
        }
    }

    fn politeness(&self) -> PolitenessCfg {
        // Regime doc §2.3: concurrency 1 (cfg default), >= 2.5 s spacing,
        // identified UA + contact (invariant 10). robots.txt is unretrievable
        // (WAF), so self-imposed politeness governs; the browser-engine seam
        // carries this UA + a `From:` header.
        PolitenessCfg::new(Duration::from_millis(2500), "ssm.leo@outlook.com")
    }

    /// Parse the register index into one [`FilingRef`] per member PDF (regime
    /// doc §2.3). Requires the browser-engine seam in production (§2.3 WAF).
    async fn discover(&self, ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        let response = ctx.http.get(INDEX_URL).await?;
        anyhow::ensure!(
            response.status().is_success(),
            "register index GET {INDEX_URL} -> {} — the Azure WAF blocks non-browser \
             clients (§2.3); freeze + review, never evade",
            response.status()
        );
        let html = response
            .text()
            .await
            .context("reading register index body")?;
        Ok(register_pdf_links(&html))
    }

    /// Fetch: GET the member PDF once, store raw bytes as the Bronze document
    /// (invariant 2). Media PDFs carry `Last-Modified` → conditional GET is the
    /// incremental primitive (§2.3); the plain GET here fails closed on the WAF
    /// 403 pending the browser-engine seam.
    async fn fetch(&self, r: &FilingRef, ctx: &RunCtx) -> anyhow::Result<RawDocRef> {
        let response = ctx.http.get(&r.url).await?;
        anyhow::ensure!(
            response.status().is_success(),
            "member PDF GET {} -> {} — the Azure WAF blocks non-browser clients \
             (§2.3); freeze + review, never evade",
            r.url,
            response.status()
        );
        let bytes = response
            .bytes()
            .await
            .with_context(|| format!("reading member PDF body of {}", r.url))?;
        ctx.bronze.put(&bytes)
    }

    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        // Vision-first regime: every document routes through the LLM seam,
        // which reads the offline cache for conformance (§6 step 6).
        self.extractor.extract(d, ctx).await
    }

    async fn normalize(
        &self,
        rows: &[StagingRow],
        ctx: &RunCtx,
    ) -> anyhow::Result<Vec<GoldCandidate>> {
        normalize::normalize_rows(rows, ctx)
    }
}

/// Every per-member register PDF linked from the index, in document order,
/// deduplicated. `external_id` is the filename stem (`{Surname}_{NNP}`) — a
/// document key, not a person id (regime doc §2.2); the `@{version}` binding
/// (media `Last-Modified`, §2.5) is threaded at fetch time (follow-up).
fn register_pdf_links(index_html: &str) -> Vec<FilingRef> {
    let doc = Html::parse_document(index_html);
    let Ok(selector) = Selector::parse("a[href$=\".pdf\"]") else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut refs = Vec::new();
    for anchor in doc.select(&selector) {
        let Some(href) = anchor.value().attr("href") else {
            continue;
        };
        if !href.contains("/Register/") {
            continue; // not a register document link
        }
        let Some(stem) = pdf_filename_stem(href) else {
            continue;
        };
        if seen.insert(stem.clone()) {
            refs.push(FilingRef {
                url: absolutize(href),
                external_id: stem,
            });
        }
    }
    refs
}

/// `.../{Surname}_{NNP}.pdf` → `{Surname}_{NNP}` (the document key, §2.2).
fn pdf_filename_stem(href: &str) -> Option<String> {
    let file = href.split(['?', '#']).next()?.rsplit('/').next()?;
    let stem = file.strip_suffix(".pdf")?;
    (!stem.is_empty()).then(|| stem.to_owned())
}

/// Site-relative href → absolute URL against the register host.
fn absolutize(href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        href.to_owned()
    } else if let Some(rest) = href.strip_prefix('/') {
        format!("{BASE}/{rest}")
    } else {
        format!("{BASE}/{href}")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn politeness_is_concurrency_one_with_polite_spacing() {
        let cfg = AustraliaRegisterAdapter::default().politeness();
        assert_eq!(cfg.concurrency, 1, "invariant 10");
        assert!(cfg.min_interval >= Duration::from_millis(2500));
        assert!(cfg.user_agent().contains("ssm.leo@outlook.com"));
    }

    #[test]
    fn register_links_extract_member_pdfs_and_dedup() {
        let html = "\
            <table>\
            <tr><td>25 May 2026</td><td>Buchholz, Mr Scott, Member for Wright, QLD</td>\
              <td><a href=\"/-/media/03_Senators_and_Members/32_Members/Register/48p/AB/Buchholz_48P.pdf\">PDF</a></td></tr>\
            <tr><td>30 Apr 2026</td><td>Chalmers, Hon James, Member for Rankin, QLD</td>\
              <td><a href=\"/-/media/03_Senators_and_Members/32_Members/Register/48p/CF/Chalmers_48P.pdf\">PDF</a></td></tr>\
            <tr><td>dup</td><td><a href=\"/-/media/03_Senators_and_Members/32_Members/Register/48p/AB/Buchholz_48P.pdf\">again</a></td></tr>\
            <tr><td><a href=\"/-/media/05_About_Parliament/Resolutions/9Oct1984.pdf\">not a register doc</a></td></tr>\
            </table>";
        let refs = register_pdf_links(html);
        assert_eq!(
            refs.len(),
            2,
            "two distinct register PDFs (dup + non-register dropped)"
        );
        assert_eq!(refs[0].external_id, "Buchholz_48P");
        assert_eq!(
            refs[0].url,
            "https://www.aph.gov.au/-/media/03_Senators_and_Members/32_Members/Register/48p/AB/Buchholz_48P.pdf"
        );
        assert_eq!(refs[1].external_id, "Chalmers_48P");
    }

    #[test]
    fn pdf_stem_strips_query_and_suffix() {
        assert_eq!(
            pdf_filename_stem("/x/y/Katter_48P.pdf?v=2"),
            Some("Katter_48P".to_owned())
        );
        assert_eq!(pdf_filename_stem("/x/y/notpdf"), None);
    }
}
