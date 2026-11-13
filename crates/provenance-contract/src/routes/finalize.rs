use super::{query, schema, Definition, Registration, SelectorBinding};
use serde_json::Value;

pub(super) fn request_schemas(definitions: &mut [Definition]) {
    for definition in definitions {
        let registration = &mut definition.registration;
        derive_bound_parameter_schemas(registration);
        schema::stamp_page_metadata(
            &mut registration.response.schema,
            &registration.response.raw_schema,
        );
        for query_route in &mut registration.queries {
            schema::stamp_page_metadata(
                &mut query_route.response.schema,
                &query_route.response.raw_schema,
            );
        }
        hide_bound_fields(definition);
        drop_body_without_fields(&mut definition.registration.request.schema);
    }
}

/// A route that gets each input field from its path and its bound context publishes no
/// request body.
#[provenance_macros::rule("rule_route_without_body_fields_takes_no_body")]
fn drop_body_without_fields(request: &mut Option<Value>) {
    let no_field = request
        .as_ref()
        .and_then(|schema| schema.pointer("/properties/data"))
        .is_some_and(|data| {
            data.get("additionalProperties") == Some(&Value::Bool(false))
                && data
                    .get("properties")
                    .and_then(Value::as_object)
                    .is_some_and(serde_json::Map::is_empty)
        });
    if no_field {
        *request = None;
    }
}

/// Every bound route parameter publishes the schema of its typed request
/// field, so the exported parameter constraints and the runtime
/// deserialization limits stay the same limits by construction.
fn derive_bound_parameter_schemas(registration: &mut Registration) {
    let raw =
        registration.request.raw.clone().unwrap_or_else(|| {
            panic!("bound parameters without a request type cannot derive schemas")
        });
    let mut derived = Vec::new();
    for binding in &registration.request.path {
        derived.push((
            binding.parameter,
            query::bound_field_schema(&raw, binding.field),
        ));
    }
    if let Some(parent) = &registration.request.parent {
        derived.push((
            parent.id_parameter,
            query::bound_leaf_schema(&raw, parent.field, "node_id"),
        ));
    }
    if let Some(selector) = &registration.request.selector {
        let (parameter, field, leaf) = match selector {
            SelectorBinding::Discussion { parameter, field } => {
                (*parameter, *field, "discussion_id")
            }
            SelectorBinding::Legacy { parameter, field } => (*parameter, *field, "thread_id"),
        };
        derived.push((parameter, query::bound_leaf_schema(&raw, field, leaf)));
    }
    for parameter in &mut registration.request.parameters {
        if parameter.location != "path" {
            continue;
        }
        let found = derived
            .iter()
            .find(|(name, _)| *name == parameter.name)
            .unwrap_or_else(|| {
                panic!(
                    "path parameter `{}` has no typed request field to derive from",
                    parameter.name
                )
            });
        parameter.schema = found.1.clone();
    }
}

fn hide_bound_fields(definition: &mut Definition) {
    let registration = &definition.registration;
    let mut fields = registration
        .request
        .path
        .iter()
        .map(|binding| binding.field)
        .collect::<Vec<_>>();
    fields.extend(
        registration
            .request
            .fixed
            .iter()
            .map(|binding| binding.field),
    );
    fields.extend(registration.request.scope_field);
    fields.extend(
        registration
            .request
            .parent
            .iter()
            .map(|binding| binding.field),
    );
    fields.extend(
        registration
            .request
            .selector
            .iter()
            .map(|binding| match binding {
                SelectorBinding::Discussion { field, .. }
                | SelectorBinding::Legacy { field, .. } => *field,
            }),
    );
    fields.extend(
        registration
            .controls
            .headers
            .iter()
            .map(|binding| binding.field),
    );
    fields.push("action");
    fields.sort_unstable();
    fields.dedup();
    for field in fields {
        schema::hide_bound_request_field(&mut definition.registration.request.schema, field);
    }
}
