use provenance_porcelain::api::{
    ApiCatalog, ApiError, ApiErrorKind, ApiMethod, ApiParameter, ApiPort, ApiPortFuture, ApiRoute,
    ApiVariant,
};
use provenance_store::operations::catalog::{self, Definition, HttpMethod};
use serde_json::{json, Value};

/// The canonical public-path routes adapted to the Porcelain api port.
#[derive(Clone)]
pub struct HostApiPort {
    host: crate::StatementHost,
}

impl HostApiPort {
    pub const fn new(host: crate::StatementHost) -> Self {
        Self { host }
    }
}

impl ApiPort for HostApiPort {
    fn invoke(&self, input: provenance_porcelain::api::ApiInput) -> ApiPortFuture<'_, Value> {
        Box::pin(async move {
            let mut headers = axum::http::HeaderMap::new();
            for (name, value) in &input.headers {
                let name = axum::http::HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| ApiError::invalid_options())?;
                let value = axum::http::HeaderValue::from_str(value)
                    .map_err(|_| ApiError::invalid_options())?;
                headers.insert(name, value);
            }
            let data = input.body.clone().map_or_else(|| json!({}), Value::Object);
            let method = match input.method {
                ApiMethod::Get => axum::http::Method::GET,
                ApiMethod::Post => axum::http::Method::POST,
                ApiMethod::Patch => axum::http::Method::PATCH,
            };
            self.host
                .invoke_resource(method, &input.path, data, input.query, headers)
                .await
                .map_err(|failure| api_error(&failure))
        })
    }

    fn discover(&self) -> ApiCatalog {
        routes(&self.host)
    }
}

/// Describe the advertised public routes from the live operation catalog,
/// so discovery never drifts from the schemas the routes actually serve.
#[provenance_macros::rule("rule_porcelain_api_catalog_discovery")]
fn routes(host: &crate::StatementHost) -> ApiCatalog {
    ApiCatalog {
        routes: catalog::definitions()
            .iter()
            .filter(|definition| host.advertises(definition.name))
            .map(route)
            .collect(),
    }
}

fn route(definition: &Definition) -> ApiRoute {
    let request_schema = definition.request_schema().map(|schema| {
        let mut data = schema
            .get("properties")
            .and_then(|properties| properties.get("data"))
            .cloned()
            .unwrap_or_else(|| json!({}));
        if let Some(defs) = schema.get("$defs") {
            data["$defs"] = defs.clone();
        }
        data
    });
    let declared = definition.query_variants();
    let variants = if declared.is_empty() {
        vec![ApiVariant {
            selector: None,
            parameters: definition.parameters().iter().map(parameter).collect(),
            success_schema: definition.success_schema(),
            failure_schema: definition.failure_schema().clone(),
        }]
    } else {
        declared
            .iter()
            .map(|variant| ApiVariant {
                selector: variant.selector.map(str::to_owned),
                parameters: variant.parameters.iter().map(parameter).collect(),
                success_schema: variant.success_schema.clone(),
                failure_schema: variant.failure_schema.clone(),
            })
            .collect()
    };
    ApiRoute {
        method: match definition.method {
            HttpMethod::Get => ApiMethod::Get,
            HttpMethod::Post => ApiMethod::Post,
            HttpMethod::Patch => ApiMethod::Patch,
        },
        path: definition.path.to_owned(),
        description: definition.description.to_owned(),
        request_schema,
        variants,
    }
}

fn parameter(parameter: &catalog::Parameter) -> ApiParameter {
    ApiParameter {
        name: parameter.name.to_owned(),
        location: parameter.location.to_owned(),
        required: parameter.required,
        schema: parameter.schema.clone(),
    }
}

/// Map one canonical transport refusal to the semantic api failure,
/// keeping the refusal envelope unchanged for structured output.
pub(crate) fn api_error(failure: &provenance_core::protocol::failure::ErasedFailure) -> ApiError {
    let kind = match failure.error.get("kind").and_then(Value::as_str) {
        Some("unknown_operation") => ApiErrorKind::UnknownPath,
        Some("method_not_allowed") => ApiErrorKind::MethodNotAllowed,
        Some("access_denied") => ApiErrorKind::AccessDenied,
        _ => ApiErrorKind::Operation,
    };
    ApiError {
        kind,
        failure: serde_json::to_value(failure)
            .unwrap_or_else(|_| json!({"error": {"kind": "internal"}, "meta": {}})),
    }
}

pub(super) fn is_available(host: &crate::StatementHost) -> bool {
    catalog::definitions()
        .iter()
        .any(|definition| host.advertises(definition.name))
}
