//! Catalog-described public-path API access shared by the CLI and MCP.

use provenance_core::protocol::failure::{FailureEnvelope, InvalidInputReason, OperationFailure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt::Display, future::Future, pin::Pin};

/// One HTTP method a public route supports.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ApiMethod {
    #[default]
    Get,
    Post,
    Patch,
}

impl ApiMethod {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "get",
            Self::Post => "post",
            Self::Patch => "patch",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "get" => Some(Self::Get),
            "post" => Some(Self::Post),
            "patch" => Some(Self::Patch),
            _ => None,
        }
    }
}

/// The structured arguments one api call accepts on every surface.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, schemars::JsonSchema)]
#[serde(deny_unknown_fields, default)]
pub struct ApiArguments {
    pub path: Option<String>,
    pub method: Option<ApiMethod>,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: Option<serde_json::Map<String, Value>>,
}

/// One semantic public-path request after validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiInput {
    pub method: ApiMethod,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub headers: BTreeMap<String, String>,
    pub body: Option<serde_json::Map<String, Value>>,
}

/// One selected api action: the discovery catalog or one bound call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApiRequest {
    Discover,
    Invoke(ApiInput),
}

/// Why one api request failed, classified for consumers that branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApiErrorKind {
    InvalidOptions,
    UnknownPath,
    MethodNotAllowed,
    AccessDenied,
    Operation,
}

/// One api failure with its canonical refusal envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApiError {
    pub kind: ApiErrorKind,
    pub failure: Value,
}

impl ApiError {
    pub fn invalid_options() -> Self {
        Self {
            kind: ApiErrorKind::InvalidOptions,
            failure: refusal(OperationFailure::InvalidInput {
                field: None,
                reason: InvalidInputReason::InvalidValue,
            }),
        }
    }
}

impl Display for ApiError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self.kind {
            ApiErrorKind::InvalidOptions => "unsupported api options",
            ApiErrorKind::UnknownPath => "unknown api path",
            ApiErrorKind::MethodNotAllowed => "method not allowed for this path",
            ApiErrorKind::AccessDenied => "api access denied",
            ApiErrorKind::Operation => "api operation failed",
        })
    }
}

impl std::error::Error for ApiError {}

fn refusal(error: OperationFailure) -> Value {
    serde_json::to_value(FailureEnvelope::new(None, error)).expect("refusal is JSON")
}

/// One catalog parameter described with its canonical location and schema.
#[derive(Clone, Debug, Serialize, Eq, PartialEq, schemars::JsonSchema)]
pub struct ApiParameter {
    pub name: String,
    pub location: String,
    pub required: bool,
    pub schema: Value,
}

/// One supported public route described by the live operation catalog.
#[derive(Clone, Debug, Serialize, Eq, PartialEq, schemars::JsonSchema)]
pub struct ApiRoute {
    pub method: ApiMethod,
    pub path: String,
    pub description: String,
    pub parameters: Vec<ApiParameter>,
    pub request_schema: Option<Value>,
    pub response_schema: Value,
}

/// The catalog-derived route inventory behind the api action.
#[derive(Clone, Debug, Serialize, Eq, PartialEq, schemars::JsonSchema)]
pub struct ApiCatalog {
    pub routes: Vec<ApiRoute>,
}

/// The structured api answer: the discovery catalog or one route envelope.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ApiOutcome {
    Catalog(ApiCatalog),
    Invoked(Value),
}

/// One future returned by an injected api port.
pub type ApiPortFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, ApiError>> + Send + 'a>>;

/// Public-path invocation and catalog discovery provided by the bound host.
pub trait ApiPort: Send + Sync {
    fn invoke(&self, input: ApiInput) -> ApiPortFuture<'_, Value>;
    fn discover(&self) -> ApiCatalog;
}

/// The typed input schema of one api tool call.
pub fn input_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(ApiArguments))
        .expect("api input schema is JSON")
}

/// The output schema covering both api answers.
pub fn output_schema() -> Value {
    serde_json::json!({
        "anyOf": [
            serde_json::to_value(schemars::schema_for!(ApiCatalog))
                .expect("catalog schema is JSON"),
            {"type": "object", "required": ["data", "meta"], "properties": {"data": {}, "meta": {}}}
        ]
    })
}

impl ApiRequest {
    /// Select discovery or one validated invocation from shared arguments.
    pub fn from(arguments: ApiArguments) -> Result<Self, ApiError> {
        todo!("api request selection")
    }
}

impl<P: ApiPort> crate::Porcelain<P> {
    /// Run one api action after validation and authorization by the port.
    pub async fn execute_api(&self, request: ApiRequest) -> Result<ApiOutcome, ApiError> {
        todo!("api orchestration")
    }
}

/// Render the route inventory as one bounded readable summary.
pub fn render_discovery_readable(catalog: &ApiCatalog) -> String {
    todo!("discovery rendering")
}
