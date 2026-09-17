use super::Operation;
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
    pub registration: super::Registration,
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
    pub const fn returns_etag(&self) -> bool {
        self.registration.controls.etag.is_some()
    }
    pub const fn mutates(&self) -> bool {
        self.registration.handler.mutates
    }
    pub fn http_statuses(&self) -> &[u16] {
        &self.registration.handler.http_statuses
    }
    pub fn request_schema(&self) -> Option<&Value> {
        self.registration.request.schema.as_ref()
    }
    pub fn failure_schema(&self) -> &Value {
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
    pub fn mcp_output_schema(&self) -> Value {
        self.success_schema()
    }
    pub fn mcp_input_schema(&self) -> Value {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();
        if let Some(body) = self.request_schema() {
            properties.insert("data".into(), body["properties"]["data"].clone());
            required.push(json!("data"));
        }
        for parameter in self.parameters() {
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
        if let Some(defs) = self.request_schema().and_then(|schema| schema.get("$defs")) {
            result["$defs"] = defs.clone();
        }
        result
    }
}

fn merge_variants(mut variants: Vec<Value>) -> Value {
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
        remove_response_metadata(
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
    let mut meta = schema::<ResponseMeta>(Contract::Serialize);
    namespace_defs(&mut meta, "ResponseMeta");
    let meta_defs = meta.as_object_mut().and_then(|o| o.remove("$defs"));
    meta.as_object_mut().map(|o| o.remove("$schema"));
    let data = match kind {
        ResponseKind::Items => {
            json!({"type":"object","additionalProperties":false,"required":["items"],"properties":{"items":payload}})
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

pub(super) fn request_envelope(mut request: Value) -> Value {
    let defs = request.as_object_mut().and_then(|o| o.remove("$defs"));
    request.as_object_mut().map(|o| o.remove("$schema"));
    let mut result = json!({"type":"object","additionalProperties":false,"required":["data"],"properties":{"data":request}});
    if let Some(defs) = defs {
        result["$defs"] = defs;
    }
    result
}

pub(super) fn hide_bound_request_field(request: &mut Option<Value>, field: &str) {
    let Some(data) = request
        .as_mut()
        .and_then(|schema| schema.pointer_mut("/properties/data"))
    else {
        return;
    };
    data.pointer_mut("/properties")
        .and_then(Value::as_object_mut)
        .map(|properties| properties.remove(field));
    if let Some(required) = data.pointer_mut("/required").and_then(Value::as_array_mut) {
        required.retain(|name| name.as_str() != Some(field));
    }
}

fn remove_response_metadata(value: &mut Value, fields: &[&str]) {
    let Some(object) = value.as_object_mut() else {
        return;
    };
    if let Some(Value::Object(properties)) = object.get_mut("properties") {
        for field in fields {
            properties.remove(*field);
        }
    }
    if let Some(Value::Array(required)) = object.get_mut("required") {
        required.retain(|field| !fields.iter().any(|name| field == *name));
    }
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

#[derive(Clone, Copy, Debug)]
pub struct ParseValueError;

pub fn parse_parameter_value(parameter: &Parameter, raw: &str) -> Result<Value, ParseValueError> {
    parse_schema_value(&parameter.schema, raw)
}

pub fn parse_schema_value(schema: &Value, raw: &str) -> Result<Value, ParseValueError> {
    parse_schema_value_in(schema, schema, raw)
}

pub fn parse_schema_value_in(
    root: &Value,
    schema: &Value,
    raw: &str,
) -> Result<Value, ParseValueError> {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let name = reference.strip_prefix("#/$defs/").ok_or(ParseValueError)?;
        let resolved = root
            .get("$defs")
            .and_then(|defs| defs.get(name))
            .ok_or(ParseValueError)?;
        return parse_schema_value_in(root, resolved, raw);
    }
    if let Some(variants) = schema
        .get("anyOf")
        .or_else(|| schema.get("oneOf"))
        .and_then(Value::as_array)
    {
        if raw == "null"
            && variants
                .iter()
                .any(|variant| variant.get("type") == Some(&json!("null")))
        {
            return Ok(Value::Null);
        }
        return variants
            .iter()
            .filter(|variant| variant.get("type") != Some(&json!("null")))
            .find_map(|variant| parse_schema_value_in(root, variant, raw).ok())
            .ok_or(ParseValueError);
    }
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let value = Value::String(raw.to_owned());
        return values
            .contains(&value)
            .then_some(value)
            .ok_or(ParseValueError);
    }
    if let Some(types) = schema.get("type").and_then(Value::as_array) {
        if raw == "null" && types.iter().any(|kind| kind == "null") {
            return Ok(Value::Null);
        }
        return types
            .iter()
            .filter_map(Value::as_str)
            .filter(|kind| *kind != "null")
            .find_map(|kind| {
                let mut variant = schema.clone();
                variant["type"] = json!(kind);
                parse_schema_value_in(root, &variant, raw).ok()
            })
            .ok_or(ParseValueError);
    }
    match schema.get("type").and_then(Value::as_str) {
        Some("string") => Ok(Value::String(raw.to_owned())),
        Some("boolean") => match raw {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(ParseValueError),
        },
        Some("integer") => {
            let value = raw.parse::<i64>().map_err(|_| ParseValueError)?;
            let minimum = schema.get("minimum").and_then(Value::as_i64);
            let maximum = schema.get("maximum").and_then(Value::as_i64);
            if minimum.is_some_and(|minimum| value < minimum)
                || maximum.is_some_and(|maximum| value > maximum)
            {
                return Err(ParseValueError);
            }
            Ok(Value::from(value))
        }
        Some("number") => {
            let value = raw.parse::<f64>().map_err(|_| ParseValueError)?;
            serde_json::Number::from_f64(value)
                .map(Value::Number)
                .ok_or(ParseValueError)
        }
        Some("null") if raw == "null" => Ok(Value::Null),
        _ => Err(ParseValueError),
    }
}
