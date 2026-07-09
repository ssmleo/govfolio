//! Runner binding for `br` (plan.md "Handoff" / `docs/regimes/br/AUTHORITY.md`
//! Quirks log): the adapter-owned glue the in-process runner needs — filing
//! identity from silver rows (design §5.4: filings name their filer), typed
//! `stg_br` staging (source-shaped, `crates/core/migrations/0010_silver_br.sql`),
//! and this regime's (empty) publish-time review-reason policy.
//!
//! Second `RunnerBinding` this project has ever built (`us_house` is the
//! reference/only prior instance) — see this file's own tests for the
//! `us_house::binding` precedent this mirrors field for field.

use anyhow::Context as _;
use async_trait::async_trait;
use sqlx::PgPool;

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::StagingRow;
use pipeline::run::{FilingIdentity, RunnerBinding, StagedSilver};

use crate::normalize::parse_br_date;
use crate::parse::SilverRow;

/// This regime's constant `filing.filing_type` (Brazil has no `us_house`-style
/// P/A filing-type letter code — every declaration is the same document kind,
/// the `declaração de bens`).
const FILING_TYPE: &str = "declaracao_de_bens";

/// TSE's documented numeric-null sentinel for a suppressed `NR_CPF_CANDIDATO`
/// (AUTHORITY.md `regime_versions`, 2024 amendment — Resolução TSE nº
/// 23.729/2024 masks CPF in the bulk open-data files from the 2024 cycle
/// on).
const CPF_SENTINEL: &str = "-4";

/// Selects this candidate's durable per-filer identifier (design
/// §3.2): CPF (`NR_CPF_CANDIDATO`) when present and not the masked sentinel,
/// falling back to the voter-registration number
/// (`NR_TITULO_ELEITORAL_CANDIDATO`), which AUTHORITY.md confirms stays
/// unmasked in every cycle checked so far, including 2024. `None` only when
/// neither field is present (conformance-mode fixtures, which never
/// populate these PII-gated fields at all).
pub(crate) fn external_identifier(cpf: Option<&str>, titulo: Option<&str>) -> Option<String> {
    cpf.filter(|value| *value != CPF_SENTINEL)
        .or(titulo)
        .map(str::to_owned)
}

/// The `br` [`RunnerBinding`].
#[derive(Debug, Default, Clone, Copy)]
pub struct BrBinding;

/// `stg_br` row image for loading staged silver back (stage replay).
/// `row_ordinal` drives the query's `order by` only (document order) — it is
/// not itself part of [`SilverRow`] (unlike `us_house`'s own `row_ordinal`
/// field), so it is not selected into this struct.
#[derive(sqlx::FromRow)]
struct StgBrRow {
    sq_candidato: String,
    nm_candidato: String,
    sg_uf: String,
    dt_eleicao_raw: String,
    election_year_raw: String,
    line_item_ordinal_raw: String,
    asset_type_code_raw: String,
    asset_type_label_raw: String,
    asset_description_raw: String,
    value_raw: String,
    last_updated_date_raw: String,
    last_updated_time_raw: String,
    extractor: String,
    nr_titulo_eleitoral_candidato: Option<String>,
    nr_cpf_candidato: Option<String>,
    confidence: f32,
}

impl StgBrRow {
    fn into_staging_row(self) -> anyhow::Result<StagingRow> {
        let confidence = self.confidence;
        let row = SilverRow {
            sq_candidato: self.sq_candidato,
            nm_candidato: self.nm_candidato,
            sg_uf: self.sg_uf,
            dt_eleicao_raw: self.dt_eleicao_raw,
            election_year_raw: self.election_year_raw,
            line_item_ordinal_raw: self.line_item_ordinal_raw,
            asset_type_code_raw: self.asset_type_code_raw,
            asset_type_label_raw: self.asset_type_label_raw,
            asset_description_raw: self.asset_description_raw,
            value_raw: self.value_raw,
            last_updated_date_raw: self.last_updated_date_raw,
            last_updated_time_raw: self.last_updated_time_raw,
            extractor: self.extractor,
            nr_titulo_eleitoral_candidato: self.nr_titulo_eleitoral_candidato,
            nr_cpf_candidato: self.nr_cpf_candidato,
        };
        Ok(StagingRow {
            payload: serde_json::to_value(&row).context("serializing staged silver payload")?,
            confidence,
        })
    }
}

fn silver_row(staged: &StagingRow) -> anyhow::Result<SilverRow> {
    serde_json::from_value(staged.payload.clone()).context("silver payload is not a br staging row")
}

