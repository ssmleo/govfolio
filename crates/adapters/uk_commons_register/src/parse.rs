//! Bronze → Silver: `serde_json` deserialize of one `/Interests/{id}`
//! response (regime doc §3 anatomy, §6 strategy — govfolio's first pure-JSON
//! parse stage; no scraping, no LLM seam on the green path). The top-level
//! object is `deny_unknown_fields` so contract drift surfaces as a freeze,
//! while the `fields[].name` VOCABULARY stays open by design (§3.2/§6.1).
//!
//! Hard rejects (regime doc §3.8 + §6.2): response id ≠ requested id, unknown
//! category id (rules change — freeze), non-Commons house/category type,
//! empty summary, unparseable date strings, §3.4 R2c unknown threshold
//! strings, unmapped currency codes, mixed-currency donor sums — errors,
//! never low-confidence rows (invariant 6 over confidence).

use anyhow::Context as _;
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::fields;

/// Extractor id recorded on every Silver row (regime doc §4 — the
/// deterministic `serde_json` path; no LLM seam on this adapter's green path).
pub(crate) const EXTRACTOR: &str = "uk_commons_register/api@1";

/// §6.2 confidence scoring: start at 1.00 …
const BASE_CONFIDENCE: f32 = 1.0;
/// … −0.02 when `registrationDate` is null (migrated legacy row — the
/// notification date is lost) …
const LEGACY_NULL_REGISTRATION_PENALTY: f32 = 0.02;
/// … −0.05 when the value is a multi-donor sum (aggregation convention on an
/// unobserved shape).
const MULTI_DONOR_SUM_PENALTY: f32 = 0.05;

/// One `stg_uk_commons_register` payload: source-faithful verbatim values
/// from the fetched JSON, regime doc §4 field for field (confidence lives on
/// the [`pipeline::adapter::StagingRow`] wrapper, not here).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SilverRow {
    pub(crate) interest_id: u32,
    /// Always 1: one interest per Bronze document (§3.8 check 5); kept for
    /// the shared `(raw_document_id, row_ordinal)` dedup-key shape.
    pub(crate) row_ordinal: u32,
    /// `updatedDates.length` — the filing version key (§2.5).
    pub(crate) version: u32,
    pub(crate) parent_interest_id: Option<u32>,
    pub(crate) category_id: u32,
    pub(crate) category_number_raw: String,
    pub(crate) category_name_raw: String,
    pub(crate) member_id: u32,
    pub(crate) member_name_raw: String,
    pub(crate) member_list_name_raw: String,
    pub(crate) member_from_raw: String,
    pub(crate) party_raw: Option<String>,
    pub(crate) house_raw: String,
    pub(crate) summary_raw: String,
    pub(crate) registration_date_raw: Option<String>,
    pub(crate) published_date_raw: Option<String>,
    pub(crate) updated_dates_raw: Vec<String>,
    pub(crate) rectified: bool,
    pub(crate) rectified_details_raw: Option<String>,
    /// The source `fields` array VERBATIM (incl. `""` vs null empties,
    /// human typos, non-ASCII — raw is sacred, invariant 2).
    pub(crate) fields_raw: serde_json::Value,
    pub(crate) extractor: String,
}

/// A Silver row plus its §6.2 rule-based confidence score.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScoredRow {
    pub(crate) row: SilverRow,
    pub(crate) confidence: f32,
}

/// One `/Interests/{id}` response (E1 `PublishedInterest`). Strict at the
/// JSON-key level: an unknown key is contract drift — freeze first, then
/// extend against re-archived evidence (§6.4). `childInterests` is absent on
/// `/Interests/{id}` by contract (§3), so its appearance is drift too.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)] // `links` is HATEOAS boilerplate — accepted, never read (§3).
struct PublishedInterest {
    id: u32,
    summary: String,
    #[serde(rename = "parentInterestId")]
    parent_interest_id: Option<u32>,
    #[serde(rename = "registrationDate")]
    registration_date: Option<String>,
    #[serde(rename = "publishedDate")]
    published_date: Option<String>,
    #[serde(rename = "updatedDates")]
    updated_dates: Vec<String>,
    category: Category,
    member: Member,
    /// Kept as raw JSON values: Silver stores this array VERBATIM; the typed
    /// §3.2 view is parsed from these same values for validation.
    fields: Vec<serde_json::Value>,
    links: serde_json::Value,
    rectified: bool,
    #[serde(rename = "rectifiedDetails")]
    rectified_details: Option<String>,
}

