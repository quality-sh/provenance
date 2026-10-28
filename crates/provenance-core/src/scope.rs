use crate::{Manifest, Scope, ScopeId};
use camino::{Utf8Component, Utf8Path};
use provenance_macros::rule;
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
        .filter_map(|scope| prefix_depth(scope, repo_relative_path).map(|depth| (scope, depth)))
        .max_by_key(|(_, depth)| *depth)
        .map(|(scope, _)| scope.id.clone())
        .ok_or_else(|| ScopeResolutionError::MissingScope {
            path: repo_relative_path.to_string(),
        })
}

/// Whether the scope owns a repository-relative path: its path prefix
/// contains the path, and no other scope has a longer prefix that contains
/// it. Scopes that declare the same prefix own the same paths.
#[rule("rule_binding_scope_longest_prefix")]
pub fn scope_owns_path(
    manifest: &Manifest,
    scope: &ScopeId,
    repo_relative_path: &Utf8Path,
) -> bool {
    let Some(own) = manifest
        .scopes
        .iter()
        .filter(|candidate| &candidate.id == scope)
        .find_map(|candidate| prefix_depth(candidate, repo_relative_path))
    else {
        return false;
    };
    manifest
        .scopes
        .iter()
        .filter_map(|candidate| prefix_depth(candidate, repo_relative_path))
        .all(|depth| depth <= own)
}

/// The number of path components in the scope's prefix when the prefix
/// contains the path. `.` and `./` components count as nothing, so `.` owns
/// every path and `./app/` is the same prefix as `app`.
fn prefix_depth(scope: &Scope, repo_relative_path: &Utf8Path) -> Option<usize> {
    let prefix = normal_components(scope.path_prefix.as_path());
    let path = normal_components(repo_relative_path);
    path.starts_with(&prefix).then_some(prefix.len())
}

fn normal_components(path: &Utf8Path) -> Vec<&str> {
    path.components()
        .filter(|component| *component != Utf8Component::CurDir)
        .map(|component| component.as_str())
        .collect()
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

    #[test]
    #[provenance_macros::verifies("rule_binding_scope_longest_prefix", examples)]
    fn the_longest_prefix_owns_a_path_and_shared_prefixes_share_it() {
        let manifest = manifest_with_scopes(&[
            ("root", "."),
            ("app", "./app/"),
            ("a", "a"),
            ("ab", "a/b"),
            ("twin", "a/b"),
        ]);
        let owners = |path: &str| {
            manifest
                .scopes
                .iter()
                .filter(|scope| scope_owns_path(&manifest, &scope.id, Utf8Path::new(path)))
                .map(|scope| scope.id.as_str())
                .collect::<Vec<_>>()
        };

        assert_eq!(owners("src/lib.rs"), ["root"]);
        assert_eq!(owners("app/src/lib.rs"), ["app"]);
        assert_eq!(owners("application/lib.rs"), ["root"]);
        assert_eq!(owners("a/lib.rs"), ["a"]);
        assert_eq!(owners("a/b/lib.rs"), ["ab", "twin"]);
        assert!(!scope_owns_path(
            &manifest,
            &ScopeId::new("missing").unwrap(),
            Utf8Path::new("src/lib.rs")
        ));
    }
}
