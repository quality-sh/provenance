//! Target-first routing over registered record operations.

use axum::http::HeaderMap;
use provenance_contract::Definition;
use provenance_core::protocol::failure::{ErasedFailure, InvalidInputReason, OperationFailure};
use provenance_core::NodeType;
use provenance_macros::rule;
pub use provenance_porcelain::action::{Action, ActionError};
use serde_json::{json, Value};
use std::collections::BTreeMap;

use super::get_port;

/// A registered operation selected for one target-first action.
pub struct TargetRoute {
    pub action: Action,
    pub target: String,
    pub kind: NodeType,
    pub definition: &'static Definition,
    pub path: String,
}

impl crate::StatementHost {
    /// Resolve the target kind and select its registered operation.
    #[rule("rule_porcelain_named_domain_actions")]
    pub async fn target_route(
        &self,
        action: Action,
        target: &str,
        create_kind: Option<NodeType>,
    ) -> Result<TargetRoute, ActionError> {
        let selected = self
            .porcelain()
            .get()
            .select_target(action, target, create_kind)
            .await?;
        let kind = selected.kind;
        let definition = self.executable_target_definition(action, kind)?;
        let path = definition.path.replace("{id}", target);
        Ok(TargetRoute {
            action,
            target: target.to_owned(),
            kind,
            definition,
            path,
        })
    }

    /// Invoke the selected registered operation without a network loopback.
    pub async fn invoke_target(
        &self,
        route: &TargetRoute,
        mut data: Value,
        headers: HeaderMap,
    ) -> Result<Value, ErasedFailure> {
        if route.action == Action::Create {
            let object = data
                .as_object_mut()
                .ok_or_else(|| crate::routing::invalid(None))?;
            if object.insert("id".into(), json!(route.target)).is_some() {
                return Err(crate::routing::invalid(Some("id")));
            }
        }
        let method = match route.definition.method {
            provenance_contract::HttpMethod::Get => axum::http::Method::GET,
            provenance_contract::HttpMethod::Post => axum::http::Method::POST,
            provenance_contract::HttpMethod::Patch => axum::http::Method::PATCH,
        };
        self.invoke_resource(method, &route.path, data, BTreeMap::new(), headers)
            .await
    }

    pub(crate) fn executable_target_definitions(
        &self,
        action: Action,
    ) -> Vec<(NodeType, &'static Definition)> {
        provenance_contract::target_definitions(action)
            .filter_map(|(kind, _)| {
                self.executable_target_definition(action, kind)
                    .ok()
                    .map(|definition| (kind, definition))
            })
            .collect()
    }

    fn executable_target_definition(
        &self,
        action: Action,
        kind: NodeType,
    ) -> Result<&'static Definition, ActionError> {
        let definition = provenance_contract::target_definition(action, kind)
            .ok_or(ActionError::InvalidOptions)?;
        if !self.advertises(definition.name) {
            return Err(ActionError::AccessDenied);
        }
        if action != Action::Create && !get_port::resolver_permits(self, kind) {
            return Err(ActionError::NotFound);
        }
        Ok(definition)
    }
}

/// Split structured target-action arguments into the body, query, and headers of one
/// registered operation. The target fills each path parameter.
pub fn target_call(
    definition: &'static Definition,
    value: &Value,
    target: &str,
) -> Result<(Value, BTreeMap<String, String>, HeaderMap), ErasedFailure> {
    let mut path: BTreeMap<String, String> = definition
        .registration
        .request
        .path
        .iter()
        .map(|binding| (binding.parameter.to_owned(), target.to_owned()))
        .collect();
    let mut arguments = value.as_object().cloned().ok_or_else(invalid)?;
    let data = arguments.remove("data").unwrap_or_else(|| json!({}));
    let mut query = BTreeMap::new();
    let mut headers = HeaderMap::new();
    let variants: Vec<provenance_contract::QueryVariant> = definition.query_variants();
    let parameters: Vec<provenance_contract::Parameter> = if variants.is_empty() {
        definition.parameters()
    } else {
        let selector: Option<&str> = arguments.get("query").and_then(Value::as_str);
        variants
            .into_iter()
            .find(|variant| variant.selector == selector)
            .ok_or_else(invalid)?
            .parameters
    };
    for parameter in parameters {
        if parameter.location == "path" && path.contains_key(parameter.name) {
            continue;
        }
        let mcp_name = if parameter.location == "header" {
            parameter.name.to_ascii_lowercase().replace('-', "_")
        } else {
            parameter.name.to_owned()
        };
        let value = arguments.remove(&mcp_name);
        if parameter.required && value.is_none() {
            return Err(invalid());
        }
        let Some(value) = value else { continue };
        let text = provenance_contract::serialize_parameter_value(&parameter, &value)
            .map_err(|_| invalid())?;
        match parameter.location {
            "path" => {
                path.insert(parameter.name.to_owned(), text);
            }
            "query" => {
                query.insert(parameter.name.to_owned(), text);
            }
            "header" => {
                let name = axum::http::HeaderName::from_bytes(parameter.name.as_bytes())
                    .map_err(|_| invalid())?;
                let value = text.parse().map_err(|_| invalid())?;
                headers.insert(name, value);
            }
            _ => return Err(invalid()),
        }
    }
    if !arguments.is_empty() {
        return Err(invalid());
    }
    Ok((data, query, headers))
}

fn invalid() -> ErasedFailure {
    ErasedFailure::new(
        None,
        OperationFailure::InvalidInput {
            field: None,
            reason: InvalidInputReason::InvalidValue,
        },
    )
}
