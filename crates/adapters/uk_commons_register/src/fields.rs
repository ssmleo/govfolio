//! Field grammar (regime doc §3.2) and the normative per-category maps:
//! §3.1 category scheme, §3.4 value rules R1–R4, §3.5 owner map. Shared by
//! parse (hard-reject validation + confidence scoring) and normalize (Gold
//! mapping) so the two stages cannot drift. Unknown vocabulary is never
//! guessed: monetary grammar gaps are hard rejects (invariant 6), owner
//! grammar gaps degrade to `unknown` for the review lane (§3.5).

use anyhow::Context as _;
use rust_decimal::Decimal;
use serde::Deserialize;

use govfolio_core::domain::enums::{AssetClass, Currency, Owner};
use govfolio_core::domain::value::ValueInterval;

use crate::details::ValueSource;

/// Typed view of one `fields[]` entry (regime doc §3.2 grammar: `{name,
/// description, type, typeInfo, value, values}`). Parsed FROM the verbatim
/// JSON the Silver row stores — the raw array is never re-serialized from
/// this view (raw is sacred). `deny_unknown_fields`: an extra key inside a
/// field entry is contract drift and must surface as a freeze (§6.4), while
/// the field NAME vocabulary stays open by design.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
// `field_type` mirrors the §3.2 grammar token `type` (a Rust keyword) and the
// details-contract field name — the postfix IS the regime-doc vocabulary.
#[allow(clippy::struct_field_names)]
pub(crate) struct Field {
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) description: Option<String>,
    #[serde(rename = "type")]
    pub(crate) field_type: String,
    #[serde(rename = "typeInfo", default)]
    pub(crate) type_info: Option<TypeInfo>,
    #[serde(default)]
    pub(crate) value: serde_json::Value,
    /// Nested rows of complex fields (`Donor[]`, `VisitLocation[]`, §3.2);
    /// absent on flat fields — the five pinned documents carry no `values`
    /// key at all.
    #[serde(default)]
    pub(crate) values: Option<Vec<Vec<serde_json::Value>>>,
}

/// `typeInfo` payload: only `currencyCode` is known (§3.2 — its presence,
/// never the field name, is what makes a `Decimal` money).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TypeInfo {
    #[serde(rename = "currencyCode", default)]
    pub(crate) currency_code: Option<String>,
}

impl Field {
    /// The field's currency code, when its `typeInfo` declares one.
    pub(crate) fn currency_code(&self) -> Option<&str> {
        self.type_info
            .as_ref()
            .and_then(|info| info.currency_code.as_deref())
    }

    /// True when this field is money: `Decimal` WITH a `currencyCode`
    /// (§3.2 — `HoursWorked` is a `Decimal` without one and is NOT money).
    pub(crate) fn is_money(&self) -> bool {
        self.field_type == "Decimal" && self.currency_code().is_some()
    }
}

/// Parses the verbatim `fields` array into the typed §3.2 view.
pub(crate) fn typed_fields(fields_raw: &serde_json::Value) -> anyhow::Result<Vec<Field>> {
    let entries = fields_raw
        .as_array()
        .context("`fields` is not a JSON array — contract drift, freeze (§6.4)")?;
    entries
        .iter()
        .map(|entry| {
            serde_json::from_value(entry.clone())
                .with_context(|| format!("field entry outside the §3.2 grammar: {entry}"))
        })
        .collect()
}

/// §3.1 category scheme: the closed set of known API category ids. An
/// interest in an unknown category is a rules change — freeze + review
/// (§3.8 check 2).
pub(crate) const KNOWN_CATEGORY_IDS: &[u32] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

/// API category id of "7 Shareholdings" (§3.1: id 8 = number "7").
const CATEGORY_SHAREHOLDINGS: u32 = 8;
/// API category id of "6 Land and property" (§3.1: id 7 = number "6").
const CATEGORY_LAND_AND_PROPERTY: u32 = 7;
/// API category id of "4 Visits outside the UK" (§3.1: id 5 = number "4").
const CATEGORY_VISITS: u32 = 5;
/// API category ids of "9 Family members employed" / "10 Family members
/// engaged in third-party lobbying" (§3.1: ids 10 and 11).
const CATEGORY_FAMILY: &[u32] = &[10, 11];

