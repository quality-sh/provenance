use super::{Operation, TargetAction};
use provenance_core::protocol::failure::OperationError;
use provenance_core::NodeType;
use schemars::{
    generate::{Contract, SchemaSettings},
    JsonSchema,
};
use serde_json::{json, Value};

mod envelope;
pub(super) use envelope::{hide_bound_request_field, request_envelope, response_envelope};

pub use super::schema_values::{
    parse_parameter_value, parse_schema_value, parse_schema_value_in, serialize_parameter_value,
    ParseValueError,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
    Patch,
}
impl HttpMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Patch => "patch",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseKind {
    Resource,
    Items,
    Result,
}

#[derive(Clone)]
pub struct Parameter {
    pub name: &'static str,
    pub location: &'static str,
    pub required: bool,
    pub schema: Value,
}

#[derive(Clone)]
pub struct Definition {
    pub name: &'static str,
    pub operation_id: &'static str,
    pub method: HttpMethod,
    pub path: &'static str,
    pub description: &'static str,
    pub registration: super::Registration,
}

#[derive(Clone)]
pub struct QueryVariant {
    pub selector: Option<&'static str>,
    pub parameters: Vec<Parameter>,
    pub success_schema: Value,
    pub failure_schema: Value,
}

pub fn target_definition(action: TargetAction, kind: NodeType) -> Option<&'static Definition> {
    target_definitions(action)
        .find_map(|(registered_kind, definition)| (registered_kind == kind).then_some(definition))
}

pub fn target_definitions(
    action: TargetAction,
) -> impl Iterator<Item = (NodeType, &'static Definition)> {
    definitions().iter().filter_map(move |definition| {
        definition
            .registration
            .target
            .filter(|binding| binding.action == action)
            .map(|binding| (binding.kind, definition))
    })
}

#[derive(Clone)]
pub(super) struct RawDefinition {
    pub request_schema: Value,
    pub success_schema: Value,
    pub failure_schema: Value,
    pub mutates: bool,
    pub http_statuses: Vec<u16>,
    pub context: super::ContextKind,
}

/// Return the canonical request schema for one registered operation type.
pub fn operation_request_schema<O: Operation>() -> Value {
    raw_definition::<O>().request_schema
}

/// Return the canonical success schema for one registered operation type.
pub fn operation_success_schema<O: Operation>() -> Value {
    raw_definition::<O>().success_schema
}

impl Definition {
    pub const fn returns_etag(&self) -> bool {
        self.registration.controls.etag.is_some()
    }
    pub const fn mutates(&self) -> bool {
        self.registration.handler.mutates
    }
    pub fn http_statuses(&self) -> &[u16] {
        &self.registration.handler.http_statuses
    }
    pub const fn request_schema(&self) -> Option<&Value> {
        self.registration.request.schema.as_ref()
    }
    pub const fn failure_schema(&self) -> &Value {
        &self.registration.handler.failure_schema
    }
    pub fn success_schema(&self) -> Value {
        let mut variants = std::iter::once(&self.registration.response)
            .chain(
                self.registration
                    .queries
                    .iter()
                    .map(|query| &query.response),
            )
            .map(|response| response.schema.clone())
            .collect::<Vec<_>>();
        if variants.len() == 1 {
            return variants.pop().unwrap();
        }
        merge_variants(variants)
    }
    pub fn parameters(&self) -> Vec<Parameter> {
        let mut parameters = self.registration.request.parameters.clone();
        if !self.registration.queries.is_empty() {
            parameters.push(query(
                "query",
                json!({"type":"string","enum":self.registration.queries.iter().map(|route| route.name).collect::<Vec<_>>() }),
            ));
        }
        for parameter in self
            .registration
            .queries
            .iter()
            .flat_map(|route| route.parameters.iter())
        {
            if !parameters
                .iter()
                .any(|found| found.name == parameter.name && found.location == parameter.location)
            {
                parameters.push(parameter.clone());
            }
        }
        parameters.extend(
            self.registration
                .controls
                .headers
                .iter()
                .map(|binding| header(binding.name)),
        );
        parameters
    }
    pub fn query_variants(&self) -> Vec<QueryVariant> {
        if self.registration.queries.is_empty() {
            return Vec::new();
        }
        let headers = self
            .registration
            .controls
            .headers
            .iter()
            .map(|binding| header(binding.name))
            .collect::<Vec<_>>();
        let base = QueryVariant {
            selector: None,
            parameters: self
                .registration
                .request
                .parameters
                .iter()
                .cloned()
                .chain(headers.iter().cloned())
                .collect(),
            success_schema: self.registration.response.schema.clone(),
            failure_schema: self.registration.handler.failure_schema.clone(),
        };
        std::iter::once(base)
            .chain(self.registration.queries.iter().map(|route| {
                let mut parameters = self
                    .registration
                    .request
                    .parameters
                    .iter()
                    .filter(|parameter| parameter.location == "path")
                    .cloned()
                    .collect::<Vec<_>>();
                parameters.push(Parameter {
                    name: "query",
                    location: "query",
                    required: true,
                    schema: json!({"type":"string","const":route.name}),
                });
                parameters.extend(route.parameters.iter().cloned());
                parameters.extend(headers.iter().cloned());
                QueryVariant {
                    selector: Some(route.name),
                    parameters,
                    success_schema: route.response.schema.clone(),
                    failure_schema: route.handler.failure_schema.clone(),
                }
            }))
            .collect()
    }
    pub fn mcp_output_schema(&self) -> Value {
        self.success_schema()
    }
    pub fn mcp_input_schema(&self) -> Value {
        let variants: Vec<QueryVariant> = self.query_variants();
        let parameter_sets: Vec<Vec<Parameter>> = if variants.is_empty() {
            vec![self.parameters()]
        } else {
            variants
                .into_iter()
                .map(|variant| variant.parameters)
                .collect()
        };
        let mut schemas: Vec<Value> = parameter_sets
            .iter()
            .map(|parameters| mcp_input_variant_schema(self.request_schema(), parameters))
            .collect();
        let mut result: Value = if schemas.len() == 1 {
            schemas.pop().unwrap()
        } else {
            json!({"oneOf":schemas})
        };
        if let Some(defs) = self.request_schema().and_then(|schema| schema.get("$defs")) {
            result["$defs"] = defs.clone();
        }
        result
    }
}

