use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use super::ids::{SchemaVersion, ScopeId};
use crate::SUPPORTED_SCHEMA_VERSION;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RepoPathPrefix(Utf8PathBuf);

impl RepoPathPrefix {
    pub fn new(value: impl Into<Utf8PathBuf>) -> Self {
        Self(value.into())
    }
    pub fn as_path(&self) -> &camino::Utf8Path {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scope {
    pub id: ScopeId,
    pub path_prefix: RepoPathPrefix,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: SchemaVersion,
    pub scopes: Vec<Scope>,
    #[serde(default)]
    pub disposition_actor_ids: Vec<String>,
}

impl Manifest {
    pub fn default_with_scope(scope: ScopeId, path_prefix: RepoPathPrefix) -> Self {
        Self {
            schema_version: SUPPORTED_SCHEMA_VERSION,
            scopes: vec![Scope {
                id: scope,
                path_prefix,
            }],
            disposition_actor_ids: Vec::new(),
        }
    }
}
