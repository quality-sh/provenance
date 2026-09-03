use crate::{Manifest, ScopeId};
use camino::Utf8Path;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScopeResolutionError {
    #[error("scope '{0}' is not defined in .provenance/state/manifest.json")]
    UnknownScope(String),
    #[error("could not infer a provenance scope for '{path}'. Pass --scope, set PROVENANCE_SCOPE, or run provenance init")]
    MissingScope { path: String },
}

#[derive(Debug, Default)]
pub struct Env(HashMap<String, String>);

impl Env {
    pub fn from_pairs<const N: usize>(pairs: [(&str, &str); N]) -> Self {
        Self(
            pairs
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        )
    }
    pub fn process() -> Self {
        Self(std::env::vars().collect())
    }
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
}

pub fn resolve_scope(
    explicit: Option<&str>,
    env: &Env,
    manifest: &Manifest,
    repo_relative_path: &Utf8Path,
) -> Result<ScopeId, ScopeResolutionError> {
    if let Some(scope) = explicit.or_else(|| env.get("PROVENANCE_SCOPE")) {
        return manifest
            .scopes
            .iter()
            .find(|candidate| candidate.id.as_str() == scope)
            .map(|candidate| candidate.id.clone())
            .ok_or_else(|| ScopeResolutionError::UnknownScope(scope.to_string()));
    }

    manifest
        .scopes
        .iter()
        .filter(|scope| {
            scope.path_prefix.as_path().as_str() == "."
                || repo_relative_path.starts_with(scope.path_prefix.as_path())
        })
        .max_by_key(|scope| scope.path_prefix.as_path().as_str().len())
        .map(|scope| scope.id.clone())
        .ok_or_else(|| ScopeResolutionError::MissingScope {
            path: repo_relative_path.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SUPPORTED_SCHEMA_VERSION;
    use crate::{RepoPathPrefix, Scope};

    fn manifest_with_scopes(scopes: &[(&str, &str)]) -> Manifest {
        Manifest {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scopes: scopes
                .iter()
                .map(|(id, prefix)| Scope {
                    id: ScopeId::new(*id).unwrap(),
                    path_prefix: RepoPathPrefix::new(*prefix),
                })
                .collect(),
            disposition_actor_ids: Vec::new(),
        }
    }

    #[test]
    fn resolves_scope_from_explicit_arg_before_env_and_manifest() {
        let manifest = manifest_with_scopes(&[("api", "services/api"), ("root", ".")]);
        let resolved = resolve_scope(
            Some("api"),
            &Env::from_pairs([("PROVENANCE_SCOPE", "root")]),
            &manifest,
            Utf8Path::new("services/web"),
        )
        .unwrap();
        assert_eq!(resolved.as_str(), "api");
    }

    #[test]
    fn resolves_longest_matching_manifest_prefix() {
        let manifest = manifest_with_scopes(&[("root", "."), ("api", "services/api")]);
        let resolved = resolve_scope(
            None,
            &Env::default(),
            &manifest,
            Utf8Path::new("services/api/src/lib.rs"),
        )
        .unwrap();
        assert_eq!(resolved.as_str(), "api");
    }
}
