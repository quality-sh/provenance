use super::{
    entry::{entries, DataFreeCall, RepositoryCall},
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

#[derive(Clone)]
pub struct Definition {
    pub name: &'static str,
    pub mutates: bool,
    pub http_statuses: Vec<u16>,
    pub request_schema: Value,
    pub success_schema: Value,
    pub failure_schema: Value,
}

impl Definition {
    /// MCP structured content must be an object; list results remain complete.
    pub fn mcp_output_schema(&self) -> Value {
        if self.success_schema["type"] != "array" {
            return self.success_schema.clone();
        }
        let mut array = self.success_schema.clone();
        let definitions = array
            .as_object_mut()
            .and_then(|value| value.remove("$defs"));
        array.as_object_mut().unwrap().remove("$schema");
        let mut schema = json!({"type":"object","required":["result"],"additionalProperties":false,"properties":{"result":array}});
        if let Some(definitions) = definitions {
            schema["$defs"] = definitions;
        }
        schema
    }
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
        mutates: O::MUTATES,
        http_statuses: [400, 401, 403, 404, 500, 503]
            .into_iter()
            .chain(O::FAILURE_STATUSES.iter().copied())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect(),
        request_schema: match O::CONTEXT {
            super::ContextKind::DataFree => {
                schema::<DataFreeCall<O::Request>>(Contract::Deserialize)
            }
            super::ContextKind::Repository => schema::<
                RepositoryCall<O::Request, provenance_core::protocol::repository::RepositoryTarget>,
            >(Contract::Deserialize),
            super::ContextKind::Scope => schema::<
                RepositoryCall<O::Request, provenance_core::protocol::repository::RepositoryScope>,
            >(Contract::Deserialize),
            super::ContextKind::Scoped => schema::<
                RepositoryCall<
                    O::Request,
                    provenance_core::protocol::repository::RepositoryContext,
                >,
            >(Contract::Deserialize),
        },
        success_schema: bind_response_identity(schema::<O::Success>(Contract::Serialize), O::NAME),
        failure_schema: failure,
    }
}

pub fn definitions() -> Vec<Definition> {
    static DEFINITIONS: std::sync::OnceLock<Vec<Definition>> = std::sync::OnceLock::new();
    DEFINITIONS
        .get_or_init(|| entries().iter().map(|entry| (entry.definition)()).collect())
        .clone()
}

/// Binds flattened query results to the registered operation identity.
pub fn bind_response_identity(mut schema: Value, operation: &str) -> Value {
    if schema["properties"].get("operation").is_some() {
        schema["properties"]["operation"] = json!({"type":"string", "const":operation});
    }
    if schema["properties"].get("protocol_version").is_some() {
        schema["properties"]["protocol_version"] =
            json!({"type":"integer", "const":SDK_PROTOCOL_VERSION});
    }
    schema
}
