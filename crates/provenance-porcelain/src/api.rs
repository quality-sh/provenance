//! Catalog-described public-path API access shared by the CLI and MCP.

use provenance_core::protocol::failure::{FailureEnvelope, InvalidInputReason, OperationFailure};
use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fmt::Display, future::Future, pin::Pin};

/// One HTTP method a public route supports.
#[derive(
    Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, Serialize, schemars::JsonSchema,
)]
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
    pub request_schema: Option<Value>,
    pub variants: Vec<ApiVariant>,
}

/// One selector-scoped input and response contract of one route.
///
/// The base variant has no selector; each query variant names its selector
/// and carries the parameters, success schema, and failure schema that the
/// canonical definition declares for it.
#[derive(Clone, Debug, Serialize, Eq, PartialEq, schemars::JsonSchema)]
pub struct ApiVariant {
    pub selector: Option<String>,
    pub parameters: Vec<ApiParameter>,
    pub success_schema: Value,
    pub failure_schema: Value,
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
pub type ApiPortFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, ApiError>> + Send + 'a>>;

/// What one api tool call offers a host client.
pub const API_DESCRIPTION: &str = "Call one public API path with an optional method, query, headers, and JSON body. Omit the path to list the catalog routes with their methods, inputs, and response schemas.";

/// Public-path invocation and catalog discovery provided by the bound host.
pub trait ApiPort: Send + Sync {
    fn invoke(&self, input: ApiInput) -> ApiPortFuture<'_, Value>;
    fn discover(&self) -> ApiCatalog;
}

/// The typed input schema of one api tool call.
pub fn input_schema() -> Value {
    serde_json::to_value(schemars::schema_for!(ApiArguments)).expect("api input schema is JSON")
}

/// The output schema covering both api answers.
///
/// Both arms are generated from typed contracts, so their shared definitions
/// are hoisted to one root document and every `#/$defs/` reference resolves.
pub fn output_schema() -> Value {
    let mut catalog =
        serde_json::to_value(schemars::schema_for!(ApiCatalog)).expect("catalog schema is JSON");
    let mut envelope = serde_json::to_value(schemars::schema_for!(
        provenance_core::protocol::SuccessEnvelope<Value>
    ))
    .expect("envelope schema is JSON");
    let mut defs = serde_json::Map::new();
    for schema in [&mut catalog, &mut envelope] {
        if let Some(definitions) = schema
            .as_object_mut()
            .and_then(|object| object.remove("$defs"))
            .and_then(|definitions| definitions.as_object().cloned())
        {
            defs.extend(definitions);
        }
    }
    let mut schema = serde_json::json!({"anyOf": [catalog, envelope]});
    if !defs.is_empty() {
        schema["$defs"] = Value::Object(defs);
    }
    schema
}

impl ApiRequest {
    /// Select discovery or one validated invocation from shared arguments.
    pub fn from(arguments: ApiArguments) -> Result<Self, ApiError> {
        let Some(path) = arguments.path else {
            let call_options = arguments.method.is_some()
                || !arguments.query.is_empty()
                || !arguments.headers.is_empty()
                || arguments.body.is_some();
            if call_options {
                return Err(ApiError::invalid_options());
            }
            return Ok(Self::Discover);
        };
        let path = path.trim().trim_matches('/');
        let invalid = || ApiError::invalid_options();
        if path.is_empty() || path.contains('?') {
            return Err(invalid());
        }
        if arguments
            .method
            .is_none_or(|method| method == ApiMethod::Get)
            && arguments.body.is_some()
        {
            return Err(invalid());
        }
        Ok(Self::Invoke(ApiInput {
            method: arguments.method.unwrap_or_default(),
            path: path.to_owned(),
            query: arguments.query,
            headers: arguments.headers,
            body: arguments.body,
        }))
    }
}

impl<P: ApiPort> crate::Porcelain<P> {
    /// Run one api action after validation by the shared public-path seam.
    ///
    /// The port resolves the path against the registered public routes, so a
    /// call never reaches an operation the catalog does not publish, and the
    /// default method stays GET for arguments that select none.
    #[rule("rule_porcelain_api_public_path")]
    pub async fn execute_api(&self, request: ApiRequest) -> Result<ApiOutcome, ApiError> {
        match request {
            ApiRequest::Discover => Ok(ApiOutcome::Catalog(self.port.discover())),
            ApiRequest::Invoke(input) => {
                let invalid = || ApiError::invalid_options();
                let path = input.path.trim().trim_matches('/');
                if path.is_empty() || path.contains('?') {
                    return Err(invalid());
                }
                if input.method == ApiMethod::Get && input.body.is_some() {
                    return Err(invalid());
                }
                let bound = ApiInput {
                    path: path.to_owned(),
                    ..input
                };
                Ok(ApiOutcome::Invoked(self.port.invoke(bound).await?))
            }
        }
    }
}

/// Render the route inventory as one bounded readable summary.
pub fn render_discovery_readable(catalog: &ApiCatalog) -> String {
    let mut lines = vec![format!("api routes: {}", catalog.routes.len())];
    for route in &catalog.routes {
        lines.push(format!(
            "- {} {}",
            route.method.as_str().to_uppercase(),
            route.path
        ));
        lines.push(format!("  {}", route.description));
        if route.request_schema.is_some() {
            lines.push("  The request carries one JSON body.".to_owned());
        }
    }
    lines.join("\n")
}