/// §3.1 `asset_class` map — honest, no creative bucketing: only
/// shareholdings are `equity`, only land/property is `real_estate`.
pub(crate) fn asset_class_for_category(category_id: u32) -> AssetClass {
    match category_id {
        CATEGORY_SHAREHOLDINGS => AssetClass::Equity,
        CATEGORY_LAND_AND_PROPERTY => AssetClass::RealEstate,
        _ => AssetClass::Other,
    }
}

/// The §3.4 R2 shareholding-threshold grammar: only the two archived strings
/// are accepted; anything else is a hard reject (R2c, fail closed).
const THRESHOLD_OPEN_ENDED: &str = "(ii) Other shareholdings, valued at more than £70,000";
const THRESHOLD_OPEN_ENDED_LOW: &str = "70000.00";
const THRESHOLD_PERCENTAGE: &str = "(i) Shareholdings: over 15% of issued share capital";

/// Outcome of the §3.4 value rules for one record.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ValueOutcome {
    /// The Gold `value`; `None` is the categorical/no-value case.
    pub(crate) interval: Option<ValueInterval>,
    /// Which rule fired (details provenance).
    pub(crate) source: ValueSource,
    /// Number of donor rows that carried money (R3) — drives the §6
    /// multi-donor confidence penalty.
    pub(crate) donor_money_rows: usize,
}

/// Applies the §3.4 value rules in order; first match wins.
///
/// # Errors
/// Hard rejects (invariant 6): R2c unknown threshold string, unmapped
/// `currencyCode`, mixed-currency donor sum, a `Decimal` value that is not a
/// JSON string, malformed donor rows.
pub(crate) fn evaluate_value(category_id: u32, fields: &[Field]) -> anyhow::Result<ValueOutcome> {
    // R1: top-level typed `Value` field — Decimal with a currencyCode.
    if let Some(field) = fields.iter().find(|f| f.name == "Value" && f.is_money()) {
        let amount = decimal_value(field)?;
        let currency = map_currency(field)?;
        let interval = ValueInterval::new(amount, Some(amount), currency)
            .map_err(|e| anyhow::anyhow!("R1 exact value: {e}"))?;
        return Ok(ValueOutcome {
            interval: Some(interval),
            source: ValueSource::ValueField,
            donor_money_rows: 0,
        });
    }

    // R2: category 7 shareholdings — threshold string grammar, fail closed.
    if category_id == CATEGORY_SHAREHOLDINGS {
        return evaluate_shareholding_threshold(fields);
    }

    // R3: category 4 visits — deterministic sum over the declared exact
    // per-donor amounts. Zero donor money falls through to R4.
    if category_id == CATEGORY_VISITS
        && let Some(outcome) = evaluate_donor_sum(fields)?
    {
        return Ok(outcome);
    }

    // R4: no money field at all — value stays NULL, never inferred.
    Ok(ValueOutcome {
        interval: None,
        source: ValueSource::None,
        donor_money_rows: 0,
    })
}

