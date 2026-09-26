//! Catalog-described public-path API access shared by the CLI and MCP.

use provenance_core::protocol::failure::{FailureEnvelope, InvalidInputReason, OperationFailure};
use provenance_macros::rule;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Display,
    future::Future,
    pin::Pin,
};

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

    /// Parse one method name in any letter case, as discovery prints
    /// uppercase names and HTTP tools accept either case.
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
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
    /// Refuse one api option with the canonical invalid-input envelope
    /// naming the argument that caused the refusal.
    pub fn invalid_options(field: Option<&str>) -> Self {
        Self {
            kind: ApiErrorKind::InvalidOptions,
            failure: refusal(OperationFailure::InvalidInput {
                field: field.map(str::to_owned),
                reason: InvalidInputReason::InvalidValue,
            }),
        }
    }

    /// Refuse a path that no public route can match, as the HTTP router does.
    fn unknown_path() -> Self {
        Self {
            kind: ApiErrorKind::UnknownPath,
            failure: refusal(OperationFailure::UnknownOperation),
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
            for (name, definition) in definitions {
                debug_assert!(
                    !defs.contains_key(&name),
                    "api output schema definitions collide on {name}"
                );
                defs.insert(name, definition);
            }
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
            let call_option = [
                ("method", arguments.method.is_some()),
                ("query", !arguments.query.is_empty()),
                ("headers", !arguments.headers.is_empty()),
                ("body", arguments.body.is_some()),
            ]
            .into_iter()
            .find_map(|(field, given)| given.then_some(field));
            return call_option.map_or(Ok(Self::Discover), |field| {
                Err(ApiError::invalid_options(Some(field)))
            });
        };
        validated(ApiInput {
            method: arguments.method.unwrap_or_default(),
            path,
            query: arguments.query,
            headers: arguments.headers,
            body: arguments.body,
        })
        .map(Self::Invoke)
    }
}

/// Check one invocation against the shared public-path contract.
///
/// The path takes at most one leading slash and no empty segment, as on the
/// HTTP router, and the returned input carries it with exactly one leading
/// slash. Header names compare without letter case, as HTTP compares them.
fn validated(input: ApiInput) -> Result<ApiInput, ApiError> {
    let relative = input.path.strip_prefix('/').unwrap_or(&input.path);
    if relative.is_empty() || relative.contains('?') {
        return Err(ApiError::invalid_options(Some("path")));
    }
    if relative.split('/').any(str::is_empty) {
        return Err(ApiError::unknown_path());
    }
    if input.method == ApiMethod::Get && input.body.is_some() {
        return Err(ApiError::invalid_options(Some("body")));
    }
    let mut names = BTreeSet::new();
    if !input
        .headers
        .keys()
        .all(|name| names.insert(name.to_ascii_lowercase()))
    {
        return Err(ApiError::invalid_options(Some("headers")));
    }
    let path = format!("/{relative}");
    Ok(ApiInput { path, ..input })
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
                let input = validated(input)?;
                Ok(ApiOutcome::Invoked(self.port.invoke(input).await?))
            }
        }
    }
}

/// Render the route inventory as one bounded readable summary.
///
/// Each route lists the inputs of its base form and of each `query` form;
/// the JSON form keeps the full request and response schemas.
pub fn render_discovery_readable(catalog: &ApiCatalog) -> String {
    let mut lines = vec![format!("api routes: {}", catalog.routes.len())];
    for route in &catalog.routes {
        lines.push(format!(
            "- {} {}",
            route.method.as_str().to_uppercase(),
            route.path
        ));
        lines.push(format!("  {}", route.description));
        for variant in &route.variants {
            let label = variant.selector.as_ref().map_or_else(
                || "inputs".to_owned(),
                |name| format!("inputs with query={name}"),
            );
            lines.push(format!(
                "  {label}: {}",
                render_parameters(&variant.parameters)
            ));
        }
        if route.request_schema.is_some() {
            lines.push("  The request carries one JSON body.".to_owned());
        }
    }
    lines.join("\n")
}

fn render_parameters(parameters: &[ApiParameter]) -> String {
    if parameters.is_empty() {
        return "none".to_owned();
    }
    parameters
        .iter()
        .map(|parameter| {
            let need = if parameter.required {
                "required"
            } else {
                "optional"
            };
            format!("{} ({}, {need})", parameter.name, parameter.location)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use std::collections::BTreeSet;

    fn definition_names(schema: &Value) -> BTreeSet<String> {
        schema["$defs"]
            .as_object()
            .map(|defs| defs.keys().cloned().collect())
            .unwrap_or_default()
    }

    #[test]
    fn output_schema_arms_share_no_definition_name() {
        let catalog = definition_names(
            &serde_json::to_value(schemars::schema_for!(super::ApiCatalog)).unwrap(),
        );
        let envelope = definition_names(
            &serde_json::to_value(schemars::schema_for!(
                provenance_core::protocol::SuccessEnvelope<Value>
            ))
            .unwrap(),
        );
        assert!(
            catalog.is_disjoint(&envelope),
            "shared names: {:?}",
            catalog.intersection(&envelope).collect::<Vec<_>>()
        );
        let merged = definition_names(&super::output_schema());
        assert_eq!(merged.len(), catalog.len() + envelope.len());
    }
}
