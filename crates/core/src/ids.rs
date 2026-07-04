//! ULID newtypes (design §4.2: "ULIDs stored as text: time-sortable, URL-safe,
//! no coordination"). Serialized as their canonical 26-char text form everywhere.

use std::fmt;
use std::str::FromStr;

use ulid::Ulid;

macro_rules! define_ulid_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            PartialOrd,
            Ord,
            serde::Serialize,
            serde::Deserialize,
        )]
        pub struct $name(Ulid);

        impl $name {
            /// Mints a fresh, time-sortable id.
            #[must_use]
            pub fn generate() -> Self {
                Self(Ulid::new())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = ulid::DecodeError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Ulid::from_string(s)?))
            }
        }

        impl schemars::JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Borrowed(stringify!($name))
            }

            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({
                    "type": "string",
                    "description": "ULID (26-char Crockford base32, time-sortable)",
                    "pattern": "^[0-7][0-9A-HJKMNP-TV-Z]{25}$"
                })
            }
        }
    };
}

define_ulid_id!(
    FilingId,
    "`filing.id` — the source filing a record came from."
);
define_ulid_id!(PoliticianId, "`politician.id`.");
define_ulid_id!(RegimeId, "`disclosure_regime.id`.");
define_ulid_id!(
    InstrumentId,
    "`instrument.id` — a resolved instrument (never guessed; see invariant 3)."
);

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn ids_round_trip_as_text() {
        let id: FilingId = "01ARZ3NDEKTSV4RRFFQ69G5FAV".parse().unwrap();
        assert_eq!(id.to_string(), "01ARZ3NDEKTSV4RRFFQ69G5FAV");
        let json = serde_json::to_value(id).unwrap();
        assert_eq!(json, serde_json::json!("01ARZ3NDEKTSV4RRFFQ69G5FAV"));
        let back: FilingId = serde_json::from_value(json).unwrap();
        assert_eq!(back, id);
    }

    #[test]
    fn ids_reject_non_ulid_text() {
        assert!("not-a-ulid".parse::<PoliticianId>().is_err());
    }

    #[test]
    fn distinct_id_types_do_not_mix() {
        // Compile-time property really; assert the newtypes exist independently.
        let f = FilingId::generate();
        let p = PoliticianId::generate();
        assert_ne!(f.to_string(), String::new());
        assert_ne!(p.to_string(), String::new());
    }
}
