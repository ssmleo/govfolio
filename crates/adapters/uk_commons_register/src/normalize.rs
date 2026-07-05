//! Silver → Gold (regime doc §5.1): every row is `record_type = interest`
//! with `side`/`transaction_date`/`as_of_date` NULL, `notified_date` from
//! `registrationDate` (NEVER `publishedDate`, §3.6), value per the §3.4
//! rules, owner per §3.5, contract-typed `details` (§5). `instrument_id`
//! stays NULL — company-name resolution is below-threshold by default
//! (invariant 3). Record-level supersession of updated versions stays with
//! the promotion machinery (`uk_interest_update_unlinked` review tasks at
//! publish, §3.7) — deferred to the runner binding like `us_senate`.

use anyhow::Context as _;
use chrono::NaiveDate;

use govfolio_core::domain::enums::RecordType;
use govfolio_core::domain::gold::GoldCandidate;
use govfolio_core::ids::{FilingId, PoliticianId, RegimeId};
use pipeline::adapter::{RunCtx, StagingRow};

use crate::details::{DetailsField, UkCommonsRegisterInterestDetailsV1};
use crate::fields::{self, Field};
use crate::parse::SilverRow;

/// Fixed conformance ULIDs (fixtures `MANIFEST.json` `conformance_ids`): API
/// documents carry no ULIDs and conformance runs with `pool: None`, so these
/// constants ARE the contract in conformance mode. Production resolves real
/// ids from Postgres (runner-binding follow-up, `us_house` Task-9 pattern).
const CONFORMANCE_REGIME_ID: &str = "0GBRREG0000000000000000001";
/// Filing ULIDs embed the interest id digits, zero-padded to 19 (MANIFEST
/// rule; all pinned fixtures are version 0, so no version is embedded).
const CONFORMANCE_FILING_PREFIX: &str = "0GBRFNG";
/// Numeric MNIS `member.id` → politician ULID (MANIFEST `politician_ids`;
/// the §2.4 resolution input is an exact id join, no name matching).
const CONFORMANCE_POLITICIANS: &[(u32, &str)] = &[
    (4051, "0GBRMBR0000000000000000001"),
    (4651, "0GBRMBR0000000000000000002"),
    (5403, "0GBRMBR0000000000000000003"),
    (4521, "0GBRMBR0000000000000000004"),
];

/// Identity binding mode (`us_house`/`us_senate` pattern): conformance runs
/// (`pool: None`) emit the fixed MANIFEST ULID constants the fixtures pin;
/// pool-backed runs emit UNBOUND identity (nil ULIDs) for the runner's
/// publish stage to bind from Postgres. FK constraints reject a nil id if a
/// bug ever let one through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IdentityMode {
    Conformance,
    Unbound,
}

/// Nil ULID: the "identity not yet bound" marker for pool-backed runs.
const UNBOUND_ID: &str = "00000000000000000000000000";

/// Normalizes staged rows into Gold candidates.
pub(crate) fn normalize_rows(
    rows: &[StagingRow],
    ctx: &RunCtx,
) -> anyhow::Result<Vec<GoldCandidate>> {
    let mode = if ctx.pool.is_some() {
        IdentityMode::Unbound
    } else {
        IdentityMode::Conformance
    };
    rows.iter().map(|row| normalize_row(row, mode)).collect()
}

