use provenance_core::protocol::failure::ErasedFailure;
use provenance_core::protocol::{read_failure::ReadFailure, QUERY_RESPONSE_BYTES};
use provenance_store::operations::catalog::{Definition, ResponseAdapter, ResponseBinding};
use serde_json::{json, Map, Value};
use std::io::{self, Write};

#[derive(Debug)]
pub struct Success {
    value: Value,
    bytes: Vec<u8>,
}

impl Success {
    pub const fn value(&self) -> &Value {
        &self.value
    }

    pub fn into_value(self) -> Value {
        self.value
    }

    #[cfg(test)]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

pub(super) fn success(
    mut value: Value,
    definition: &Definition,
    binding: &ResponseBinding,
    query_response: bool,
) -> Result<Success, ErasedFailure> {
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
    let value = json!({"data":data,"meta":meta});
    let bytes = encode(definition.name, &value, query_response)?;
    Ok(Success { value, bytes })
}

fn encode(
    operation: &'static str,
    value: &Value,
    query_response: bool,
) -> Result<Vec<u8>, ErasedFailure> {
    let mut bytes = EncodedBytes {
        bytes: Vec::new(),
        limit: query_response.then_some(QUERY_RESPONSE_BYTES),
    };
    match serde_json::to_writer(&mut bytes, value) {
        Ok(()) => Ok(bytes.bytes),
        Err(error) if error.is_io() => Err(ErasedFailure::declared(
            operation,
            ReadFailure::PageBudgetExceeded,
            409,
        )),
        Err(_) => Err(ErasedFailure::new(
            Some(operation),
            provenance_core::protocol::failure::OperationFailure::Internal,
        )),
    }
}

struct EncodedBytes {
    bytes: Vec<u8>,
    limit: Option<usize>,
}

impl Write for EncodedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .limit
            .is_some_and(|limit| self.bytes.len().saturating_add(bytes.len()) > limit)
        {
            return Err(io::Error::other("query response budget"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
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
            .iter()
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

    fn raw_search(payload: &str) -> Value {
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

    fn payload_bytes_at_limit(
        definition: &catalog::Definition,
        binding: &catalog::ResponseBinding,
    ) -> usize {
        let empty = success(raw_search(""), definition, binding, true).unwrap();
        QUERY_RESPONSE_BYTES - empty.bytes().len()
    }

    #[test]
    fn finalized_public_envelope_accepts_the_exact_limit() {
        let (definition, binding) = search_binding();
        let remaining = payload_bytes_at_limit(definition, &binding);

        let at_limit = success(
            raw_search(&"x".repeat(remaining)),
            definition,
            &binding,
            true,
        )
        .unwrap();
        assert_eq!(at_limit.bytes().len(), QUERY_RESPONSE_BYTES);
        assert_eq!(
            at_limit.value()["meta"]["freshness_cause"],
            "catch_up_failed"
        );
    }

    #[test]
    fn finalized_public_envelope_refuses_one_byte_over_the_limit() {
        let (definition, binding) = search_binding();
        let remaining = payload_bytes_at_limit(definition, &binding);

        let failure = success(
            raw_search(&"x".repeat(remaining + 1)),
            definition,
            &binding,
            true,
        )
        .unwrap_err();
        assert_eq!(failure.status_code(), 409);
        assert_eq!(failure.error, json!({"kind": "page_budget_exceeded"}));
        assert!(serde_json::to_vec(&failure).unwrap().len() <= QUERY_RESPONSE_BYTES);
    }

    #[test]
    fn ordinary_public_envelope_does_not_use_the_query_limit() {
        let (definition, binding) = search_binding();
        let remaining = payload_bytes_at_limit(definition, &binding);
        let ordinary = success(
            raw_search(&"x".repeat(remaining + 1)),
            definition,
            &binding,
            false,
        )
        .unwrap();
        assert_eq!(ordinary.bytes().len(), QUERY_RESPONSE_BYTES + 1);
    }
}
