//! Target-first routing over registered record operations.

use axum::http::HeaderMap;
use provenance_core::{protocol::RecordResolution, NodeType};
use provenance_macros::rule;
use provenance_store::operations::catalog::{self, Definition};
pub use provenance_store::operations::catalog::TargetAction as Action;
use serde_json::{json, Value};
use std::{collections::BTreeMap, fmt::Display};

use super::get_port;

/// A registered operation selected for one target-first action.
pub struct TargetRoute {
    pub action: Action,
    pub target: String,
    pub kind: NodeType,
    pub definition: &'static Definition,
    pub path: String,
}

/// A target-first action that cannot be routed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionError {
    InvalidOptions,
    NotFound,
    AmbiguousIdentity,
    Operation(String),
}

impl Display for ActionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidOptions => formatter.write_str("unsupported action options"),
            Self::NotFound => formatter.write_str("record does not exist"),
            Self::AmbiguousIdentity => formatter.write_str("record ID is not unique"),
            Self::Operation(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ActionError {}

impl crate::StatementHost {
    /// Resolve the target kind and select its registered operation.
    #[rule("rule_porcelain_create_names_new_record")]
    #[rule("rule_porcelain_existing_action_infers_kind")]
    #[rule("rule_porcelain_named_domain_actions")]
    pub async fn target_route(
        &self,
        action: Action,
        target: &str,
        create_kind: Option<NodeType>,
    ) -> Result<TargetRoute, ActionError> {
        if target.is_empty() || (action == Action::Create) != create_kind.is_some() {
            return Err(ActionError::InvalidOptions);
        }
        let kind = if let Some(kind) = create_kind {
            kind
        } else {
            let response = get_port::resolve(self, target)
                .await
                .map_err(ActionError::Operation)?;
            match response {
                RecordResolution::Found(record) => record.node_type(),
                RecordResolution::Missing => return Err(ActionError::NotFound),
                RecordResolution::Ambiguous => return Err(ActionError::AmbiguousIdentity),
            }
        };
        let definition = definition(action, kind).ok_or(ActionError::InvalidOptions)?;
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
    ) -> Result<Value, provenance_core::protocol::failure::ErasedFailure> {
        if route.action == Action::Create {
            let object = data
                .as_object_mut()
                .ok_or_else(|| crate::routing::invalid(None))?;
            if object.insert("id".into(), json!(route.target)).is_some() {
                return Err(crate::routing::invalid(Some("id")));
            }
        }
        let method = match route.definition.method {
            catalog::HttpMethod::Get => axum::http::Method::GET,
            catalog::HttpMethod::Post => axum::http::Method::POST,
            catalog::HttpMethod::Patch => axum::http::Method::PATCH,
        };
        self.invoke_resource(method, &route.path, data, BTreeMap::new(), headers)
            .await
    }
}

fn definition(action: Action, kind: NodeType) -> Option<&'static Definition> {
    catalog::target_definition(action, kind)
}

/// Render one catalog result for a reader while preserving the structured value.
pub fn render_readable(action: Action, target: &str, kind: NodeType, value: &Value) -> String {
    format!(
        "{} {} {}\n\n{}",
        action.as_str(),
        kind.as_str(),
        target,
        serde_json::to_string_pretty(value.get("data").unwrap_or(value))
            .expect("registered output is JSON")
    )
}