fn normalize_row(staged: &StagingRow, mode: IdentityMode) -> anyhow::Result<GoldCandidate> {
    let row: SilverRow = serde_json::from_value(staged.payload.clone())
        .context("silver payload is not a uk_commons_register staging row")?;

    let typed = fields::typed_fields(&row.fields_raw)?;
    // §3.4 value rules (parse already hard-rejected grammar violations).
    let outcome = fields::evaluate_value(row.category_id, &typed)?;
    // §3.5 owner map; §3.1 asset-class map.
    let owner = fields::owner_for_category(row.category_id, &typed);
    let asset_class = fields::asset_class_for_category(row.category_id);

    // §3.6: notified_date = registrationDate; NULL on legacy rows — honest,
    // never substituted with publishedDate.
    let notified_date = row
        .registration_date_raw
        .as_deref()
        .map(parse_iso_date)
        .transpose()?;
    // §5: published_date is contractually required; a null is unobserved and
    // rejects at promotion.
    let published_date = row
        .published_date_raw
        .clone()
        .context("null publishedDate — reject at promotion (§5, unobserved)")?;

    let shareholding_threshold_raw = fields::shareholding_threshold(&typed).map(ToOwned::to_owned);
    let details = UkCommonsRegisterInterestDetailsV1 {
        interest_id: row.interest_id,
        version: row.version,
        parent_interest_id: row.parent_interest_id,
        category_id: row.category_id,
        category_number: row.category_number_raw.clone(),
        category_name: row.category_name_raw.clone(),
        member_id: row.member_id,
        registration_date: row.registration_date_raw.clone(),
        published_date,
        updated_dates: row.updated_dates_raw.clone(),
        rectified: row.rectified,
        rectified_details: row.rectified_details_raw.clone(),
        shareholding_threshold_raw,
        value_source: outcome.source,
        fields: typed
            .iter()
            .map(flatten_field)
            .collect::<anyhow::Result<_>>()?,
    };

    let (filing_id, politician_id, regime_id) = match mode {
        IdentityMode::Conformance => (
            conformance_filing_id(row.interest_id)?,
            conformance_politician_id(row.member_id)?,
            CONFORMANCE_REGIME_ID
                .parse::<RegimeId>()
                .map_err(|e| anyhow::anyhow!("conformance regime id: {e}"))?,
        ),
        IdentityMode::Unbound => (
            unbound_id::<FilingId>("filing")?,
            unbound_id::<PoliticianId>("politician")?,
            unbound_id::<RegimeId>("regime")?,
        ),
    };

    Ok(GoldCandidate {
        filing_id,
        politician_id,
        regime_id,
        instrument_id: None, // company-name resolution is below threshold — never guess
        asset_description_raw: row.summary_raw.clone(),
        record_type: RecordType::Interest,
        asset_class,
        side: None,
        transaction_date: None,
        as_of_date: None,
        notified_date,
        value: outcome.interval,
        owner,
        extraction_confidence: Some(staged.confidence),
        extracted_by: row.extractor.clone(),
        // Computed at promotion over (filing_id, ordinal, content) — the
        // candidate ships without it (GoldCandidate contract).
        fingerprint: None,
        details: serde_json::to_value(&details).context("serializing details")?,
    })
}

/// One typed field → the §5 flattened [`DetailsField`] (recursing into the
/// nested rows of complex fields).
fn flatten_field(field: &Field) -> anyhow::Result<DetailsField> {
    let values = field
        .values
        .as_ref()
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    row.iter()
                        .map(|entry| {
                            let sub: Field =
                                serde_json::from_value(entry.clone()).with_context(|| {
                                    format!("nested field outside the §3.2 grammar: {entry}")
                                })?;
                            flatten_field(&sub)
                        })
                        .collect::<anyhow::Result<Vec<_>>>()
                })
                .collect::<anyhow::Result<Vec<_>>>()
        })
        .transpose()?;
    Ok(DetailsField {
        name: field.name.clone(),
        description: field.description.clone(),
        field_type: field.field_type.clone(),
        currency_code: field.currency_code().map(ToOwned::to_owned),
        value: field.value.clone(),
        values,
    })
}

/// `YYYY-MM-DD` as printed → [`NaiveDate`] (format pre-validated at parse).
fn parse_iso_date(raw: &str) -> anyhow::Result<NaiveDate> {
    NaiveDate::parse_from_str(raw, "%Y-%m-%d").with_context(|| format!("unparseable date {raw:?}"))
}

/// Nil-ULID placeholder for a not-yet-bound identity field.
fn unbound_id<T: std::str::FromStr>(what: &str) -> anyhow::Result<T>
where
    T::Err: std::fmt::Display,
{
    UNBOUND_ID
        .parse()
        .map_err(|e: T::Err| anyhow::anyhow!("unbound {what} id: {e}"))
}

/// Conformance filing ULID: prefix + interest id digits zero-padded to 19
/// (MANIFEST rule).
fn conformance_filing_id(interest_id: u32) -> anyhow::Result<FilingId> {
    format!("{CONFORMANCE_FILING_PREFIX}{interest_id:0>19}")
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance filing id for {interest_id}: {e}"))
}