/// `category` object (§3 anatomy).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
// `parent_category_ids`/`links` accepted, never read.
// `category_type` mirrors the source key `type` (a Rust keyword) — the
// prefix disambiguates, the vocabulary is the regime doc's.
#[allow(clippy::struct_field_names)]
struct Category {
    id: u32,
    number: String,
    name: String,
    #[serde(rename = "parentCategoryIds")]
    parent_category_ids: serde_json::Value,
    #[serde(rename = "type")]
    category_type: String,
    links: serde_json::Value,
}

/// `member` object (§3 anatomy / §2.4 resolution input).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(dead_code)]
// `links` accepted, never read.
// `member_from` mirrors the source key `memberFrom` (§3 anatomy) — the
// regime-doc vocabulary, not a naming accident.
#[allow(clippy::struct_field_names)]
struct Member {
    id: u32,
    #[serde(rename = "nameDisplayAs")]
    name_display_as: String,
    #[serde(rename = "nameListAs")]
    name_list_as: String,
    house: String,
    #[serde(rename = "memberFrom")]
    member_from: String,
    party: Option<String>,
    links: serde_json::Value,
}

/// Parses one interest document into exactly one scored Silver row. The
/// requested interest id is threaded by the caller — §3.8 check 1 rejects a
/// response whose printed id disagrees.
pub(crate) fn parse_document(text: &str, requested_id: u32) -> anyhow::Result<ScoredRow> {
    let doc: PublishedInterest = serde_json::from_str(text).context(
        "document outside the archived PublishedInterest shape — contract drift, freeze (§6.4)",
    )?;

    // §3.8 integrity cross-checks — document REJECTS, not scores.
    anyhow::ensure!(
        doc.id == requested_id,
        "response id {} != requested id {requested_id} — hard reject (§3.8 check 1)",
        doc.id
    );
    anyhow::ensure!(
        fields::KNOWN_CATEGORY_IDS.contains(&doc.category.id),
        "unknown category id {} ({:?}) — rules change, freeze + review (§3.8 check 2)",
        doc.category.id,
        doc.category.name
    );
    anyhow::ensure!(
        doc.member.house == "Commons",
        "member.house {:?} is not Commons — hard reject (§3.8 check 3)",
        doc.member.house
    );
    anyhow::ensure!(
        doc.category.category_type == "Commons",
        "category.type {:?} is not Commons — hard reject (§3.8 check 3)",
        doc.category.category_type
    );
    anyhow::ensure!(
        !doc.summary.is_empty(),
        "empty summary — hard reject (§3.3, invariant 2)"
    );
    validate_date("registrationDate", doc.registration_date.as_deref())?;
    validate_date("publishedDate", doc.published_date.as_deref())?;
    for date in &doc.updated_dates {
        validate_date("updatedDates entry", Some(date))?;
    }
    let version = u32::try_from(doc.updated_dates.len()).context("updatedDates overflow")?;

    // §3.4 value rules run here as validation (R2c / unmapped currency /
    // mixed-currency donor sums are parse-time hard rejects, §6.2) and to
    // score the multi-donor aggregation penalty.
    let fields_raw = serde_json::Value::Array(doc.fields);
    let typed = fields::typed_fields(&fields_raw)?;
    let outcome = fields::evaluate_value(doc.category.id, &typed)?;

    let mut confidence = BASE_CONFIDENCE;
    if doc.registration_date.is_none() {
        confidence -= LEGACY_NULL_REGISTRATION_PENALTY;
    }
    if outcome.donor_money_rows > 1 {
        confidence -= MULTI_DONOR_SUM_PENALTY;
    }

    Ok(ScoredRow {
        row: SilverRow {
            interest_id: doc.id,
            row_ordinal: 1,
            version,
            parent_interest_id: doc.parent_interest_id,
            category_id: doc.category.id,
            category_number_raw: doc.category.number,
            category_name_raw: doc.category.name,
            member_id: doc.member.id,
            member_name_raw: doc.member.name_display_as,
            member_list_name_raw: doc.member.name_list_as,
            member_from_raw: doc.member.member_from,
            party_raw: doc.member.party,
            house_raw: doc.member.house,
            summary_raw: doc.summary,
            registration_date_raw: doc.registration_date,
            published_date_raw: doc.published_date,
            updated_dates_raw: doc.updated_dates,
            rectified: doc.rectified,
            rectified_details_raw: doc.rectified_details,
            fields_raw,
            extractor: EXTRACTOR.to_owned(),
        },
        confidence,
    })
}

