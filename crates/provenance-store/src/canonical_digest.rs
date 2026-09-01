//! Canonical bytes, and the hashes taken over them.
//!
//! A digest is a claim about bytes somebody else holds, so the bytes have to
//! be produced the same way every time: object keys sorted, no incidental
//! whitespace, one SHA-256 over the result. Two machines that agree on the
//! records must agree on the digit string, or the pin means nothing.
//!
//! This module is the one home for canonical serialization in the crate. The
//! graph-reference digest and the projection digest both hash bytes from
//! here; neither keeps a private copy.

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
    use super::{canonical_bytes, digest, sha256};

    #[test]
    fn canonical_bytes_sort_object_keys_recursively_without_whitespace() {
        let value = serde_json::json!({
            "b": {"z": 1, "a": [ {"k": 2, "c": 3} ]},
            "a": "text",
        });
        let bytes = canonical_bytes(&value).unwrap();
        assert_eq!(
            String::from_utf8(bytes).unwrap(),
            r#"{"a":"text","b":{"a":[{"c":3,"k":2}],"z":1}}"#
        );
    }

    #[test]
    fn digest_prefixes_the_sha256_hex_of_the_bytes() {
        assert_eq!(
            digest(b"abc"),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha256_returns_lowercase_hex_without_prefix() {
        assert_eq!(
            sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
