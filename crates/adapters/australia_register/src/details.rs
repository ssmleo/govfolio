//! The `(australia_register, interest)` and `(australia_register,
//! change_notification)` details contracts (regime doc §5, invariant 5). Doc
//! comments are contract surface: schemars embeds them as `description`, so
//! edits here must be re-snapshotted deliberately (schema-contracts skill).
//! Optional fields serialize as explicit nulls so the full contract surface is
//! visible on every candidate (fixtures `MANIFEST.json` `details_nulls`).

use std::collections::BTreeMap;

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Owner band of a Form A category entry, or the owner derived from an
/// alteration `Spouse -`/`Spouse gift -` marker (regime doc §3.5). Never
/// `joint` — the register records joint interests as the Member's own (§3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OwnerBand {
    /// The Member's own interest (`Self` band, or an unmarked alteration line).
    #[serde(rename = "self")]
    Self_,
    /// The `Spouse/Partner` band, or a `Spouse -`/`Spouse gift -` alteration line.
    Spouse,
    /// The `Dependent Children` band.
    Dependent,
}

/// Which §6 extraction path produced the row (provenance). The whole document
/// is one flavour: a clean text layer (deterministic cross-check available) or
/// scanned/vision-only (regime doc §3.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceFlavour {
    /// Clean, machine-readable text layer confirmed field-for-field (§3.9 case 2).
    TextLayer,
    /// Scanned image / vision-only extraction (§3.9 cases 1 & 3).
    ScannedVision,
}

/// The alteration axis of a Notification of Alteration (regime doc §3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AdditionDeletion {
    /// A newly notified interest.
    Addition,
    /// A notified removal of a prior interest.
    Deletion,
}

/// `details` payload of one Form A category entry (regime doc §5). No entry
/// carries a monetary value — the register is descriptive (§3.6), so `value`
/// is NULL on the Gold row and no amount lives here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AustraliaRegisterInterestDetailsV1 {
    /// Member document key, e.g. `Buchholz_48P` (threaded from the fetch URL).
    pub document_filename: String,
    /// Parliament number (e.g. `48`) from the URL `{np}`; the form stamp is raw.
    pub parliament_no: u32,
    /// 1-based row position across the whole compound document.
    #[schemars(range(min = 1))]
    pub row_ordinal: u32,
    /// 1-based source page the row was read from.
    #[schemars(range(min = 1))]
    pub page_ordinal: u32,
    /// Registrable-interest category number 1..14 (regime doc §3.2 census).
    #[schemars(range(min = 1, max = 14))]
    pub category_number: u32,
    /// Canonical §3.2 category heading (normalized across form-version wordings).
    pub category_name: String,
    /// Owner band, normalized (regime doc §3.5); null when unresolvable.
    pub owner_band: Option<OwnerBand>,
    /// Form-header `ELECTORAL DIVISION` verbatim — the §2.4 resolution key.
    pub electoral_division: String,
    /// Form-header `STATE` verbatim.
    pub state: String,
    /// The cell / `Details` text verbatim (regime doc §4, invariant 2).
    pub entry_text: String,
    /// Column-labelled parts of a multi-column grid (`{}` when single-column /
    /// unresolved) — lossless (regime doc §4 `entry_fields`).
    pub entry_fields: BTreeMap<String, String>,
    /// Statement completion date when legible (`DD/MM/YYYY` → ISO); null on
    /// alteration rows and when illegible (fail-soft, regime doc §3.7).
    pub statement_date: Option<NaiveDate>,
    /// Ingestion language: always `en` in v1 (regime doc §1).
    pub language: String,
    /// Which §6 extraction path produced the row.
    pub source_flavour: SourceFlavour,
}

/// `details` payload of one Notification-of-Alteration entry (regime doc §5):
/// every interest field PLUS the alteration axis, the notified date, and the
/// raw Parliament stamp.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AustraliaRegisterChangeNotificationDetailsV1 {
    /// The shared category surface (flattened to the same top-level keys);
    /// `statement_date` is null on alteration rows — the notified date lives in
    /// `submitted_date`.
    #[serde(flatten)]
    pub interest: AustraliaRegisterInterestDetailsV1,
    /// `ADDITION`/`DELETION` axis, normalized (regime doc §3.3).
    pub addition_deletion: AdditionDeletion,
    /// `Submitted Date`/legible `Date:` parsed `DD/MM/YYYY` (or `D/M/YY`) → ISO;
    /// null when handwritten-illegible (fail-soft, regime doc §3.7).
    pub submitted_date: Option<NaiveDate>,
    /// The `{NN}TH PARLIAMENT` stamp on the page, verbatim; null when the page
    /// bears no header (regime doc §3.3 mis-stamp quirk — stamp raw, URL wins).
    pub parliament_stamp: Option<String>,
}

