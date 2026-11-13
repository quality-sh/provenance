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
        publish_body_fields(definition);
    }
}

/// Every bound route parameter publishes the schema of its typed request
/// field, so the exported parameter constraints and the runtime
/// deserialization limits stay the same limits by construction.
fn derive_bound_parameter_schemas(registration: &mut Registration) {
    let raw = raw_request(registration).clone();
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

/// The request fields that the route fills from its path, its bound context and its headers.
fn bound_fields(registration: &Registration) -> Vec<&'static str> {
    let request = &registration.request;
    let mut fields = request
        .path
        .iter()
        .map(|binding| binding.field)
        .collect::<Vec<_>>();
    fields.extend(request.fixed.iter().map(|binding| binding.field));
    fields.extend(request.scope_field);
    fields.extend(request.parent.iter().map(|binding| binding.field));
    fields.extend(request.selector.iter().map(|binding| match binding {
        SelectorBinding::Discussion { field, .. } | SelectorBinding::Legacy { field, .. } => *field,
    }));
    fields.extend(
        registration
            .controls
            .headers
            .iter()
            .map(|binding| binding.field),
    );
    fields
}

/// The request type of the operation of a registration, with each typed field.
fn raw_request(registration: &Registration) -> &Value {
    let operation = registration.handler.operation;
    registration
        .request
        .raw
        .as_ref()
        .unwrap_or_else(|| panic!("operation `{operation}` has no request type"))
}

/// A route that gets each input field from its path, its bound context and its request
/// headers publishes no request body. Each other route publishes a body without those fields.
#[provenance_macros::rule("rule_route_without_body_fields_takes_no_body")]
fn publish_body_fields(definition: &mut Definition) {
    let registration = &mut definition.registration;
    if registration.request.schema.is_none() {
        return;
    }
    let bound = bound_fields(registration);
    let operation = registration.handler.operation;
    let from_body = query::request_fields(operation, raw_request(registration))
        .iter()
        .any(|(field, _)| !bound.contains(&field.as_str()));
    if !from_body {
        registration.request.schema = None;
        return;
    }
    for field in bound {
        schema::hide_bound_request_field(&mut registration.request.schema, field);
    }
}

#[cfg(test)]
mod tests {
    use super::publish_body_fields;
    use crate::{
        schema, ContextKind, Definition, HandlerBinding, HeaderBinding, HttpMethod, PathBinding,
        Registration, ResponseBinding, ResponseKind,
    };
    use serde_json::{json, Value};
    use std::panic::{catch_unwind, AssertUnwindSafe};

    /// A POST route of the operation `example`. The path fills `id`, the bound context fills
    /// `scope_id`, and the `If-Match` header fills `expected_etag`.
    fn route(raw: &Value) -> Definition {
        let handler = HandlerBinding {
            operation: "example",
            context: ContextKind::Scoped,
            mutates: true,
            http_statuses: Vec::new(),
            failure_schema: Value::Null,
        };
        let response = ResponseBinding::direct(ResponseKind::Result, Value::Null, Value::Null);
        let body = schema::request_envelope(raw.clone());
        let mut registration = Registration::new(handler, Some(body), response);
        registration.request.raw = Some(raw.clone());
        registration.request.path = vec![PathBinding {
            parameter: "id",
            field: "id",
        }];
        registration.request.scope_field = Some("scope_id");
        registration.controls.headers.push(HeaderBinding {
            name: "If-Match",
            field: "expected_etag",
            trim_quotes: true,
            numeric: false,
            required: true,
        });
        Definition {
            name: "example-route",
            operation_id: "exampleRoute",
            method: HttpMethod::Post,
            path: "/examples/{id}",
            description: "An example.",
            registration,
        }
    }

    /// The request body that the route publishes for the request type `raw`, or the message
    /// of the refusal that stops the catalog construction.
    fn published(raw: &Value) -> Result<Option<Value>, String> {
        let mut definition = route(raw);
        let outcome = catch_unwind(AssertUnwindSafe(|| publish_body_fields(&mut definition)));
        match outcome {
            Ok(()) => Ok(definition.registration.request.schema),
            Err(refusal) => Err(refusal
                .downcast_ref::<String>()
                .cloned()
                .expect("the refusal has a message")),
        }
    }

    fn closed(properties: &Value) -> Value {
        json!({"type":"object","additionalProperties":false,"properties":properties})
    }

    /// An implementation aid with no Rule binding: the catalog of the crate holds no request
    /// of these forms with these bindings, so only this step can show them. The unit type has
    /// no field. An object that refuses each field that its `properties` do not name has
    /// those properties. The route decides its body from them.
    #[test]
    fn a_request_that_shows_its_fields_decides_the_body() {
        let field = json!({"type":"string"});
        for raw in [
            json!({"type":"null"}),
            json!({"type":"object","additionalProperties":false}),
            closed(&json!({"id":field,"scope_id":field})),
            closed(&json!({"id":field,"scope_id":field,"expected_etag":field})),
        ] {
            assert_eq!(published(&raw), Ok(None), "{raw}");
        }

        let with_title = closed(&json!({"id":field,"expected_etag":field,"title":field}));
        let mut with_condition = with_title.clone();
        with_condition["anyOf"] = json!([{"required":["title"]},{"required":["expected_etag"]}]);
        for raw in [with_title, with_condition] {
            let body = published(&raw).unwrap().expect("the route has a body");
            let data = body["properties"]["data"]["properties"]
                .as_object()
                .unwrap();
            assert_eq!(data.keys().collect::<Vec<_>>(), ["title"], "{raw}");
        }
    }

    /// An implementation aid with no Rule binding: a request of one of these forms stops the
    /// construction of the catalog, so the catalog of the crate cannot hold one. A field of
    /// such a request can come from a keyword that is not its top-level `properties`, so the
    /// route does not decide its body. The refusal names the operation.
    #[test]
    fn a_request_that_does_not_show_its_fields_stops_catalog_construction() {
        let field = json!({"type":"string"});
        let inner = closed(&json!({"title":field}));
        let id = json!({"id":field});
        let pattern = json!({"^x":field});
        for raw in [
            json!({"$ref":"#/$defs/Inner","$defs":{"Inner":inner}}),
            json!({"allOf":[inner]}),
            json!({"oneOf":[inner,{"type":"null"}]}),
            json!({"type":"object","properties":id,"oneOf":[{"properties":{"title":field}}]}),
            json!({"type":"object","properties":id}),
            json!({"type":"object","additionalProperties":field,"properties":id}),
            json!({"type":"object","additionalProperties":false,"patternProperties":pattern}),
            json!({"type":["object","null"],"additionalProperties":false,"properties":id}),
            json!({"type":"object","additionalProperties":false,"properties":[]}),
            json!({"type":"array","items":field}),
        ] {
            let refusal = published(&raw).expect_err(&raw.to_string());
            assert!(refusal.contains("operation `example`"), "{refusal}");
        }
    }
}
