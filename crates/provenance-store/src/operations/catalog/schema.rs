use super::{entry::entries, Operation};
use provenance_core::protocol::{failure::OperationError, ResponseMeta};
use schemars::{
    generate::{Contract, SchemaSettings},
    JsonSchema,
};
use serde_json::{json, Value};

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
    pub mutates: bool,
    pub http_statuses: Vec<u16>,
    pub parameters: Vec<Parameter>,
    pub request_schema: Option<Value>,
    pub success_schema: Value,
    pub failure_schema: Value,
    pub response_kind: ResponseKind,
    pub backing: &'static str,
    pub context: super::ContextKind,
    pub inject_scope: bool,
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

impl Definition {
    pub fn returns_etag(&self) -> bool {
        matches!(
            self.name,
            "get-requirement" | "create-requirement" | "update-requirement"
        ) || self.name.contains("discussion") && self.response_kind == ResponseKind::Resource
    }
    pub fn mcp_output_schema(&self) -> Value {
        self.success_schema.clone()
    }
    pub fn mcp_input_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        if let Some(body) = &self.request_schema {
            properties.insert("data".into(), body["properties"]["data"].clone());
            required.push(json!("data"));
        }
        for parameter in &self.parameters {
            let name = if parameter.location == "header" {
                parameter.name.to_ascii_lowercase().replace('-', "_")
            } else {
                parameter.name.to_owned()
            };
            properties.insert(name.clone(), parameter.schema.clone());
            if parameter.required {
                required.push(json!(name));
            }
        }
        let mut result = json!({"type":"object","additionalProperties":false,"properties":properties,"required":required});
        if let Some(defs) = self
            .request_schema
            .as_ref()
            .and_then(|schema| schema.get("$defs"))
        {
            result["$defs"] = defs.clone();
        }
        result
    }
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

pub(super) fn raw_for(name: &str) -> RawDefinition {
    let entries = entries();
    let entry = entries
        .iter()
        .find(|entry| entry.name == name)
        .or_else(|| {
            name.starts_with("list-")
                .then(|| entries.iter().find(|entry| entry.name == "get"))
                .flatten()
        })
        .expect("backing operation");
    (entry.definition)()
}

fn statuses(mutates: bool, declared: &[u16]) -> Vec<u16> {
    [400, 401, 403, 404, 500, 503]
        .into_iter()
        .chain(mutates.then_some(409))
        .chain(declared.iter().copied())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(super) fn response_envelope(mut payload: Value, kind: ResponseKind) -> Value {
    if kind != ResponseKind::Items {
        strip_fields(
            &mut payload,
            &[
                "protocol_version",
                "operation",
                "stamp",
                "freshness_error",
                "freshness_cause",
                "limit",
                "has_more",
                "next_cursor",
            ],
        );
    }
    let defs = payload.as_object_mut().and_then(|o| o.remove("$defs"));
    payload.as_object_mut().map(|o| o.remove("$schema"));
    if let Some(result) = payload
        .get("properties")
        .and_then(|p| p.get("result"))
        .cloned()
    {
        payload = result;
    }
    let mut meta = schema::<ResponseMeta>(Contract::Serialize);
    namespace_defs(&mut meta, "ResponseMeta");
    let meta_defs = meta.as_object_mut().and_then(|o| o.remove("$defs"));
    meta.as_object_mut().map(|o| o.remove("$schema"));
    let data = match kind {
        ResponseKind::Items => {
            let items = payload
                .get("properties")
                .and_then(|properties| {
                    ["items", "entries", "nodes", "sites", "rules"]
                        .into_iter()
                        .find_map(|name| properties.get(name))
                })
                .cloned()
                .unwrap_or(payload);
            json!({"type":"object","additionalProperties":false,"required":["items"],"properties":{"items":items}})
        }
        ResponseKind::Resource | ResponseKind::Result => payload,
    };
    let mut result = json!({"type":"object","additionalProperties":false,"required":["data","meta"],"properties":{"data":data,"meta":meta}});
    let mut merged = serde_json::Map::new();
    if let Some(Value::Object(defs)) = defs {
        merged.extend(defs);
    }
    if let Some(Value::Object(defs)) = meta_defs {
        for (name, value) in defs {
            merged.entry(name).or_insert(value);
        }
    }
    if !merged.is_empty() {
        result["$defs"] = Value::Object(merged);
    }
    result
}

pub(super) fn namespace_defs(value: &mut Value, prefix: &str) {
    match value {
        Value::Object(object) => {
            if let Some(Value::String(reference)) = object.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/$defs/") {
                    *reference = format!("#/$defs/{prefix}{name}");
                }
            }
            if let Some(Value::Object(defs)) = object.remove("$defs") {
                object.insert(
                    "$defs".into(),
                    Value::Object(
                        defs.into_iter()
                            .map(|(name, schema)| (format!("{prefix}{name}"), schema))
                            .collect(),
                    ),
                );
            }
            for child in object.values_mut() {
                namespace_defs(child, prefix);
            }
        }
        Value::Array(array) => {
            for child in array {
                namespace_defs(child, prefix);
            }
        }
        _ => {}
    }
}

pub(super) fn request_envelope(mut request: Value, strip: &[&str]) -> Value {
    strip_fields(&mut request, strip);
    let defs = request.as_object_mut().and_then(|o| o.remove("$defs"));
    request.as_object_mut().map(|o| o.remove("$schema"));
    let mut result = json!({"type":"object","additionalProperties":false,"required":["data"],"properties":{"data":request}});
    if let Some(defs) = defs {
        result["$defs"] = defs;
    }
    result
}

fn strip_fields(value: &mut Value, fields: &[&str]) {
    match value {
        Value::Object(object) => {
            if let Some(Value::Object(properties)) = object.get_mut("properties") {
                for field in fields {
                    properties.remove(*field);
                }
            }
            if let Some(Value::Array(required)) = object.get_mut("required") {
                required.retain(|field| !fields.iter().any(|name| field == *name));
            }
            for child in object.values_mut() {
                strip_fields(child, fields);
            }
        }
        Value::Array(array) => {
            for child in array {
                strip_fields(child, fields);
            }
        }
        _ => {}
    }
}

pub fn definitions() -> Vec<Definition> {
    static DEFINITIONS: std::sync::OnceLock<Vec<Definition>> = std::sync::OnceLock::new();
    DEFINITIONS.get_or_init(super::routes::definitions).clone()
}

pub fn bind_response_identity(schema: Value, _: &str) -> Value {
    schema
}

pub(super) fn type_schema<T: JsonSchema>(contract: Contract) -> Value {
    schema::<T>(contract)
}

pub(super) fn path(name: &'static str) -> Parameter {
    Parameter {
        name,
        location: "path",
        required: true,
        schema: json!({"type":"string","minLength":1}),
    }
}
pub(super) fn query(name: &'static str, schema: Value) -> Parameter {
    Parameter {
        name,
        location: "query",
        required: false,
        schema,
    }
}
pub(super) fn header(name: &'static str) -> Parameter {
    Parameter {
        name,
        location: "header",
        required: true,
        schema: json!({"type":"string","minLength":1}),
    }
}