/// Fixed conformance politician ULID keyed by the MNIS member id; unknown
/// members are refused (extend the MANIFEST table — never guess).
fn conformance_politician_id(member_id: u32) -> anyhow::Result<PoliticianId> {
    CONFORMANCE_POLITICIANS
        .iter()
        .find(|(known, _)| *known == member_id)
        .with_context(|| {
            format!("no conformance politician id for MNIS {member_id} — never guess")
        })?
        .1
        .parse()
        .map_err(|e| anyhow::anyhow!("conformance politician id for MNIS {member_id}: {e}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::json;

    use super::*;

    #[test]
    fn manifest_ulid_constants_are_valid_crockford() {
        assert_eq!(
            conformance_filing_id(15475).unwrap().to_string(),
            "0GBRFNG0000000000000015475"
        );
        assert_eq!(
            conformance_politician_id(4051).unwrap().to_string(),
            "0GBRMBR0000000000000000001"
        );
        assert_eq!(
            CONFORMANCE_REGIME_ID
                .parse::<RegimeId>()
                .unwrap()
                .to_string(),
            CONFORMANCE_REGIME_ID
        );
    }

    #[test]
    fn unknown_members_are_refused_not_guessed() {
        assert!(conformance_politician_id(9999).is_err());
    }

    fn staged(payload: serde_json::Value) -> StagingRow {
        StagingRow {
            payload,
            confidence: 1.0,
        }
    }

    fn shareholding_payload() -> serde_json::Value {
        json!({
            "interest_id": 15475,
            "row_ordinal": 1,
            "version": 0,
            "parent_interest_id": null,
            "category_id": 8,
            "category_number_raw": "7",
            "category_name_raw": "Shareholdings",
            "member_id": 4051,
            "member_name_raw": "John Glen",
            "member_list_name_raw": "Glen, John",
            "member_from_raw": "Salisbury",
            "party_raw": "Conservative",
            "house_raw": "Commons",
            "summary_raw": "Shares in Lockhouse Systems Limited",
            "registration_date_raw": "2026-06-17",
            "published_date_raw": "2026-06-17",
            "updated_dates_raw": [],
            "rectified": false,
            "rectified_details_raw": null,
            "fields_raw": [
                {"name": "ShareholdingThreshold", "description": "Registrable threshold",
                 "type": "String", "typeInfo": null,
                 "value": "(ii) Other shareholdings, valued at more than £70,000"},
                {"name": "HeldOnBehalfOf", "description": "Held jointly or on behalf of",
                 "type": "String", "typeInfo": null, "value": null}
            ],
            "extractor": "uk_commons_register/api@1"
        })
    }

    #[test]
    fn shareholding_maps_to_open_ended_interest_gold() {
        let candidate =
            normalize_row(&staged(shareholding_payload()), IdentityMode::Conformance).unwrap();
        candidate.validate().unwrap();
        assert_eq!(candidate.record_type, RecordType::Interest);
        assert_eq!(candidate.side, None, "side is NULL on interests");
        assert_eq!(candidate.transaction_date, None);
        assert_eq!(
            candidate.notified_date,
            Some(NaiveDate::from_ymd_opt(2026, 6, 17).unwrap())
        );
        let value = candidate.value.unwrap();
        assert_eq!(value.low(), dec!(70000.00));
        assert_eq!(value.high(), None, "open-ended threshold");
        assert_eq!(
            serde_json::to_value(candidate.owner).unwrap(),
            json!("self")
        );
        assert_eq!(candidate.instrument_id, None, "invariant 3");
        assert_eq!(
            candidate.details["value_source"],
            json!("shareholding_threshold")
        );
        assert_eq!(
            candidate.details["shareholding_threshold_raw"],
            json!("(ii) Other shareholdings, valued at more than £70,000")
        );
        assert_eq!(
            candidate.filing_id.to_string(),
            "0GBRFNG0000000000000015475"
        );
    }

    #[test]
    fn null_published_date_rejects_at_promotion() {
        let mut payload = shareholding_payload();
        payload["published_date_raw"] = json!(null);
        assert!(
            normalize_row(&staged(payload), IdentityMode::Conformance).is_err(),
            "§5: published_date required, null unobserved"
        );
    }

    #[test]
    fn pool_backed_mode_emits_unbound_identity() {
        let candidate =
            normalize_row(&staged(shareholding_payload()), IdentityMode::Unbound).unwrap();
        assert_eq!(candidate.filing_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.politician_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.regime_id.to_string(), UNBOUND_ID);
        assert_eq!(candidate.fingerprint, None, "computed at publish");
    }

    #[test]
    fn nested_donor_rows_flatten_recursively_into_details() {
        let field: Field = serde_json::from_value(json!({
            "name": "Donors", "description": null, "type": "Donor[]", "typeInfo": null,
            "value": null, "values": [
                [{"name": "Value", "description": "Donor value", "type": "Decimal",
                  "typeInfo": {"currencyCode": "GBP"}, "value": "1588.83"}]
            ]
        }))
        .unwrap();
        let flat = flatten_field(&field).unwrap();
        assert_eq!(flat.field_type, "Donor[]");
        assert_eq!(flat.currency_code, None);
        let rows = flat.values.unwrap();
        assert_eq!(rows[0][0].name, "Value");
        assert_eq!(rows[0][0].currency_code.as_deref(), Some("GBP"));
        assert_eq!(rows[0][0].values, None, "leaf fields carry null values");
    }
}
