//! Target-first routing over registered record operations.

use axum::http::HeaderMap;
use provenance_core::NodeType;
use provenance_macros::rule;
pub use provenance_porcelain::action::{description, render_readable, Action, ActionError};
use provenance_store::operations::catalog::{self, Definition};
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
        let selected = provenance_porcelain::Porcelain::new(super::HostGetPort::new(self.clone()))
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

    pub(super) fn executable_target_definitions(
        &self,
        action: Action,
    ) -> Vec<(NodeType, &'static Definition)> {
        catalog::target_definitions(action)
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
        let definition =
            catalog::target_definition(action, kind).ok_or(ActionError::InvalidOptions)?;
        if !self.advertises(definition.name) {
            return Err(ActionError::AccessDenied);
        }
        if action != Action::Create && !get_port::resolver_permits(self, kind) {
            return Err(ActionError::NotFound);
        }
        Ok(definition)
    }
}
