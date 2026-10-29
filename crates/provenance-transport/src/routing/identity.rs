use super::invalid;
use provenance_contract::Definition;
use provenance_core::protocol::failure::ErasedFailure;
use serde_json::Value;
use std::collections::BTreeMap;

pub(super) fn reject(
    data: &Value,
    _path: &BTreeMap<String, String>,
    definition: &Definition,
) -> Result<(), ErasedFailure> {
    let Some(object) = data.as_object() else {
        return Ok(());
    };
    if object.get("context").is_some_and(Value::is_object) {
        return Err(invalid(Some("context")));
    }
    let mut bound = definition
        .registration
        .request
        .path
        .iter()
        .map(|binding| binding.field)
        .chain(definition.registration.request.scope_field)
        .chain(
            definition
                .registration
                .request
                .parent
                .iter()
                .map(|binding| binding.field),
        )
        .chain(
            definition
                .registration
                .request
                .selector
                .iter()
                .map(|binding| match binding {
                    provenance_contract::SelectorBinding::Discussion { field, .. }
                    | provenance_contract::SelectorBinding::Legacy { field, .. } => *field,
                }),
        )
        .chain(
            definition
                .registration
                .controls
                .headers
                .iter()
                .map(|binding| binding.field),
        );
    let repeated = bound.find(|name| object.contains_key(*name));
    repeated.map_or(Ok(()), |field| Err(invalid(Some(field))))
}
