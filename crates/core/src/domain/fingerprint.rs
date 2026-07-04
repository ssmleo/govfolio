//! Idempotency fingerprint for `disclosure_record.fingerprint` (invariant 4:
//! idempotent Gold writes — `ON CONFLICT (fingerprint) DO NOTHING`).
//!
//! Determinism is the whole point: the same `(filing_id, ordinal, canonical content)`
//! must hash identically across runs, machines, and serializer whims. So content is
//! canonicalized first — object keys recursively sorted (never `HashMap`/insertion
//! order), compact serialization (no whitespace) — then sha256'd together with the
//! identity fields, NUL-separated to keep field boundaries unambiguous.
//!
//! Assumption (documented per plan Task 6): `serde_json`'s `arbitrary_precision`
//! feature is OFF. Content arrives as `serde_json::Value` from our own serializers,
//! where money is already decimal STRINGS (invariant 7) — so no float-formatting
//! drift can enter the hash through amounts. Plain JSON numbers (ordinals, counts)
//! serialize via ryu/itoa shortest round-trip, which is deterministic.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Deterministic sha256 fingerprint of one disclosure record, as 64 lowercase hex
/// characters. Same `(filing_id, ordinal, canonical content)` in → same string out,
/// regardless of JSON key order or whitespace in `content`.
#[must_use]
pub fn fingerprint(filing_id: &str, ordinal: u32, content: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(filing_id.as_bytes());
    hasher.update([0u8]); // field separator: "ab"+1 must not collide with "a"+…
    hasher.update(ordinal.to_be_bytes());
    hasher.update([0u8]);
    hasher.update(canonicalize(content).to_string().as_bytes());
    hex_lower(&hasher.finalize())
}

/// Rebuilds `value` with every object's keys in sorted order, recursively.
///
/// Explicit sort — not reliance on `serde_json::Map`'s default `BTreeMap` backing —
/// so the hash survives any future crate enabling the `preserve_order` feature
/// (cargo feature unification would flip the backing to insertion-ordered globally).
fn canonicalize(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_unstable_by(|a, b| a.0.cmp(b.0));
            let mut sorted = serde_json::Map::with_capacity(entries.len());
            for (key, val) in entries {
                sorted.insert(key.clone(), canonicalize(val));
            }
            Value::Object(sorted)
        }
        // Array order is significant JSON semantics — preserved, members canonicalized.
        Value::Array(items) => Value::Array(items.iter().map(canonicalize).collect()),
        other => other.clone(),
    }
}

/// Digest bytes → lowercase hex, no allocations beyond the output string.
fn hex_lower(digest: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(char::from(HEX[usize::from(byte >> 4)]));
        out.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use serde_json::{Value, json};

    use super::fingerprint;

    fn ptr_content() -> Value {
        json!({
            "asset_description_raw": "Apple Inc. (AAPL)",
            "side": "buy",
            "value": {"low": "1001.00", "high": "15000.00", "currency": "USD"}
        })
    }

    #[test]
    fn deterministic_64_lowercase_hex_across_runs() {
        let a = fingerprint("01ARZ3NDEKTSV4RRFFQ69G5FAV", 3, &ptr_content());
        let b = fingerprint("01ARZ3NDEKTSV4RRFFQ69G5FAV", 3, &ptr_content());
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')));
    }

    #[test]
    fn key_order_and_whitespace_do_not_alter_the_hash() {
        let compact: Value = serde_json::from_str(
            r#"{"value":{"high":"15000.00","currency":"USD","low":"1001.00"},"side":"buy","asset_description_raw":"Apple Inc. (AAPL)"}"#,
        )
        .unwrap();
        let airy: Value = serde_json::from_str(
            "{\n  \"asset_description_raw\" : \"Apple Inc. (AAPL)\" ,\n  \"side\" : \"buy\" ,\n  \"value\" : { \"low\" : \"1001.00\" ,\t\"high\" : \"15000.00\" , \"currency\" : \"USD\" }\n}",
        )
        .unwrap();
        assert_eq!(
            fingerprint("01ARZ3NDEKTSV4RRFFQ69G5FAV", 3, &compact),
            fingerprint("01ARZ3NDEKTSV4RRFFQ69G5FAV", 3, &airy),
        );
    }

    #[test]
    fn changing_value_low_alters_the_hash() {
        let mut tweaked = ptr_content();
        tweaked["value"]["low"] = json!("1002.00");
        assert_ne!(
            fingerprint("01ARZ3NDEKTSV4RRFFQ69G5FAV", 3, &ptr_content()),
            fingerprint("01ARZ3NDEKTSV4RRFFQ69G5FAV", 3, &tweaked),
        );
    }

    #[test]
    fn identity_fields_discriminate() {
        let base = fingerprint("01ARZ3NDEKTSV4RRFFQ69G5FAV", 3, &ptr_content());
        assert_ne!(
            base,
            fingerprint("01BX5ZZKBKACTAV9WEVGEMMVRZ", 3, &ptr_content()),
            "different filing_id must fingerprint differently"
        );
        assert_ne!(
            base,
            fingerprint("01ARZ3NDEKTSV4RRFFQ69G5FAV", 4, &ptr_content()),
            "different ordinal must fingerprint differently"
        );
    }

    #[test]
    fn array_order_is_significant() {
        // Arrays are ordered JSON semantics — canonicalization must NOT sort them.
        assert_ne!(
            fingerprint("F", 0, &json!({"items": [1, 2]})),
            fingerprint("F", 0, &json!({"items": [2, 1]})),
        );
    }
}
