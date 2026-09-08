//! Bounded response decoding and contract validation for named operations.
use crate::OperationFailure;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    error::Error as StdError,
    fmt,
    sync::{Arc, OnceLock},
};

pub const MAX_RESPONSE_BYTES: usize = 16 * 1024 * 1024;

/// Safe public context with the underlying cause retained for error chaining.
pub struct ResponseFailure {
    cause: Box<dyn StdError + Send + Sync>,
}
impl fmt::Debug for ResponseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResponseFailure")
    }
}
impl fmt::Display for ResponseFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("HTTP response failure")
    }
}
impl StdError for ResponseFailure {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(self.cause.as_ref())
    }
}
impl ResponseFailure {
    fn new(cause: impl StdError + Send + Sync + 'static) -> Self {
        Self {
            cause: Box::new(cause),
        }
    }
    fn contract() -> Self {
        Self::new(std::io::Error::other(
            "response does not match the operation contract",
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid HTTP host URL")]
    InvalidUrl,
    #[error("invalid bearer credential format")]
    InvalidCredentials,
    #[error("HTTP connection failed")]
    Connection(#[source] ResponseFailure),
    #[error("host response does not match the operation contract")]
    MalformedResponse(#[source] ResponseFailure),
    #[error("incompatible operation protocol: expected {expected}, received {received}")]
    ProtocolMismatch { expected: u32, received: u32 },
    #[error("operation failed with status {status}")]
    Operation {
        status: u16,
        failure: OperationFailure,
    },
    #[error("write outcome is uncertain; inspect repository state before retrying")]
    UncertainWrite {
        operation: &'static str,
        #[source]
        cause: ResponseFailure,
        failure: Option<Box<OperationFailure>>,
    },
}

pub fn connection(operation: &'static str, mutates: bool, cause: reqwest::Error) -> Error {
    outcome(operation, mutates, ResponseFailure::new(cause), false)
}
const fn outcome(
    operation: &'static str,
    mutates: bool,
    cause: ResponseFailure,
    malformed: bool,
) -> Error {
    if mutates {
        Error::UncertainWrite {
            operation,
            cause,
            failure: None,
        }
    } else if malformed {
        Error::MalformedResponse(cause)
    } else {
        Error::Connection(cause)
    }
}
pub fn metadata_status() -> Error {
    Error::Connection(ResponseFailure::contract())
}
pub fn uncertain(operation: &'static str, failure: OperationFailure) -> Error {
    Error::UncertainWrite {
        operation,
        cause: ResponseFailure::contract(),
        failure: Some(Box::new(failure)),
    }
}
pub fn uncertain_kind(value: &Value, mutates: bool) -> bool {
    match value.pointer("/error/kind").and_then(Value::as_str) {
        Some("uncertain_write") => true,
        Some("internal" | "write_failed") => mutates,
        _ => false,
    }
}

pub async fn read_json(
    mut response: reqwest::Response,
    operation: &'static str,
    mutates: bool,
) -> Result<Value, Error> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|cause| connection(operation, mutates, cause))?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(outcome(
                operation,
                mutates,
                ResponseFailure::contract(),
                true,
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|cause| outcome(operation, mutates, ResponseFailure::new(cause), true))
}

pub fn decode<T: DeserializeOwned>(
    value: Value,
    operation: &'static str,
    mutates: bool,
) -> Result<T, Error> {
    serde_json::from_value(value)
        .map_err(|cause| outcome(operation, mutates, ResponseFailure::new(cause), true))
}

pub fn validate(
    value: &Value,
    schema: &str,
    operation: &'static str,
    mutates: bool,
) -> Result<(), Error> {
    if validators()[schema].is_valid(value) {
        Ok(())
    } else {
        Err(outcome(
            operation,
            mutates,
            ResponseFailure::contract(),
            true,
        ))
    }
}

struct NoExternalSchemas;
impl jsonschema::SchemaResolver for NoExternalSchemas {
    fn resolve(
        &self,
        _: &Value,
        _: &reqwest::Url,
        _: &str,
    ) -> Result<Arc<Value>, jsonschema::SchemaResolverError> {
        Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "external schemas are disabled",
        )
        .into())
    }
}
fn validators() -> &'static BTreeMap<String, jsonschema::JSONSchema> {
    static VALIDATORS: OnceLock<BTreeMap<String, jsonschema::JSONSchema>> = OnceLock::new();
    VALIDATORS.get_or_init(|| {
        let mut document: Value = serde_json::from_str(include_str!("generated/responses.json")).expect("generated response schemas are JSON");
        document["components"]["schemas"]["MetadataOutput"]["properties"]["protocol_version"].as_object_mut().expect("metadata version property").remove("const");
        document["response_schemas"].as_array().expect("generated response inventory").iter().map(|name| {
            let name = name.as_str().expect("schema name").to_owned();
            let schema = json!({"$ref":format!("#/components/schemas/{name}"),"components":document["components"]});
            let validator = jsonschema::JSONSchema::options()
                .with_draft(jsonschema::Draft::Draft202012)
                .with_resolver(NoExternalSchemas)
                .compile(&schema).expect("generated response schema is valid");
            (name, validator)
        }).collect()
    })
}