/// Source dates print as `YYYY-MM-DD` (§3.2 `DateOnly`); anything else is an
/// unparseable date string — hard reject (§6.2). Nulls pass through (their
/// per-field nullability is ruled on elsewhere).
fn validate_date(what: &str, raw: Option<&str>) -> anyhow::Result<()> {
    if let Some(raw) = raw {
        NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .with_context(|| format!("{what} {raw:?} is not YYYY-MM-DD — hard reject (§6.2)"))?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn doc_json() -> serde_json::Value {
        json!({
            "id": 15475,
            "summary": "Shares in Lockhouse Systems Limited",
            "parentInterestId": null,
            "registrationDate": "2026-06-17",
            "publishedDate": "2026-06-17",
            "updatedDates": [],
            "category": {
                "id": 8, "number": "7", "name": "Shareholdings",
                "parentCategoryIds": [], "type": "Commons", "links": []
            },
            "member": {
                "id": 4051, "nameDisplayAs": "John Glen", "nameListAs": "Glen, John",
                "house": "Commons", "memberFrom": "Salisbury",
                "party": "Conservative", "links": []
            },
            "fields": [
                {"name": "ShareholdingThreshold", "description": "Registrable threshold",
                 "type": "String", "typeInfo": null,
                 "value": "(ii) Other shareholdings, valued at more than £70,000"}
            ],
            "links": [],
            "rectified": false,
            "rectifiedDetails": null
        })
    }

    fn parse(value: &serde_json::Value, requested_id: u32) -> anyhow::Result<ScoredRow> {
        parse_document(&value.to_string(), requested_id)
    }

    #[test]
    #[allow(clippy::float_cmp)] // bit-equality IS the contract (MANIFEST float rule)
    fn clean_document_parses_to_one_full_confidence_row() {
        let scored = parse(&doc_json(), 15475).unwrap();
        assert_eq!(scored.confidence, 1.0);
        assert_eq!(scored.row.interest_id, 15475);
        assert_eq!(scored.row.row_ordinal, 1);
        assert_eq!(scored.row.version, 0);
        assert_eq!(scored.row.category_number_raw, "7");
        assert_eq!(scored.row.extractor, "uk_commons_register/api@1");
        assert_eq!(
            scored.row.fields_raw[0]["value"],
            json!("(ii) Other shareholdings, valued at more than £70,000"),
            "fields stay verbatim"
        );
    }

    #[test]
    fn id_mismatch_is_a_hard_reject() {
        assert!(parse(&doc_json(), 99999).is_err(), "§3.8 check 1");
    }

    #[test]
    fn unknown_category_id_freezes() {
        let mut doc = doc_json();
        doc["category"]["id"] = json!(13);
        assert!(parse(&doc, 15475).is_err(), "§3.8 check 2");
    }

    #[test]
    fn non_commons_house_or_category_type_rejects() {
        let mut doc = doc_json();
        doc["member"]["house"] = json!("Lords");
        assert!(parse(&doc, 15475).is_err(), "§3.8 check 3 (house)");
        let mut doc = doc_json();
        doc["category"]["type"] = json!("Lords");
        assert!(parse(&doc, 15475).is_err(), "§3.8 check 3 (category type)");
    }

    #[test]
    fn empty_summary_and_bad_dates_reject() {
        let mut doc = doc_json();
        doc["summary"] = json!("");
        assert!(parse(&doc, 15475).is_err(), "empty summary");
        let mut doc = doc_json();
        doc["registrationDate"] = json!("17/06/2026");
        assert!(parse(&doc, 15475).is_err(), "unparseable date");
    }

    #[test]
    fn unknown_top_level_key_is_contract_drift() {
        let mut doc = doc_json();
        doc["surprise"] = json!(1);
        assert!(parse(&doc, 15475).is_err(), "deny_unknown_fields (§6.1)");
    }

    #[test]
    #[allow(clippy::float_cmp)] // bit-equality IS the contract (MANIFEST float rule)
    fn null_registration_date_docks_confidence_and_version_counts_updates() {
        let mut doc = doc_json();
        doc["registrationDate"] = json!(null);
        doc["updatedDates"] = json!(["2024-07-26", "2026-06-18"]);
        let scored = parse(&doc, 15475).unwrap();
        assert_eq!(scored.confidence, 1.0 - 0.02);
        assert_eq!(scored.row.version, 2);
        assert_eq!(scored.row.registration_date_raw, None);
    }

    #[test]
    fn r2c_unknown_threshold_string_rejects_at_parse() {
        let mut doc = doc_json();
        doc["fields"][0]["value"] = json!("(iii) A brand new threshold");
        assert!(parse(&doc, 15475).is_err(), "R2c — §6.2 parse-time reject");
    }
}
