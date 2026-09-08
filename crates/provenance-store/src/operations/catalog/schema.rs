use super::{
    entry::{entries, DataFreeCall},
    Operation,
};
use provenance_core::{
    protocol::failure::{FailureEnvelope, OperationError},
    SDK_PROTOCOL_VERSION,
};
use schemars::{
    generate::{Contract, SchemaSettings},
    JsonSchema,
};
use serde_json::{json, Value};

pub struct Definition {
    pub name: &'static str,
    pub request_schema: Value,
    pub success_schema: Value,
    pub failure_schema: Value,
}

impl Definition {
    pub fn mcp_input_schema(&self) -> Value {
        let mut call = self.request_schema.clone();
        let object = call
            .as_object_mut()
            .expect("operation requests have object schemas");
        let definitions = object.remove("$defs");
        object.remove("$schema");
        let mut input = json!({"type":"object", "required":["protocol_version","call"], "additionalProperties":false,
            "properties":{"protocol_version":{"type":"integer","const":SDK_PROTOCOL_VERSION},"call":call}});
        if let Some(definitions) = definitions {
            input["$defs"] = definitions;
        }
        input
    }
}

fn schema<T: JsonSchema>(contract: Contract) -> Value {
    let root = SchemaSettings::draft2020_12()
        .with(|settings| settings.contract = contract)
        .into_generator()
        .into_root_schema_for::<T>();
    serde_json::to_value(root).expect("a schema is JSON data")
}

pub(super) fn definition<O: Operation>() -> Definition {
    let mut failure = schema::<FailureEnvelope<OperationError<O::Failure>>>(Contract::Serialize);
    failure["properties"]["protocol_version"] =
        json!({"type":"integer","const":SDK_PROTOCOL_VERSION});
    failure["properties"]["operation"] = json!({"type":"string","const":O::NAME});
    Definition {
        name: O::NAME,
        request_schema: schema::<DataFreeCall<O::Request>>(Contract::Deserialize),
        success_schema: bind_response_identity(schema::<O::Success>(Contract::Serialize), O::NAME),
        failure_schema: failure,
    }
}

pub fn definitions() -> Vec<Definition> {
    entries().iter().map(|entry| (entry.definition)()).collect()
}

/// Binds flattened query results to the registered operation identity.
pub fn bind_response_identity(mut schema: Value, operation: &str) -> Value {
    if schema["properties"].get("operation").is_some() {
        schema["properties"]["operation"] = json!({"type":"string", "const":operation});
        schema["properties"]["protocol_version"] =
            json!({"type":"integer", "const":SDK_PROTOCOL_VERSION});
    }
    schema
}