#[async_trait]
impl RunnerBinding for BrBinding {
    fn silver_table(&self) -> &'static str {
        "stg_br"
    }

    fn filing_identity(&self, rows: &[StagingRow]) -> anyhow::Result<FilingIdentity> {
        let first = rows
            .first()
            .context("no silver rows — cannot derive filing identity")?;
        let row = silver_row(first)?;
        for staged in &rows[1..] {
            let other = silver_row(staged)?;
            anyhow::ensure!(
                other.sq_candidato == row.sq_candidato,
                "silver rows disagree on sq_candidato ({} vs {}) — fail closed",
                other.sq_candidato,
                row.sq_candidato
            );
        }
        let election_year: u16 = row
            .election_year_raw
            .parse()
            .with_context(|| format!("ANO_ELEICAO {:?} is not a year", row.election_year_raw))?;
        // SQ_CANDIDATO alone is only unique within one election cycle's file
        // set (AUTHORITY.md identifiers_available / plan.md field-mapping
        // table) — AND, confirmed goal 093 Phase 2, is not even guaranteed
        // nationally unique WITHIN one cycle for every year (2006 reuses the
        // same number across different states) — compose with the election
        // year AND state, matching `crate::adapter::BrAdapter::discover_year`'s
        // identical external_id scheme exactly (the publish-time drift guard
        // in `pipeline::run` requires these to agree byte for byte).
        let external_id = format!("{election_year}:{}:{}", row.sg_uf, row.sq_candidato);
        let filed_date = parse_br_date(&row.dt_eleicao_raw)?;
        let identifier = external_identifier(
            row.nr_cpf_candidato.as_deref(),
            row.nr_titulo_eleitoral_candidato.as_deref(),
        );
        Ok(FilingIdentity {
            external_id,
            filer_name: row.nm_candidato,
            district: row.sg_uf,
            filing_type: FILING_TYPE.to_owned(),
            filed_date: Some(filed_date),
            external_identifier: identifier,
        })
    }

    async fn stage_silver(
        &self,
        pool: &PgPool,
        raw_document_id: &str,
        rows: &[StagingRow],
    ) -> anyhow::Result<Vec<StagedSilver>> {
        for (index, staged) in rows.iter().enumerate() {
            let row = silver_row(staged)?;
            // `SilverRow` carries no `row_ordinal` field of its own (unlike
            // `us_house`'s) — plan.md's own "Row unit" section leaves the
            // exact join-field set to rust-builder, and one candidate's asset
            // items have no independent per-document position other than
            // their place in `parse()`'s emitted `Vec` (source-file order,
            // `row_order_assumption_flag`). Position-in-slice is therefore
            // this binding's own choice of a stable, 1-based staging ordinal.
            let ordinal = i32::try_from(index + 1).context("row ordinal overflow")?;
            sqlx::query(
                "insert into stg_br \
                   (id, raw_document_id, row_ordinal, sq_candidato, nm_candidato, sg_uf, \
                    dt_eleicao_raw, election_year_raw, line_item_ordinal_raw, \
                    asset_type_code_raw, asset_type_label_raw, asset_description_raw, \
                    value_raw, last_updated_date_raw, last_updated_time_raw, extractor, \
                    nr_titulo_eleitoral_candidato, nr_cpf_candidato, confidence) \
                 values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, \
                         $16, $17, $18, $19) \
                 on conflict (raw_document_id, row_ordinal) do nothing",
            )
            .bind(ulid::Ulid::new().to_string())
            .bind(raw_document_id)
            .bind(ordinal)
            .bind(&row.sq_candidato)
            .bind(&row.nm_candidato)
            .bind(&row.sg_uf)
            .bind(&row.dt_eleicao_raw)
            .bind(&row.election_year_raw)
            .bind(&row.line_item_ordinal_raw)
            .bind(&row.asset_type_code_raw)
            .bind(&row.asset_type_label_raw)
            .bind(&row.asset_description_raw)
            .bind(&row.value_raw)
            .bind(&row.last_updated_date_raw)
            .bind(&row.last_updated_time_raw)
            .bind(&row.extractor)
            .bind(&row.nr_titulo_eleitoral_candidato)
            .bind(&row.nr_cpf_candidato)
            .bind(staged.confidence)
            .execute(pool)
            .await
            .with_context(|| format!("staging stg_br row {ordinal}"))?;
        }
        // Surviving row ids (existing rows on replay), document order.
        let ids: Vec<String> = sqlx::query_scalar(
            "select id from stg_br where raw_document_id = $1 order by row_ordinal",
        )
        .bind(raw_document_id)
        .fetch_all(pool)
        .await
        .context("reading staged stg_br ids")?;
        Ok(ids
            .into_iter()
            .map(|stg_id| StagedSilver { stg_id })
            .collect())
    }

    async fn load_silver(
        &self,
        pool: &PgPool,
        raw_document_id: &str,
    ) -> anyhow::Result<Vec<StagingRow>> {
        let rows: Vec<StgBrRow> = sqlx::query_as(
            "select sq_candidato, nm_candidato, sg_uf, dt_eleicao_raw, \
                    election_year_raw, line_item_ordinal_raw, asset_type_code_raw, \
                    asset_type_label_raw, asset_description_raw, value_raw, \
                    last_updated_date_raw, last_updated_time_raw, extractor, \
                    nr_titulo_eleitoral_candidato, nr_cpf_candidato, confidence \
             from stg_br where raw_document_id = $1 order by row_ordinal",
        )
        .bind(raw_document_id)
        .fetch_all(pool)
        .await
        .context("loading staged stg_br rows")?;
        rows.into_iter().map(StgBrRow::into_staging_row).collect()
    }

    fn review_reasons(&self, _candidate: &GoldCandidate) -> Vec<String> {
        // plan.md edge case 2 (independently audited, AUTHORITY.md Quirks
        // log): DT_ULT_ATUAL_BEM_CANDIDATO is a bulk backend re-timestamp
        // artifact for 85-99% of a whole state's rows, not a reliable
        // per-item amendment signal — unlike us_house's "Amended" filing
        // status, there is no analogous trustworthy per-row trigger in this
        // regime's `details` to flag for review. The one other candidate
        // signal considered, an unmapped CD_TIPO_BEM_CANDIDATO code, already
        // surfaces via a lowered `extraction_confidence` on the Gold row
        // itself (normalize.rs's UNMAPPED_ASSET_CLASS_PENALTY) — there is no
        // established convention elsewhere in this codebase tying a
        // confidence penalty to a separate review_task, so none is invented
        // here (CLAUDE.md: don't invent a trigger without a real reason).
        Vec::new()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn staged(payload: serde_json::Value) -> StagingRow {
        StagingRow {
            payload,
            confidence: 1.0,
        }
    }

    fn payload(sq_candidato: &str) -> serde_json::Value {
        json!({
            "sq_candidato": sq_candidato,
            "nm_candidato": "ROGÉRIO DA SILVA E SILVA",
            "sg_uf": "AC",
            "dt_eleicao_raw": "02/10/2022",
            "election_year_raw": "2022",
            "line_item_ordinal_raw": "1",
            "asset_type_code_raw": "12",
            "asset_type_label_raw": "Casa",
            "asset_description_raw": "Casa na zona rural de xapuri",
            "value_raw": "10000,00",
            "last_updated_date_raw": "02/10/2022",
            "last_updated_time_raw": "23:21:28",
            "extractor": "br_bem_candidato/csv@1"
        })
    }

    #[test]
    fn filing_identity_comes_from_the_document_itself() {
        let identity = BrBinding
            .filing_identity(&[staged(payload("10001595344"))])
            .unwrap();
        assert_eq!(identity.external_id, "2022:AC:10001595344");
        assert_eq!(identity.filer_name, "ROGÉRIO DA SILVA E SILVA");
        assert_eq!(identity.district, "AC");
        assert_eq!(identity.filing_type, "declaracao_de_bens");
        assert_eq!(identity.filed_date.unwrap().to_string(), "2022-10-02");
        // Conformance-mode payload carries neither PII field (skipped, not
        // null) — no identifier to resolve by.
        assert_eq!(identity.external_identifier, None);
    }

    #[test]
    fn external_identifier_prefers_cpf() {
        assert_eq!(
            external_identifier(Some("80673872653"), Some("066773530590")),
            Some("80673872653".to_owned())
        );
    }

    #[test]
    fn external_identifier_falls_back_to_titulo_when_cpf_masked() {
        // TSE's documented sentinel for a CPF suppressed from the 2024 cycle
        // on (AUTHORITY.md regime_versions) — must not be treated as a real,
        // distinguishing value.
        assert_eq!(
            external_identifier(Some("-4"), Some("066773530590")),
            Some("066773530590".to_owned())
        );
    }

    #[test]
    fn external_identifier_none_when_neither_field_present() {
        assert_eq!(external_identifier(None, None), None);
    }

    #[test]
    fn disagreeing_sq_candidatos_fail_closed() {
        let rows = [
            staged(payload("10001595344")),
            staged(payload("99999999999")),
        ];
        assert!(BrBinding.filing_identity(&rows).is_err());
    }

    #[test]
    fn no_rows_fails_closed() {
        assert!(BrBinding.filing_identity(&[]).is_err());
    }

    #[test]
    fn review_reasons_is_always_empty() {
        let candidate: GoldCandidate = serde_json::from_value(json!({
            "filing_id": "00000000000000000000000000",
            "politician_id": "00000000000000000000000000",
            "regime_id": "00000000000000000000000000",
            "instrument_id": null,
            "asset_description_raw": "X",
            "record_type": "holding",
            "asset_class": "real_estate",
            "side": null,
            "transaction_date": null,
            "as_of_date": "2022-10-02",
            "notified_date": null,
            "value": null,
            "owner": null,
            "extraction_confidence": 0.90,
            "extracted_by": "t",
            "fingerprint": null,
            "details": {}
        }))
        .unwrap();
        assert_eq!(BrBinding.review_reasons(&candidate), Vec::<String>::new());
    }
}