/// §3.4 R2a/R2b/R2c: the `ShareholdingThreshold` grammar.
fn evaluate_shareholding_threshold(fields: &[Field]) -> anyhow::Result<ValueOutcome> {
    let threshold = shareholding_threshold(fields).with_context(
        || "category 7 without a ShareholdingThreshold string — hard reject (R2c, fail closed)",
    )?;
    match threshold {
        THRESHOLD_OPEN_ENDED => {
            let low: Decimal = THRESHOLD_OPEN_ENDED_LOW
                .parse()
                .map_err(|e| anyhow::anyhow!("R2a threshold low: {e}"))?;
            let interval = ValueInterval::new(low, None, Currency::GBP)
                .map_err(|e| anyhow::anyhow!("R2a open-ended interval: {e}"))?;
            Ok(ValueOutcome {
                interval: Some(interval),
                source: ValueSource::ShareholdingThreshold,
                donor_money_rows: 0,
            })
        }
        THRESHOLD_PERCENTAGE => Ok(ValueOutcome {
            interval: None,
            source: ValueSource::None,
            donor_money_rows: 0,
        }),
        other => anyhow::bail!(
            "ShareholdingThreshold {other:?} outside the archived grammar — hard reject \
             (R2c, fail closed)"
        ),
    }
}

/// The category-7 `ShareholdingThreshold` string, when present and non-null.
pub(crate) fn shareholding_threshold(fields: &[Field]) -> Option<&str> {
    fields
        .iter()
        .find(|f| f.name == "ShareholdingThreshold")
        .and_then(|f| f.value.as_str())
}

/// §3.4 R3: sum of per-donor exact amounts. `Ok(None)` when no donor row
/// carries money (falls through to R4).
fn evaluate_donor_sum(fields: &[Field]) -> anyhow::Result<Option<ValueOutcome>> {
    let mut sum = Decimal::ZERO;
    let mut currency: Option<Currency> = None;
    let mut donor_money_rows = 0usize;
    for field in fields.iter().filter(|f| f.field_type == "Donor[]") {
        for row in field.values.as_deref().unwrap_or_default() {
            for entry in row {
                let sub: Field = serde_json::from_value(entry.clone()).with_context(|| {
                    format!("donor sub-field outside the §3.2 grammar: {entry}")
                })?;
                if !(sub.name == "Value" && sub.is_money()) {
                    continue;
                }
                let amount = decimal_value(&sub)?;
                let donor_currency = map_currency(&sub)?;
                if let Some(seen) = currency {
                    anyhow::ensure!(
                        seen == donor_currency,
                        "mixed donor currencies ({seen:?} vs {donor_currency:?}) — hard reject (R3)"
                    );
                } else {
                    currency = Some(donor_currency);
                }
                sum += amount;
                donor_money_rows += 1;
            }
        }
    }
    let Some(currency) = currency else {
        return Ok(None);
    };
    let interval = ValueInterval::new(sum, Some(sum), currency)
        .map_err(|e| anyhow::anyhow!("R3 donor sum: {e}"))?;
    Ok(Some(ValueOutcome {
        interval: Some(interval),
        source: ValueSource::SumOfDonors,
        donor_money_rows,
    }))
}

/// A money field's `Decimal` value — a JSON string per the §3.2 table; any
/// other JSON type is contract drift (hard reject, §6.4).
fn decimal_value(field: &Field) -> anyhow::Result<Decimal> {
    let raw = field.value.as_str().with_context(|| {
        format!(
            "Decimal field {:?} carries a non-string value {} — outside the §3.2 table, \
             hard reject",
            field.name, field.value
        )
    })?;
    raw.parse()
        .map_err(|e| anyhow::anyhow!("Decimal field {:?} value {raw:?}: {e}", field.name))
}

/// `currencyCode` → the core [`Currency`] enum; an unmapped code is a hard
/// reject (§3.4 — only GBP observed).
fn map_currency(field: &Field) -> anyhow::Result<Currency> {
    let code = field
        .currency_code()
        .with_context(|| format!("field {:?} has no currencyCode", field.name))?;
    serde_json::from_value(serde_json::Value::String(code.to_owned())).with_context(|| {
        format!("currencyCode {code:?} outside the core Currency enum — hard reject (§3.4)")
    })
}

