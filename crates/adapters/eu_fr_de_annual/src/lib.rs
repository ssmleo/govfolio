//! EU Parliament DPI + France HATVP DIA + Germany Bundestag — annual private-interest
//! declarations. ONE adapter crate (`eu_fr_de_annual`) registering the single
//! conformance name, with THREE source sub-adapters (`eu`/`fr`/`de`) and THREE
//! `disclosure_regime` rows (`eu_parliament_dpi`, `fr_hatvp_dia`, `de_bundestag`);
//! conformance dispatches by the single adapter name over source-namespaced
//! fixtures `fixtures/{eu,fr,de}_<case>/`. See the authoritative methodology
//! `docs/regimes/eu_fr_de_annual.md` (§0 architecture, §0.1 shared conventions) and
//! the per-source builder notes in `fixtures/MANIFEST.json`.
//!
//! The three parse inputs share nothing — a schema-constrained multilingual PDF
//! (EU, LLM-vision seam, offline cache), a structured XML feed (FR, deterministic
//! `quick-xml`), and a bot-gated HTML fragment (DE, deterministic light DOM) — so
//! each lives in its own module. The crate's `Adapter` impl detects the source
//! from the Bronze bytes and dispatches `parse`/`normalize` + `regime()` to the
//! matching sub-module. Every row is `record_type = interest` (§0.1). Live
//! discovery/fetch per source (EU europarl GET, FR HATVP open-data GET, DE
//! browser-engine Enodia seam) is a documented runner-binding follow-up — this
//! leg wires the offline conformance path and fails closed on the live path.

pub mod de;
pub mod eu;
pub mod fr;

mod dom;
mod ids;
mod util;

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use anyhow::Context as _;
use async_trait::async_trait;

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::{
    FilingRef, JurisdictionAdapter, PolitenessCfg, RawDocRef, RegimeRef, RunCtx, StagingRow,
};

/// Which source sub-adapter a document belongs to (§0). Detected from the Bronze
/// bytes: a PDF is a DPI, HATVP XML is a DIA, a Bundestag HTML page is DE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    Eu,
    Fr,
    De,
}

impl Source {
    /// The `disclosure_regime` slug this source binds to (§0).
    const fn regime_code(self) -> &'static str {
        match self {
            Self::Eu => eu::REGIME,
            Self::Fr => fr::REGIME,
            Self::De => de::REGIME,
        }
    }

    /// Sniffs the source from the document's leading bytes.
    fn detect(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.starts_with(b"%PDF") {
            return Ok(Self::Eu);
        }
        let head_len = bytes.len().min(8192);
        let head = String::from_utf8_lossy(&bytes[..head_len]);
        if head.contains("<declaration") {
            Ok(Self::Fr)
        } else if head.contains("<html") || head.to_ascii_lowercase().contains("<!doctype html") {
            Ok(Self::De)
        } else {
            anyhow::bail!(
                "document is neither a DPI PDF, a HATVP DIA XML, nor a Bundestag HTML page — freeze (invariant 6)"
            )
        }
    }
}

/// The compound `eu_fr_de_annual` adapter (§0). Holds the EU LLM-vision seam and
/// the last-parsed source (so `regime()`, which the conformance harness calls per
/// fixture after `parse`, returns the right per-source regime code).
#[derive(Debug)]
pub struct EuFrDeAnnualAdapter {
    eu_extractor: eu::LlmExtractor,
    current: Mutex<Source>,
}

impl Default for EuFrDeAnnualAdapter {
    fn default() -> Self {
        Self {
            eu_extractor: eu::LlmExtractor::default(),
            current: Mutex::new(Source::Fr),
        }
    }
}

impl EuFrDeAnnualAdapter {
    fn set_source(&self, source: Source) {
        *self.current.lock().unwrap_or_else(PoisonError::into_inner) = source;
    }

