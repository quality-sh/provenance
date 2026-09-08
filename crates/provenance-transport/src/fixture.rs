//! Explicit test policy. This module is absent without the test-fixture feature.
pub mod records;
use provenance_core::{
    protocol::failure::{InvalidInputReason, OperationFailure},
    Manifest, ScopeId,
};
use provenance_store::{
    layout::ProvenanceLayout,
    operations::{
        catalog::{
            ContextResolver, ExecutionNeeds, PreparedContext, PreparedRead, PreparedRepository,
            RequestedContext,
        },
        read_policy::ReadPolicy,
    },
    settings::Settings,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

pub struct Target {
    pub id: String,
    pub root: PathBuf,
}

/// A fixed target map and explicit grants for one fixture principal.
/// A caller-owned MCP stream represents this same authenticated fixture principal.
pub struct FixtureAccess {
    targets: BTreeMap<String, PathBuf>,
    grants: BTreeSet<(String, String)>,
    token: String,
    authority: String,
    denied_operations: BTreeSet<String>,
}
impl FixtureAccess {
    pub fn new(
        targets: Vec<Target>,
        grants: Vec<(String, String)>,
        token: &str,
        authority: &str,
    ) -> Result<Self, OperationFailure> {
        let invalid = || OperationFailure::InvalidInput {
            field: None,
            reason: InvalidInputReason::InvalidValue,
        };
        if token.is_empty() || authority.is_empty() || authority.chars().any(char::is_control) {
            return Err(invalid());
        }
        let mut mapped = BTreeMap::new();
        for target in targets {
            if target.id.is_empty()
                || target.id.contains(['/', '\\'])
                || target.id.chars().any(char::is_control)
                || !target.root.is_absolute()
            {
                return Err(invalid());
            }
            let root = target.root.canonicalize().map_err(|_| invalid())?;
            if !root.is_dir() || root.to_str().is_none() || mapped.insert(target.id, root).is_some()
            {
                return Err(invalid());
            }
        }
        let grants: BTreeSet<_> = grants.into_iter().collect();
        if grants
            .iter()
            .any(|(target, scope)| !mapped.contains_key(target) || ScopeId::new(scope).is_err())
        {
            return Err(invalid());
        }
        Ok(Self {
            targets: mapped,
            grants,
            token: token.to_owned(),
            authority: authority.to_owned(),
            denied_operations: BTreeSet::new(),
        })
    }
    #[must_use]
    pub fn deny_operation(mut self, operation: &str) -> Self {
        self.denied_operations.insert(operation.to_owned());
        self
    }
    pub(crate) fn permits_operation(&self, operation: &str) -> bool {
        !self.denied_operations.contains(operation)
    }
    pub(crate) fn authenticate(
        &self,
        headers: &axum::http::HeaderMap,
    ) -> Result<(), OperationFailure> {
        let credential = headers
            .get("authorization")
            .and_then(|header| header.to_str().ok());
        if credential != Some(format!("Bearer {}", self.token).as_str()) {
            return Err(OperationFailure::Unauthenticated);
        }
        if headers.get("host").and_then(|header| header.to_str().ok())
            != Some(self.authority.as_str())
            || headers.contains_key("origin")
        {
            return Err(OperationFailure::AccessDenied);
        }
        Ok(())
    }
}
impl ContextResolver for FixtureAccess {
    fn prepare(
        &self,
        operation: &'static str,
        context: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        let (target, selected) = match context {
            RequestedContext::Repository(context) => (context.repository, None),
            RequestedContext::Scoped(context) => (context.repository.clone(), Some(context)),
        };
        let root = self
            .targets
            .get(&target)
            .ok_or(OperationFailure::UnknownTarget)?;
        let granted = selected.as_ref().map_or_else(
            || self.grants.iter().any(|(granted, _)| granted == &target),
            |context| {
                self.grants
                    .contains(&(target.clone(), context.scope.clone()))
            },
        );
        if !self.permits_operation(operation) || !granted {
            return Err(OperationFailure::AccessDenied);
        }
        let Some(context) = selected else {
            return Ok(PreparedContext::for_repository(PreparedRepository {
                root: root.to_str().ok_or(OperationFailure::Internal)?.into(),
                requested_target: target,
            }));
        };
        let scope = ScopeId::new(context.scope).map_err(|_| OperationFailure::InvalidInput {
            field: Some("context.scope".to_owned()),
            reason: InvalidInputReason::InvalidValue,
        })?;
        let layout = ProvenanceLayout::new(root.to_str().ok_or(OperationFailure::Internal)?);
        // This policy load does not acquire a publication lock, recover, or open a cache.
        let bytes =
            std::fs::read(layout.manifest_path()).map_err(|_| OperationFailure::Internal)?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|_| OperationFailure::Internal)?;
        if !manifest
            .scopes
            .iter()
            .any(|candidate| candidate.id == scope)
        {
            return Err(OperationFailure::UnknownScope);
        }
        let settings = Settings::load(&layout).map_err(|_| OperationFailure::Internal)?;
        Ok(PreparedContext::read(PreparedRead {
            root: root.to_str().ok_or(OperationFailure::Internal)?.into(),
            scope,
            policy: ReadPolicy::resolve(&settings, context.freshness),
            requested_target: context.repository,
            external: true,
        }))
    }
}