/// §3.5 owner map. Grammar gaps (unobserved `HeldOnBehalfOf` vocabulary,
/// missing `IsSoleOwner`) degrade to `unknown` — the review lane at
/// promotion — never a guess.
pub(crate) fn owner_for_category(category_id: u32, fields: &[Field]) -> Option<Owner> {
    if CATEGORY_FAMILY.contains(&category_id) {
        // Categories 9/10: no asset of the Member — the subject is a third party.
        return None;
    }
    if category_id == CATEGORY_SHAREHOLDINGS {
        let held_on_behalf_of = fields
            .iter()
            .find(|f| f.name == "HeldOnBehalfOf")
            .map(|f| f.value.is_null());
        return match held_on_behalf_of {
            Some(true) => Some(Owner::Self_),
            // Non-null value vocabulary is unobserved; a missing field is
            // outside the archived shape — both stay `unknown` (fail closed).
            Some(false) | Option::None => Some(Owner::Unknown),
        };
    }
    if category_id == CATEGORY_LAND_AND_PROPERTY {
        let is_sole_owner = fields
            .iter()
            .find(|f| f.name == "IsSoleOwner")
            .and_then(|f| f.value.as_bool());
        return match is_sole_owner {
            Some(true) => Some(Owner::Self_),
            Some(false) => Some(Owner::Joint),
            Option::None => Some(Owner::Unknown),
        };
    }
    // All other categories: the registered interest is the Member's own.
    Some(Owner::Self_)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rust_decimal_macros::dec;
    use serde_json::json;

    use super::*;

    fn fields_of(value: &serde_json::Value) -> Vec<Field> {
        typed_fields(value).unwrap()
    }

    #[test]
    fn r1_exact_value_keys_on_currency_code_not_field_name() {
        // HoursWorked is a Decimal WITHOUT currencyCode — never money (§3.2).
        let fields = fields_of(&json!([
            {"name": "HoursWorked", "description": null, "type": "Decimal",
             "typeInfo": null, "value": "5.00"},
            {"name": "Value", "description": null, "type": "Decimal",
             "typeInfo": {"currencyCode": "GBP"}, "value": "500.00"}
        ]));
        let outcome = evaluate_value(1, &fields).unwrap();
        let interval = outcome.interval.unwrap();
        assert_eq!(interval.low(), dec!(500.00));
        assert_eq!(interval.high().unwrap(), dec!(500.00));
        assert_eq!(interval.currency(), Currency::GBP);
        assert_eq!(outcome.source, ValueSource::ValueField);
    }

    #[test]
    fn r1_rejects_unmapped_currency_and_non_string_decimal() {
        let unmapped = fields_of(&json!([
            {"name": "Value", "description": null, "type": "Decimal",
             "typeInfo": {"currencyCode": "JPY"}, "value": "500.00"}
        ]));
        assert!(evaluate_value(1, &unmapped).is_err(), "unmapped currency");
        let non_string = fields_of(&json!([
            {"name": "Value", "description": null, "type": "Decimal",
             "typeInfo": {"currencyCode": "GBP"}, "value": 500.0}
        ]));
        assert!(
            evaluate_value(1, &non_string).is_err(),
            "non-string Decimal"
        );
    }

    #[test]
    fn r2_threshold_grammar_is_closed() {
        let open_ended = fields_of(&json!([
            {"name": "ShareholdingThreshold", "description": null, "type": "String",
             "typeInfo": null, "value": "(ii) Other shareholdings, valued at more than £70,000"}
        ]));
        let outcome = evaluate_value(8, &open_ended).unwrap();
        let interval = outcome.interval.unwrap();
        assert_eq!(interval.low(), dec!(70000.00));
        assert_eq!(interval.high(), None, "open-ended");
        assert_eq!(outcome.source, ValueSource::ShareholdingThreshold);

        let percentage = fields_of(&json!([
            {"name": "ShareholdingThreshold", "description": null, "type": "String",
             "typeInfo": null, "value": "(i) Shareholdings: over 15% of issued share capital"}
        ]));
        let outcome = evaluate_value(8, &percentage).unwrap();
        assert_eq!(outcome.interval, None, "R2b: percentage has no money value");
        assert_eq!(outcome.source, ValueSource::None);

        let unknown = fields_of(&json!([
            {"name": "ShareholdingThreshold", "description": null, "type": "String",
             "typeInfo": null, "value": "(iii) Some new threshold"}
        ]));
        assert!(evaluate_value(8, &unknown).is_err(), "R2c hard reject");
        assert!(evaluate_value(8, &[]).is_err(), "missing threshold field");
    }

    #[test]
    fn r3_donor_sum_is_deterministic_and_rejects_mixed_currencies() {
        let donors = fields_of(&json!([
            {"name": "Donors", "description": null, "type": "Donor[]", "typeInfo": null,
             "value": null, "values": [
                [{"name": "Name", "description": null, "type": "String",
                  "typeInfo": null, "value": "A"},
                 {"name": "Value", "description": null, "type": "Decimal",
                  "typeInfo": {"currencyCode": "GBP"}, "value": "1588.83"}],
                [{"name": "Value", "description": null, "type": "Decimal",
                  "typeInfo": {"currencyCode": "GBP"}, "value": "100.17"}]
             ]}
        ]));
        let outcome = evaluate_value(5, &donors).unwrap();
        let interval = outcome.interval.unwrap();
        assert_eq!(interval.low(), dec!(1689.00));
        assert_eq!(interval.high().unwrap(), dec!(1689.00));
        assert_eq!(outcome.source, ValueSource::SumOfDonors);
        assert_eq!(outcome.donor_money_rows, 2);

        let mixed = fields_of(&json!([
            {"name": "Donors", "description": null, "type": "Donor[]", "typeInfo": null,
             "value": null, "values": [
                [{"name": "Value", "description": null, "type": "Decimal",
                  "typeInfo": {"currencyCode": "GBP"}, "value": "10.00"}],
                [{"name": "Value", "description": null, "type": "Decimal",
                  "typeInfo": {"currencyCode": "USD"}, "value": "10.00"}]
             ]}
        ]));
        assert!(
            evaluate_value(5, &mixed).is_err(),
            "mixed currencies reject"
        );
    }

    #[test]
    fn r4_no_money_means_null_value_never_inferred() {
        let land = fields_of(&json!([
            {"name": "RegistrableRentalIncome", "description": null, "type": "Boolean",
             "typeInfo": null, "value": true}
        ]));
        let outcome = evaluate_value(7, &land).unwrap();
        assert_eq!(outcome.interval, None);
        assert_eq!(outcome.source, ValueSource::None);
    }

    #[test]
    fn owner_map_follows_the_regime_doc() {
        let held_null = fields_of(&json!([
            {"name": "HeldOnBehalfOf", "description": null, "type": "String",
             "typeInfo": null, "value": null}
        ]));
        assert_eq!(owner_for_category(8, &held_null), Some(Owner::Self_));
        let held_set = fields_of(&json!([
            {"name": "HeldOnBehalfOf", "description": null, "type": "String",
             "typeInfo": null, "value": "spouse"}
        ]));
        assert_eq!(
            owner_for_category(8, &held_set),
            Some(Owner::Unknown),
            "unobserved vocabulary stays unknown, never guessed"
        );
        let sole = fields_of(&json!([
            {"name": "IsSoleOwner", "description": null, "type": "Boolean",
             "typeInfo": null, "value": false}
        ]));
        assert_eq!(owner_for_category(7, &sole), Some(Owner::Joint));
        assert_eq!(owner_for_category(10, &[]), None, "family: third party");
        assert_eq!(owner_for_category(11, &[]), None);
        assert_eq!(owner_for_category(3, &[]), Some(Owner::Self_));
    }

    #[test]
    fn unknown_field_keys_are_contract_drift() {
        let drifted = json!([
            {"name": "X", "description": null, "type": "String",
             "typeInfo": null, "value": null, "surprise": 1}
        ]);
        assert!(
            typed_fields(&drifted).is_err(),
            "deny_unknown_fields (§6.4)"
        );
    }
}