    fn source(&self) -> Source {
        *self.current.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[async_trait]
impl JurisdictionAdapter for EuFrDeAnnualAdapter {
    fn regime(&self) -> RegimeRef {
        RegimeRef {
            code: self.source().regime_code(),
        }
    }

    fn politeness(&self) -> PolitenessCfg {
        // Conservative combined config (europarl robots Crawl-delay 3 governs; the
        // trait exposes one config, per-source spacing is applied at fetch). The
        // HTTP client is unused on the offline conformance path.
        PolitenessCfg::new(Duration::from_secs(3), "ssm.leo@outlook.com")
    }

    /// Live discovery per source (EU europarl per-MEP DPI links, FR HATVP
    /// `liste.csv`, DE browser-engine biografien sweep) is a documented
    /// runner-binding follow-up; this leg fails closed rather than sweeping
    /// blind (invariant 6).
    async fn discover(&self, _ctx: &RunCtx) -> anyhow::Result<Vec<FilingRef>> {
        anyhow::bail!(
            "eu_fr_de_annual live discovery is a per-source runner-binding follow-up \
             (EU europarl GET, FR HATVP open-data GET, DE browser-engine Enodia seam) — \
             the offline conformance path drives parse/normalize directly (invariant 6)"
        )
    }

    /// Live fetch per source is a documented follow-up (the DE host is
    /// Enodia-bot-gated and requires the browser-engine seam, §DE.2; never
    /// fingerprint evasion). Conformance reads fixtures straight into Bronze.
    async fn fetch(&self, _r: &FilingRef, _ctx: &RunCtx) -> anyhow::Result<RawDocRef> {
        anyhow::bail!(
            "eu_fr_de_annual live fetch is a per-source runner-binding follow-up \
             (DE requires the browser-engine seam behind the Enodia gate, §DE.2) — \
             freeze + review, never evade (invariant 6)"
        )
    }

    async fn parse(&self, d: &RawDocRef, ctx: &RunCtx) -> anyhow::Result<Vec<StagingRow>> {
        let bytes = ctx
            .bronze
            .get(d)
            .context("reading Bronze document for parse")?;
        let source = Source::detect(&bytes)?;
        self.set_source(source);
        match source {
            Source::Eu => self.eu_extractor.extract(d, ctx).await,
            Source::Fr => fr::parse(&bytes, &d.sha256),
            Source::De => de::parse(&bytes, &d.sha256),
        }
    }

    async fn normalize(
        &self,
        rows: &[StagingRow],
        ctx: &RunCtx,
    ) -> anyhow::Result<Vec<GoldCandidate>> {
        // Dispatch on the Silver payload shape (stateless): DPI rows carry
        // `dpi_uuid`, DIA rows `declaration_stem`, Bundestag rows `mdb_id`.
        let first = rows
            .first()
            .context("normalize received zero rows — freeze (invariant 6)")?;
        if first.payload.get("dpi_uuid").is_some() {
            eu::normalize(rows, ctx)
        } else if first.payload.get("declaration_stem").is_some() {
            fr::normalize(rows, ctx)
        } else if first.payload.get("mdb_id").is_some() {
            de::normalize(rows, ctx)
        } else {
            anyhow::bail!("unrecognized Silver row shape at normalize — freeze (invariant 6)")
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn source_detects_by_document_shape() {
        assert_eq!(Source::detect(b"%PDF-1.7\n...").unwrap(), Source::Eu);
        assert_eq!(
            Source::detect(br#"<?xml version="1.0"?><declaration><uuid>x</uuid></declaration>"#)
                .unwrap(),
            Source::Fr
        );
        assert_eq!(
            Source::detect(b"\n<!DOCTYPE html>\n<html lang=\"de\"><body></body></html>").unwrap(),
            Source::De
        );
        assert!(Source::detect(b"garbage").is_err());
    }

    #[test]
    fn regime_code_follows_the_last_parsed_source() {
        let adapter = EuFrDeAnnualAdapter::default();
        adapter.set_source(Source::Eu);
        assert_eq!(adapter.regime().code, "eu_parliament_dpi");
        adapter.set_source(Source::De);
        assert_eq!(adapter.regime().code, "de_bundestag");
    }
}
