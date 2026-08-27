//! Canonical bytes, and the hashes taken over them.
//!
//! A graph digest and a projection digest are claims about bytes somebody
//! else holds, so the bytes have to be produced the same way every time:
//! object keys sorted, no incidental whitespace, one SHA-256 over the
//! result. Two machines that agree on the graph must agree on the digit
//! string, or the pin means nothing.
//!
//! This is the one home of canonical serialization. `graph_reference` and
//! `cache` both hash through here; neither keeps a second copy.

use serde::Serialize;
use sha2::{Digest, Sha256};

pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let value = serde_json::to_value(value)?;
    let mut bytes = Vec::new();
    write_canonical_json(&value, &mut bytes)?;
    Ok(bytes)
}

fn write_canonical_json(
    value: &serde_json::Value,
    output: &mut Vec<u8>,
) -> Result<(), serde_json::Error> {
    match value {
        serde_json::Value::Object(map) => {
            output.push(b'{');
            let mut entries: Vec<_> = map.iter().collect();
            entries.sort_by_key(|(key, _)| key.as_str());
            for (index, (key, value)) in entries.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                output.extend(serde_json::to_vec(key)?);
                output.push(b':');
                write_canonical_json(value, output)?;
            }
            output.push(b'}');
        }
        serde_json::Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical_json(value, output)?;
            }
            output.push(b']');
        }
        _ => output.extend(serde_json::to_vec(value)?),
    }
    Ok(())
}

pub fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", sha256(bytes))
}

pub fn sha256(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let digest = Sha256::digest(bytes);
    digest.iter().fold(
        String::with_capacity(digest.len() * 2),
        |mut output, byte| {
            write!(output, "{byte:02x}").expect("writing to a String cannot fail");
            output
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn object_keys_are_sorted_and_whitespace_is_absent() {
        let bytes = canonical_bytes(&json!({"b": 1, "a": {"d": 2, "c": [3, 4]}})).unwrap();
        assert_eq!(bytes, br#"{"a":{"c":[3,4],"d":2},"b":1}"#);
    }

    #[test]
    fn digest_is_prefixed_sha256_of_bytes() {
        let bytes = canonical_bytes(&json!({"a": 1})).unwrap();
        assert_eq!(digest(&bytes), format!("sha256:{}", sha256(&bytes)));
    }

    #[test]
    fn equal_values_in_differing_key_order_hash_identically() {
        let left = canonical_bytes(&json!({"x": 1, "y": 2})).unwrap();
        let right = canonical_bytes(&json!({"y": 2, "x": 1})).unwrap();
        assert_eq!(sha256(&left), sha256(&right));
    }
}
