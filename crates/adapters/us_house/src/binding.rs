//! Runner binding for `us_house` (plan Task 9): the adapter-owned glue the
//! in-process runner needs — filing identity from silver rows (design §5.4:
//! filings name their filer), typed `stg_us_house` staging (source-shaped,
//! regime doc §4), and the §3.7 amendment review rule at publish.

use anyhow::Context as _;
use async_trait::async_trait;
use sqlx::PgPool;

use govfolio_core::domain::gold::GoldCandidate;
use pipeline::adapter::StagingRow;
use pipeline::run::{FilingIdentity, RunnerBinding, StagedSilver};

use crate::normalize::parse_source_date;
use crate::parse::SilverRow;

/// The `us_house` [`RunnerBinding`].
#[derive(Debug, Default, Clone, Copy)]
pub struct UsHouseBinding;

/// `stg_us_house` row image for loading staged silver back (stage replay).
#[derive(sqlx::FromRow)]
struct StgUsHouseRow {
    #[sqlx(try_from = "i32")]
    row_ordinal: u32,
    doc_id: String,
    filer_name_raw: String,
    filer_status_raw: String,
    state_district_raw: String,
    row_id_raw: Option<String>,
    owner_code_raw: Option<String>,
    asset_raw: String,
    asset_type_code_raw: Option<String>,
    transaction_type_raw: String,
    transaction_date_raw: String,
    notification_date_raw: String,
    amount_raw: String,
    cap_gains_over_200: Option<bool>,
    filing_status_raw: String,
    subholding_of_raw: Option<String>,
    description_raw: Option<String>,
    comments_raw: Option<String>,
    vehicle_owner_code_raw: Option<String>,
    vehicle_location_raw: Option<String>,
    signed_date_raw: String,
    extractor: String,
    confidence: f32,
}

impl StgUsHouseRow {
    fn into_staging_row(self) -> anyhow::Result<StagingRow> {
        let confidence = self.confidence;
        let row = SilverRow {
            doc_id: self.doc_id,
            row_ordinal: self.row_ordinal,
            filer_name_raw: self.filer_name_raw,
            filer_status_raw: self.filer_status_raw,
            state_district_raw: self.state_district_raw,
            row_id_raw: self.row_id_raw,
            owner_code_raw: self.owner_code_raw,
            asset_raw: self.asset_raw,
            asset_type_code_raw: self.asset_type_code_raw,
            transaction_type_raw: self.transaction_type_raw,
            transaction_date_raw: self.transaction_date_raw,
            notification_date_raw: self.notification_date_raw,
            amount_raw: self.amount_raw,
            cap_gains_over_200: self.cap_gains_over_200,
            filing_status_raw: self.filing_status_raw,
            subholding_of_raw: self.subholding_of_raw,
            description_raw: self.description_raw,
            comments_raw: self.comments_raw,
            vehicle_owner_code_raw: self.vehicle_owner_code_raw,
            vehicle_location_raw: self.vehicle_location_raw,
            signed_date_raw: self.signed_date_raw,
            extractor: self.extractor,
        };
        Ok(StagingRow {
            payload: serde_json::to_value(&row).context("serializing staged silver payload")?,
            confidence,
        })
    }
}

fn silver_row(staged: &StagingRow) -> anyhow::Result<SilverRow> {
    serde_json::from_value(staged.payload.clone())
        .context("silver payload is not a us_house staging row")
}

