use provenance_core::protocol::failure::ErasedFailure;
use provenance_store::operations::catalog::{Definition, ResponseAdapter, ResponseBinding};
use serde_json::{json, Map, Value};

pub fn success(
    mut value: Value,
    definition: &Definition,
    binding: &ResponseBinding,
) -> Result<Value, ErasedFailure> {
    let mut meta = Map::new();
    if let Some(object) = value.as_object_mut() {
        move_meta(object, &mut meta);
        object.remove("protocol_version");
        object.remove("operation");
    }
    let data = match binding.adapter {
        ResponseAdapter::Direct => value,
        ResponseAdapter::Result => take_result(value, definition, &mut meta)?,
        ResponseAdapter::ArrayItems => {
            if !value.is_array() {
                return Err(malformed(definition));
            }
            json!({"items":value})
        }
        ResponseAdapter::ObjectItems(field) => {
            let items = value
                .as_object_mut()
                .and_then(|object| object.remove(field))
                .ok_or_else(|| malformed(definition))?;
            if !items.is_array() {
                return Err(malformed(definition));
            }
            json!({"items":items})
        }
        ResponseAdapter::ResultItems(field) => {
            let mut result = take_result(value, definition, &mut meta)?;
            let items = result
                .as_object_mut()
                .and_then(|object| object.remove(field))
                .ok_or_else(|| malformed(definition))?;
            if !items.is_array() {
                return Err(malformed(definition));
            }
            json!({"items":items})
        }
    };
    Ok(json!({"data":data,"meta":meta}))
}

fn take_result(
    mut value: Value,
    definition: &Definition,
    meta: &mut Map<String, Value>,
) -> Result<Value, ErasedFailure> {
    let result = value
        .as_object_mut()
        .and_then(|object| take_field(object, "result"))
        .ok_or_else(|| malformed(definition))?;
    let mut result = result;
    if let Some(object) = result.as_object_mut() {
        move_page_meta(object, meta);
    }
    Ok(result)
}

fn take_field(object: &mut Map<String, Value>, field: &str) -> Option<Value> {
    object.remove(field)
}

fn malformed(definition: &Definition) -> ErasedFailure {
    ErasedFailure::new(
        Some(definition.name),
        provenance_core::protocol::failure::OperationFailure::Internal,
    )
}

fn move_meta(object: &mut Map<String, Value>, meta: &mut Map<String, Value>) {
    for field in [
        "stamp",
        "freshness_error",
        "freshness_cause",
        "limit",
        "has_more",
        "next_cursor",
    ] {
        if let Some(value) = object.remove(field) {
            meta.insert(field.into(), value);
        }
    }
}

fn move_page_meta(object: &mut Map<String, Value>, meta: &mut Map<String, Value>) {
    for field in ["limit", "has_more", "next_cursor"] {
        if let Some(value) = object.remove(field) {
            meta.insert(field.into(), value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_core::protocol::{QUERY_RESPONSE_BYTES, SDK_PROTOCOL_VERSION};
    use provenance_store::operations::catalog;

    fn search_binding() -> (&'static catalog::Definition, catalog::ResponseBinding) {
        let definition = catalog::definitions()
            .into_iter()
            .find(|definition| definition.path == "/rules")
            .unwrap();
        let binding = definition
            .registration
            .queries
            .iter()
            .find(|query| query.name == "search")
            .unwrap()
            .response
            .clone();
        (definition, binding)
    }

    fn raw_search(payload: String) -> Value {
        json!({
            "protocol_version": SDK_PROTOCOL_VERSION,
            "operation": "search",
            "stamp": {
                "serial": 1,
                "digest": "digest",
                "instance_id": "instance",
                "derivation": 1,
                "policy": "catch_up_failed",
                "attested": ["rules"],
                "live": []
            },
            "freshness_error": "catch-up failed; answer uses the stored projection",
            "freshness_cause": "catch_up_failed",
            "next_cursor": null,
            "limit": 50,
            "has_more": false,
            "nodes": [{"node_type": "rule", "id": "rule_large", "statement": payload}]
        })
    }

    #[test]
    fn finalized_public_envelope_accepts_the_limit_and_refuses_one_byte_more() {
        let (definition, binding) = search_binding();
        let empty = success(raw_search(String::new()), definition, &binding).unwrap();
        let remaining = QUERY_RESPONSE_BYTES - empty.bytes().len();

        let at_limit = success(raw_search("x".repeat(remaining)), definition, &binding).unwrap();
        assert_eq!(at_limit.bytes().len(), QUERY_RESPONSE_BYTES);
        assert_eq!(
            at_limit.value()["meta"]["freshness_cause"],
            "catch_up_failed"
        );

        let failure = success(raw_search("x".repeat(remaining + 1)), definition, &binding)
            .unwrap_err();
        assert_eq!(failure.status_code(), 409);
        assert_eq!(failure.error, json!({"kind": "page_budget_exceeded"}));
        assert!(serde_json::to_vec(&failure).unwrap().len() <= QUERY_RESPONSE_BYTES);
    }
}