/// JSON Schema for the `(australia_register, interest)` details contract.
#[must_use]
pub fn interest_details_schema() -> schemars::Schema {
    schemars::schema_for!(AustraliaRegisterInterestDetailsV1)
}

/// JSON Schema for the `(australia_register, change_notification)` details contract.
#[must_use]
pub fn change_notification_details_schema() -> schemars::Schema {
    schemars::schema_for!(AustraliaRegisterChangeNotificationDetailsV1)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sample_interest() -> AustraliaRegisterInterestDetailsV1 {
        AustraliaRegisterInterestDetailsV1 {
            document_filename: "Katter_48P".to_owned(),
            parliament_no: 48,
            row_ordinal: 1,
            page_ordinal: 2,
            category_number: 1,
            category_name: "Shareholdings in public and private companies".to_owned(),
            owner_band: Some(OwnerBand::Self_),
            electoral_division: "Kennedy".to_owned(),
            state: "QLD".to_owned(),
            entry_text: "A number of AMP shares".to_owned(),
            entry_fields: BTreeMap::from([(
                "company".to_owned(),
                "A number of AMP shares".to_owned(),
            )]),
            statement_date: None,
            language: "en".to_owned(),
            source_flavour: SourceFlavour::ScannedVision,
        }
    }

    #[test]
    fn optional_fields_serialize_as_explicit_nulls() {
        let value = serde_json::to_value(sample_interest()).unwrap();
        assert_eq!(value["owner_band"], json!("self"));
        assert_eq!(
            value["statement_date"],
            json!(null),
            "explicit null, never omitted"
        );
        assert_eq!(value["source_flavour"], json!("scanned_vision"));
        assert_eq!(value["language"], json!("en"));
        let object = value.as_object().unwrap();
        assert_eq!(object.len(), 14, "every contract field present: {value}");
    }

    #[test]
    fn change_notification_flattens_interest_and_adds_the_alteration_axis() {
        let cn = AustraliaRegisterChangeNotificationDetailsV1 {
            interest: AustraliaRegisterInterestDetailsV1 {
                statement_date: None,
                ..sample_interest()
            },
            addition_deletion: AdditionDeletion::Addition,
            submitted_date: NaiveDate::from_ymd_opt(2026, 2, 25),
            parliament_stamp: Some("48TH PARLIAMENT".to_owned()),
        };
        let value = serde_json::to_value(&cn).unwrap();
        // Interest fields sit at the top level (flatten), plus the three extras.
        assert_eq!(value["document_filename"], json!("Katter_48P"));
        assert_eq!(value["statement_date"], json!(null));
        assert_eq!(value["addition_deletion"], json!("addition"));
        assert_eq!(value["submitted_date"], json!("2026-02-25"));
        assert_eq!(value["parliament_stamp"], json!("48TH PARLIAMENT"));
        assert_eq!(value.as_object().unwrap().len(), 17);
    }

    #[test]
    fn interest_schema_requires_the_mandatory_fields() {
        let schema = serde_json::to_value(interest_details_schema()).unwrap();
        let required = schema["required"].as_array().unwrap();
        for field in [
            "document_filename",
            "parliament_no",
            "row_ordinal",
            "page_ordinal",
            "category_number",
            "category_name",
            "electoral_division",
            "state",
            "entry_text",
            "entry_fields",
            "language",
            "source_flavour",
        ] {
            assert!(
                required.contains(&json!(field)),
                "{field} must be required: {schema}"
            );
        }
        // owner_band + statement_date stay optional (nullable, §3.5/§3.7).
        for field in ["owner_band", "statement_date"] {
            assert!(
                !required.contains(&json!(field)),
                "{field} must stay optional: {schema}"
            );
        }
    }
}
