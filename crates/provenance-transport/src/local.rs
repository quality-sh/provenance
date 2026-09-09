//! Access for one local caller, repository target, and scope.
use crate::HostAccess;
use axum::http::HeaderMap;
use provenance_core::{
    protocol::failure::{InvalidInputReason, OperationFailure},
    Manifest, ScopeId,
};
use provenance_store::{
    layout::ProvenanceLayout,
    operations::{
        catalog::{
            self, ContextResolver, ExecutionNeed, ExecutionNeeds, PreparedContext, PreparedRead,
            PreparedRepository, PreparedScope, RequestedContext,
        },
        read_policy::ReadPolicy,
    },
    settings::Settings,
};
use std::{net::SocketAddr, path::Path};

pub struct LocalAccess {
    layout: ProvenanceLayout,
    root: String,
    repository: String,
    scope: ScopeId,
    credential: String,
    authority: String,
    origin: String,
}

impl LocalAccess {
    /// Bind a caller credential to one existing repository and scope.
    pub fn new(
        root: &Path,
        repository: &str,
        scope: &str,
        token: &str,
        address: SocketAddr,
    ) -> Result<Self, OperationFailure> {
        let invalid = || OperationFailure::InvalidInput {
            field: None,
            reason: InvalidInputReason::InvalidValue,
        };
        if repository.is_empty()
            || repository.contains(['/', '\\'])
            || repository.chars().any(char::is_control)
            || token.len() != 64
            || !token.bytes().all(|byte| byte.is_ascii_hexdigit())
            || address.ip() != std::net::Ipv4Addr::LOCALHOST
            || address.port() == 0
        {
            return Err(invalid());
        }
        let root = root.canonicalize().map_err(|_| invalid())?;
        if !root.is_dir() {
            return Err(invalid());
        }
        let layout = ProvenanceLayout::new(root.to_str().ok_or_else(invalid)?);
        let scope = ScopeId::new(scope).map_err(|_| invalid())?;
        let access = Self {
            root: root.to_str().ok_or_else(invalid)?.to_owned(),
            layout,
            repository: repository.to_owned(),
            scope,
            credential: format!("Bearer {token}"),
            authority: address.to_string(),
            origin: format!("http://{address}"),
        };
        access.check_scope()?;
        Ok(access)
    }

    /// Check the destination and browser origin on both assets and operations.
    pub fn check_origin(&self, headers: &HeaderMap) -> Result<(), OperationFailure> {
        if single(headers, "host") != Some(self.authority.as_str())
            || (headers.contains_key("origin")
                && single(headers, "origin") != Some(self.origin.as_str()))
            || (headers.contains_key("sec-fetch-site")
                && !matches!(
                    single(headers, "sec-fetch-site"),
                    Some("same-origin" | "none")
                ))
        {
            return Err(OperationFailure::AccessDenied);
        }
        Ok(())
    }

    fn check_scope(&self) -> Result<(), OperationFailure> {
        let bytes =
            std::fs::read(self.layout.manifest_path()).map_err(|_| OperationFailure::Internal)?;
        let manifest: Manifest =
            serde_json::from_slice(&bytes).map_err(|_| OperationFailure::Internal)?;
        if !manifest.scopes.iter().any(|scope| scope.id == self.scope) {
            return Err(OperationFailure::UnknownScope);
        }
        Ok(())
    }
}

fn single<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let mut values = headers.get_all(name).iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value)
}

impl HostAccess for LocalAccess {
    fn authenticate(&self, headers: &HeaderMap) -> Result<(), OperationFailure> {
        let supplied = single(headers, "authorization")
            .unwrap_or_default()
            .as_bytes();
        let expected = self.credential.as_bytes();
        let difference = supplied
            .iter()
            .zip(expected)
            .fold(0, |acc, (a, b)| acc | (a ^ b));
        if supplied.len() != expected.len() || difference != 0 {
            return Err(OperationFailure::Unauthenticated);
        }
        self.check_origin(headers)
    }

    fn advertises(&self, operation: &str) -> bool {
        catalog::contains(operation)
    }
}

impl ContextResolver for LocalAccess {
    fn prepare(
        &self,
        operation: &'static str,
        context: RequestedContext,
        needs: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        let (repository, scope, freshness) = match context {
            RequestedContext::Repository(context) => (context.repository, None, None),
            RequestedContext::Scope(context) => (context.repository, Some(context.scope), None),
            RequestedContext::Scoped(context) => {
                (context.repository, Some(context.scope), context.freshness)
            }
        };
        if repository != self.repository {
            return Err(OperationFailure::UnknownTarget);
        }
        if !self.advertises(operation)
            || scope
                .as_deref()
                .is_some_and(|scope| scope != self.scope.as_str())
        {
            return Err(OperationFailure::AccessDenied);
        }
        let root = self.root.clone().into();
        if scope.is_none() {
            return Ok(PreparedContext::for_repository(PreparedRepository {
                root,
                requested_target: repository,
            }));
        }
        // Validate the grant before settings, recovery, or projection access.
        self.check_scope()?;
        if !needs.contains(&ExecutionNeed::ProjectionMaintenance) {
            return Ok(PreparedContext::for_scope(PreparedScope {
                root,
                scope: self.scope.clone(),
                requested_target: repository,
            }));
        }
        let settings = Settings::load(&self.layout).map_err(|_| OperationFailure::Internal)?;
        Ok(PreparedContext::read(PreparedRead {
            root,
            scope: self.scope.clone(),
            policy: ReadPolicy::resolve(&settings, freshness),
            requested_target: repository,
            external: true,
        }))
    }
}
