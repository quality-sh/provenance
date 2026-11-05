//! The MCP tool schemas as JSON Schema 2020-12 documents.
//!
//! MCP reads a schema without `$schema` as 2020-12. The schema generator gives integer
//! and number formats (`uint32`, `int64`, `double`) that 2020-12 does not define, so
//! a validator that asserts formats refuses the schema. Each such format goes, and the
//! range of a fixed-width integer becomes `minimum` and `maximum`.

use serde_json::{json, Map, Value};

/// The formats that JSON Schema 2020-12 defines (Validation, section 7.3).
const FORMATS: &[&str] = &[
    "date-time",
    "date",
    "time",
    "duration",
    "email",
    "idn-email",
    "hostname",
    "idn-hostname",
    "ipv4",
    "ipv6",
    "uri",
    "uri-reference",
    "iri",
    "iri-reference",
    "uuid",
    "uri-template",
    "json-pointer",
    "relative-json-pointer",
    "regex",
];

/// Keywords whose values are instances, not schemas.
const INSTANCE_KEYWORDS: &[&str] = &["enum", "const", "default", "examples"];

/// Remove each format that 2020-12 does not define, and keep the range of a fixed-width
/// integer as `minimum` and `maximum`. A 64-bit range is wider than the integers that
/// a JSON number holds exactly, so an unsigned 64-bit integer keeps only `minimum: 0`.
pub fn to_2020_12(schema: &mut Value) {
    match schema {
        Value::Object(object) => {
            normalize_format(object);
            for (name, child) in object.iter_mut() {
                if !INSTANCE_KEYWORDS.contains(&name.as_str()) {
                    to_2020_12(child);
                }
            }
        }
        Value::Array(values) => values.iter_mut().for_each(to_2020_12),
        _ => {}
    }
}

fn normalize_format(object: &mut Map<String, Value>) {
    let Some(Value::String(format)) = object.get("format") else {
        return;
    };
    if FORMATS.contains(&format.as_str()) {
        return;
    }
    let (minimum, maximum) = match format.as_str() {
        "uint8" => (Some(json!(0)), Some(json!(u8::MAX))),
        "uint16" => (Some(json!(0)), Some(json!(u16::MAX))),
        "uint32" => (Some(json!(0)), Some(json!(u32::MAX))),
        "uint" | "uint64" | "uint128" => (Some(json!(0)), None),
        "int8" => (Some(json!(i8::MIN)), Some(json!(i8::MAX))),
        "int16" => (Some(json!(i16::MIN)), Some(json!(i16::MAX))),
        "int32" => (Some(json!(i32::MIN)), Some(json!(i32::MAX))),
        _ => (None, None),
    };
    object.remove("format");
    for (keyword, bound) in [("minimum", minimum), ("maximum", maximum)] {
        if let Some(bound) = bound {
            object.entry(keyword).or_insert(bound);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::to_2020_12;
    use serde_json::json;

    #[test]
    fn a_generator_format_becomes_a_range() {
        let mut schema = json!({
            "properties": {
                "count": {"type": "integer", "format": "uint32", "minimum": 0},
                "limit": {"type": ["integer", "null"], "format": "uint", "minimum": 1},
                "at": {"type": "integer", "format": "int64"},
                "share": {"type": "number", "format": "double"},
                "when": {"type": "string", "format": "date-time"}
            },
            "default": {"format": "uint"}
        });
        to_2020_12(&mut schema);
        assert_eq!(
            schema,
            json!({
                "properties": {
                    "count": {"type": "integer", "minimum": 0, "maximum": 4_294_967_295_u32},
                    "limit": {"type": ["integer", "null"], "minimum": 1},
                    "at": {"type": "integer"},
                    "share": {"type": "number"},
                    "when": {"type": "string", "format": "date-time"}
                },
                "default": {"format": "uint"}
            })
        );
    }
}