#[async_trait]
impl RunnerBinding for UsHouseBinding {
    fn silver_table(&self) -> &'static str {
        "stg_us_house"
    }

    fn filing_identity(&self, rows: &[StagingRow]) -> anyhow::Result<FilingIdentity> {
        let first = rows
            .first()
            .context("no silver rows — cannot derive filing identity")?;
        let row = silver_row(first)?;
        for staged in &rows[1..] {
            let other = silver_row(staged)?;
            anyhow::ensure!(
                other.doc_id == row.doc_id,
                "silver rows disagree on doc_id ({} vs {}) — fail closed",
                other.doc_id,
                row.doc_id
            );
        }
        // filed_date: the filer-claimed signature date (regime doc §2.2: it
        // equals the index FilingDate on every sample) — or, on paper
        // filings, the clerk received stamp (quirks log 2026-07-05).
        let filed_date = parse_source_date(&row.signed_date_raw)?;
        Ok(FilingIdentity {
            external_id: row.doc_id,
            filer_name: row.filer_name_raw,
            district: row.state_district_raw,
            filing_type: "P".to_owned(), // this adapter only discovers PTRs
            filed_date: Some(filed_date),
        })
    }

    async fn stage_silver(
        &self,
        pool: &PgPool,
        raw_document_id: &str,
        rows: &[StagingRow],
    ) -> anyhow::Result<Vec<StagedSilver>> {
        for staged in rows {
            let row = silver_row(staged)?;
            let ordinal = i32::try_from(row.row_ordinal).context("row ordinal overflow")?;
            sqlx::query(
                "insert into stg_us_house \
                   (id, raw_document_id, row_ordinal, doc_id, filer_name_raw, filer_status_raw, \
                    state_district_raw, row_id_raw, owner_code_raw, asset_raw, \
                    asset_type_code_raw, transaction_type_raw, transaction_date_raw, \
                    notification_date_raw, amount_raw, cap_gains_over_200, filing_status_raw, \
                    subholding_of_raw, description_raw, comments_raw, vehicle_owner_code_raw, \
                    vehicle_location_raw, signed_date_raw, extractor, confidence) \
                 values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, \
                         $16, $17, $18, $19, $20, $21, $22, $23, $24, $25) \
                 on conflict (raw_document_id, row_ordinal) do nothing",
            )
            .bind(ulid::Ulid::new().to_string())
            .bind(raw_document_id)
            .bind(ordinal)
            .bind(&row.doc_id)
            .bind(&row.filer_name_raw)
            .bind(&row.filer_status_raw)
            .bind(&row.state_district_raw)
            .bind(&row.row_id_raw)
            .bind(&row.owner_code_raw)
            .bind(&row.asset_raw)
            .bind(&row.asset_type_code_raw)
            .bind(&row.transaction_type_raw)
            .bind(&row.transaction_date_raw)
            .bind(&row.notification_date_raw)
            .bind(&row.amount_raw)
            .bind(row.cap_gains_over_200)
            .bind(&row.filing_status_raw)
            .bind(&row.subholding_of_raw)
            .bind(&row.description_raw)
            .bind(&row.comments_raw)
            .bind(&row.vehicle_owner_code_raw)
            .bind(&row.vehicle_location_raw)
            .bind(&row.signed_date_raw)
            .bind(&row.extractor)
            .bind(staged.confidence)
            .execute(pool)
            .await
            .with_context(|| format!("staging stg_us_house row {}", row.row_ordinal))?;
        }
        // Surviving row ids (existing rows on replay), document order.
        let ids: Vec<String> = sqlx::query_scalar(
            "select id from stg_us_house where raw_document_id = $1 order by row_ordinal",
        )
        .bind(raw_document_id)
        .fetch_all(pool)
        .await
        .context("reading staged stg_us_house ids")?;
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
        let rows: Vec<StgUsHouseRow> = sqlx::query_as(
            "select row_ordinal, doc_id, filer_name_raw, filer_status_raw, state_district_raw, \
                    row_id_raw, owner_code_raw, asset_raw, asset_type_code_raw, \
                    transaction_type_raw, transaction_date_raw, notification_date_raw, \
                    amount_raw, cap_gains_over_200, filing_status_raw, subholding_of_raw, \
                    description_raw, comments_raw, vehicle_owner_code_raw, vehicle_location_raw, \
                    signed_date_raw, extractor, confidence \
             from stg_us_house where raw_document_id = $1 order by row_ordinal",
        )
        .bind(raw_document_id)
        .fetch_all(pool)
        .await
        .context("loading staged stg_us_house rows")?;
        rows.into_iter()
            .map(StgUsHouseRow::into_staging_row)
            .collect()
    }

    fn review_reasons(&self, candidate: &GoldCandidate) -> Vec<String> {
        // Regime doc §3.7: amended rows publish as normal Gold inserts with
        // supersession NULL; each one opens a ptr_amendment_unlinked task.
        if candidate
            .details
            .get("filing_status_raw")
            .and_then(serde_json::Value::as_str)
            == Some("Amended")
        {
            vec!["ptr_amendment_unlinked".to_owned()]
        } else {
            Vec::new()
        }
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
            confidence: 0.98,
        }
    }

    fn payload(doc_id: &str) -> serde_json::Value {
        json!({
            "doc_id": doc_id,
            "row_ordinal": 1,
            "filer_name_raw": "Hon. Nicholas Begich III",
            "filer_status_raw": "Member",
            "state_district_raw": "AK00",
            "row_id_raw": null,
            "owner_code_raw": null,
            "asset_raw": "Listen Ventures IV, LP [HN]",
            "asset_type_code_raw": "HN",
            "transaction_type_raw": "P",
            "transaction_date_raw": "05/13/2026",
            "notification_date_raw": "05/13/2026",
            "amount_raw": "$250,001 - $500,000",
            "cap_gains_over_200": null,
            "filing_status_raw": "New",
            "subholding_of_raw": null,
            "description_raw": null,
            "comments_raw": null,
            "vehicle_owner_code_raw": null,
            "vehicle_location_raw": null,
            "signed_date_raw": "06/12/2026",
            "extractor": "us_house_ptr/text@1"
        })
    }

    #[test]
    fn filing_identity_comes_from_the_document_itself() {
        let identity = UsHouseBinding
            .filing_identity(&[staged(payload("20020055"))])
            .unwrap();
        assert_eq!(identity.external_id, "20020055");
        assert_eq!(identity.filer_name, "Hon. Nicholas Begich III");
        assert_eq!(identity.district, "AK00");
        assert_eq!(identity.filing_type, "P");
        assert_eq!(identity.filed_date.unwrap().to_string(), "2026-06-12");
    }

    #[test]
    fn disagreeing_doc_ids_fail_closed() {
        let rows = [staged(payload("20020055")), staged(payload("20099999"))];
        assert!(UsHouseBinding.filing_identity(&rows).is_err());
    }

    #[test]
    fn amended_rows_route_to_the_unlinked_amendment_queue() {
        let mut candidate: GoldCandidate = serde_json::from_value(json!({
            "filing_id": "00000000000000000000000000",
            "politician_id": "00000000000000000000000000",
            "regime_id": "00000000000000000000000000",
            "instrument_id": null,
            "asset_description_raw": "X",
            "record_type": "transaction",
            "asset_class": "equity",
            "side": "buy",
            "transaction_date": "2026-06-01",
            "as_of_date": null,
            "notified_date": null,
            "value": null,
            "owner": null,
            "extraction_confidence": null,
            "extracted_by": "t",
            "fingerprint": null,
            "details": {"filing_status_raw": "Amended"}
        }))
        .unwrap();
        assert_eq!(
            UsHouseBinding.review_reasons(&candidate),
            ["ptr_amendment_unlinked"]
        );
        candidate.details = json!({"filing_status_raw": "New"});
        assert_eq!(
            UsHouseBinding.review_reasons(&candidate),
            Vec::<String>::new()
        );
    }
}
