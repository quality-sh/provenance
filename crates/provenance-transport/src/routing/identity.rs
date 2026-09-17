use super::invalid;
use provenance_core::protocol::failure::ErasedFailure;
use provenance_store::operations::catalog::Definition;
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
    let bound = definition
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
                    provenance_store::operations::catalog::SelectorBinding::Discussion {
                        field,
                        ..
                    }
                    | provenance_store::operations::catalog::SelectorBinding::Legacy {
                        field,
                        ..
                    } => *field,
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
    let repeated = ["repository", "scope", "context"]
        .into_iter()
        .chain(bound)
        .find(|name| object.contains_key(*name));
    repeated.map_or(Ok(()), |field| Err(invalid(Some(field))))
}
