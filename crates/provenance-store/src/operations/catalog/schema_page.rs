//! Per-operation page guarantees for exported response envelopes.
//!
//! The shared [`ResponseMeta`] carrier stays optional at runtime. The
//! exported schema does not project that breadth blindly: the operation's raw
//! success type decides which page facts the envelope meta requires, so a
//! paging response cannot pass validation with `meta: {}` while a
//! metadata-free member response stays valid.

use super::schema::resolve_schema;
use serde_json::{json, Value};

/// Restate the operation's own page guarantees in the exported `meta` schema.
/// The raw success type decides: when its producer requires `limit` and
/// `has_more`, the envelope meta requires them with the producer's own field
/// schemas, and `next_cursor` appears only when the operation's page carries
/// it, with the producer's nullability. Operations whose producers ship no
/// page facts keep a metadata-free meta.
pub(super) fn stamp_page_metadata(envelope: &mut Value, raw: &Value) {
    let Some((limit, has_more, next_cursor)) = page_facts(raw) else {
        return;
    };
    let Some(meta) = envelope
        .pointer_mut("/properties/meta")
        .and_then(Value::as_object_mut)
    else {
        panic!("envelope meta is missing for a paging response: {envelope}");
    };
    let Some(properties) = meta.get_mut("properties").and_then(Value::as_object_mut) else {
        panic!("envelope meta has no properties for a paging response: {meta:?}");
    };
    properties.remove("limit");
    properties.remove("has_more");
    properties.remove("next_cursor");
    properties.insert("limit".into(), limit);
    properties.insert("has_more".into(), has_more);
    if let Some(next_cursor) = next_cursor {
        properties.insert("next_cursor".into(), next_cursor);
    }
    meta.insert("required".into(), json!(["limit", "has_more"]));
}

/// The page facts one raw success type guarantees: its required non-null
/// `limit` and `has_more` fields, plus `next_cursor` when the producer
/// declares one. The transport lifts these fields from the payload object
/// itself (flattened query answers) or from its `result` member (resource
/// reads), so both holders are checked.
fn page_facts(raw: &Value) -> Option<(Value, Value, Option<Value>)> {
    let root = resolve_schema(raw, raw);
    let root_result = root
        .get("properties")
        .and_then(|properties| properties.get("result"))
        .map(|result| resolve_schema(raw, result));
    for holder in [Some(root), root_result].into_iter().flatten() {
        let Some(properties) = holder.get("properties") else {
            continue;
        };
        let (Some(limit), Some(has_more)) = (
            required_page_field(raw, holder, properties, "limit", "integer"),
            required_page_field(raw, holder, properties, "has_more", "boolean"),
        ) else {
            continue;
        };
        let next_cursor = optional_page_field(raw, properties, "next_cursor");
        return Some((limit, has_more, next_cursor));
    }
    None
}

fn required_page_field(
    raw: &Value,
    holder: &Value,
    properties: &Value,
    name: &str,
    kind: &str,
) -> Option<Value> {
    let is_required = holder
        .get("required")
        .and_then(Value::as_array)
        .is_some_and(|required| required.iter().any(|field| field == name));
    if !is_required {
        return None;
    }
    let field = resolve_schema(raw, properties.get(name)?);
    (field.get("type").and_then(Value::as_str) == Some(kind)).then(|| field.clone())
}

fn optional_page_field(raw: &Value, properties: &Value, name: &str) -> Option<Value> {
    let field = resolve_schema(raw, properties.get(name)?);
    let is_nullable = match field.get("type") {
        Some(Value::String(kind)) => kind == "null",
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .any(|kind| kind == "null"),
        _ => false,
    };
    is_nullable.then(|| field.clone())
}