fn mcp_input_variant_schema(body: Option<&Value>, parameters: &[Parameter]) -> Value {
    let mut properties: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut required: Vec<Value> = Vec::new();
    if let Some(body) = body {
        properties.insert("data".into(), body["properties"]["data"].clone());
        required.push(json!("data"));
    }
    for parameter in parameters {
        let name: String = if parameter.location == "header" {
            parameter.name.to_ascii_lowercase().replace('-', "_")
        } else {
            parameter.name.to_owned()
        };
        properties.insert(name.clone(), parameter.schema.clone());
        if parameter.required {
            required.push(json!(name));
        }
    }
    json!({"type":"object","additionalProperties":false,"properties":properties,"required":required})
}

fn merge_variants(variants: Vec<Value>) -> Value {
    let mut unique = Vec::new();
    for variant in variants {
        if !unique.contains(&variant) {
            unique.push(variant);
        }
    }
    let mut variants = unique;
    if variants.len() == 1 {
        return variants.pop().unwrap();
    }
    let mut defs = serde_json::Map::new();
    for schema in &mut variants {
        if let Some(Value::Object(found)) = schema
            .as_object_mut()
            .and_then(|object| object.remove("$defs"))
        {
            for (name, value) in found {
                defs.entry(name).or_insert(value);
            }
        }
    }
    let mut schema = json!({"anyOf":variants});
    if !defs.is_empty() {
        schema["$defs"] = Value::Object(defs);
    }
    schema
}

fn schema<T: JsonSchema>(contract: Contract) -> Value {
    serde_json::to_value(
        SchemaSettings::draft2020_12()
            .with(|settings| settings.contract = contract)
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .expect("schema is JSON")
}

pub(super) fn raw_definition<O: Operation>() -> RawDefinition {
    RawDefinition {
        request_schema: schema::<O::Request>(Contract::Deserialize),
        success_schema: schema::<O::Success>(Contract::Serialize),
        failure_schema: schema::<
            provenance_core::protocol::failure::FailureEnvelope<OperationError<O::Failure>>,
        >(Contract::Serialize),
        mutates: O::MUTATES,
        http_statuses: statuses(O::MUTATES, O::FAILURE_STATUSES),
        context: O::CONTEXT,
    }
}

fn statuses(mutates: bool, declared: &[u16]) -> Vec<u16> {
    [400, 401, 403, 404, 405, 500, 503]
        .into_iter()
        .chain(mutates.then_some(409))
        .chain(declared.iter().copied())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn property_schema(root: &Value, path: &[&str]) -> Value {
    let mut current = root;
    for name in path {
        let resolved = resolve_schema(root, current);
        current = resolved
            .get("properties")
            .and_then(|properties| properties.get(*name))
            .unwrap_or_else(|| panic!("registered response property {name}: {resolved}"));
    }
    let mut selected = current.clone();
    if let Some(defs) = root.get("$defs") {
        selected["$defs"] = defs.clone();
    }
    selected
}

fn resolve_schema<'a>(root: &'a Value, schema: &'a Value) -> &'a Value {
    schema
        .get("$ref")
        .and_then(Value::as_str)
        .and_then(|reference| reference.strip_prefix("#/$defs/"))
        .and_then(|name| root.get("$defs").and_then(|defs| defs.get(name)))
        .unwrap_or(schema)
}

pub fn definitions() -> &'static [Definition] {
    static DEFINITIONS: std::sync::OnceLock<Vec<Definition>> = std::sync::OnceLock::new();
    DEFINITIONS.get_or_init(super::routes::definitions)
}

pub(super) fn type_schema<T: JsonSchema>(contract: Contract) -> Value {
    schema::<T>(contract)
}

#[allow(clippy::missing_const_for_fn)]
pub(super) fn path(name: &'static str) -> Parameter {
    Parameter {
        name,
        location: "path",
        required: true,
        schema: json!({"type":"string","minLength":1}),
    }
}
pub(super) const fn query(name: &'static str, schema: Value) -> Parameter {
    Parameter {
        name,
        location: "query",
        required: false,
        schema,
    }
}
#[allow(clippy::missing_const_for_fn)]
pub(super) fn header(name: &'static str) -> Parameter {
    Parameter {
        name,
        location: "header",
        required: true,
        schema: json!({"type":"string","minLength":1}),
    }
}
