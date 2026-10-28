//! The opened repository: the one door to canonical state.
//!
//! A command or a host opens one handle for a root and a scope. The handle
//! holds the canonical layout, the selected scope and the settings read at
//! open, and gives out stores. An operation takes the handle; it does not
//! discover, canonicalize or re-read settings itself.

use crate::layout::{with_initialized_graph, GraphNotInitialized, ProvenanceLayout};
use crate::settings::{Settings, SettingsError};
use crate::state_store::StateStore;
use crate::write_error::{SourceFailure, WriteError, WriteFailure};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};
use provenance_core::{Manifest, ScopeId};
use provenance_macros::{rule, verifies};
use std::path::Path;
use std::sync::Arc;

/// One opened repository. A clone shares the open; one open per command or host.
#[derive(Clone, Debug)]
#[verifies("rule_operation_acts_on_opened_repository", construction)]
pub struct Repository {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    identity: RepositoryIdentity,
    layout: ProvenanceLayout,
}

/// What a repository is, apart from where its state lives: the working tree,
/// the selected scope and the settings read at open. The identity has no
/// store constructor, so a holder cannot reach canonical state through it.
#[derive(Clone, Debug)]
pub struct RepositoryIdentity {
    root: Utf8PathBuf,
    scope: ScopeId,
    settings: Arc<Settings>,
}

impl RepositoryIdentity {
    /// The canonical working-tree root.
    pub fn working_tree(&self) -> &Utf8Path {
        &self.root
    }

    /// The scope the handle selected at open.
    pub const fn scope(&self) -> &ScopeId {
        &self.scope
    }

    /// The settings read once at open.
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}

/// Why a repository did not open.
#[derive(Debug, thiserror::Error)]
pub enum OpenFailure {
    #[error("resolve repository `{path}`: {error}")]
    InvalidRoot {
        path: String,
        #[source]
        error: std::io::Error,
    },
    #[error("{}", GraphNotInitialized)]
    NotInitialized,
    #[error("unknown scope `{selected}`; declared scopes: {}", .declared.join(", "))]
    UnknownScope {
        selected: String,
        declared: Vec<String>,
    },
    #[error(transparent)]
    Settings(#[from] SettingsError),
    /// The initialized-graph gate did not complete: an I/O failure, a lock
    /// failure, a manifest that is not a file, or a manifest that does not parse.
    #[error("{0:#}")]
    Io(anyhow::Error),
}

impl From<OpenFailure> for OperationFailure {
    fn from(failure: OpenFailure) -> Self {
        match failure {
            OpenFailure::InvalidRoot { .. } => Self::InvalidInput {
                field: None,
                reason: InvalidInputReason::InvalidValue,
            },
            OpenFailure::UnknownScope { .. } => Self::UnknownScope,
            OpenFailure::NotInitialized | OpenFailure::Settings(_) | OpenFailure::Io(_) => {
                Self::Internal
            }
        }
    }
}

/// A request named a scope other than the one the handle selected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("request scope `{}` does not match the selected scope `{}`", .requested.as_str(), .selected.as_str())]
pub struct ScopeMismatch {
    pub selected: ScopeId,
    pub requested: ScopeId,
}

impl From<ScopeMismatch> for WriteError {
    fn from(mismatch: ScopeMismatch) -> Self {
        Self(SourceFailure::wrap(WriteFailure::ScopeMismatch, mismatch))
    }
}

impl Repository {
    /// Opens the repository at `root` for one scope.
    ///
    /// The root is canonicalized, the initialized-graph gate runs once, the
    /// manifest must declare the scope, and the settings file is read once.
    #[rule("rule_repository_reads_settings_once_at_open")]
    pub fn open(root: impl AsRef<Path>, scope: &str) -> Result<Self, OpenFailure> {
        let root = canonical_root(root.as_ref())?;
        let layout = ProvenanceLayout::new(root.clone());
        let manifest = with_initialized_graph(&layout, Ok).map_err(gate_failure)?;
        let scope = declared_scope(&manifest, scope)?;
        let settings = Settings::load(&layout)?;
        Ok(Self {
            inner: Arc::new(Inner {
                identity: RepositoryIdentity {
                    root,
                    scope,
                    settings: Arc::new(settings),
                },
                layout,
            }),
        })
    }

    /// Opens one explicit root, or the nearest enclosing project of the
    /// current directory: the first ancestor with a manifest or a `.git`.
    #[rule("rule_sdk_project_discovery")]
    pub fn discover(root: Option<Utf8PathBuf>, scope: &str) -> Result<Self, OpenFailure> {
        let cwd = std::env::current_dir().map_err(|error| OpenFailure::Io(error.into()))?;
        let cwd = Utf8PathBuf::from_path_buf(cwd).map_err(|path| {
            OpenFailure::Io(anyhow::anyhow!(
                "current directory is not UTF-8: {}",
                path.display()
            ))
        })?;
        let Some(root) = root else {
            let found = cwd
                .ancestors()
                .find(|ancestor| {
                    ancestor.join(".provenance/state/manifest.json").is_file()
                        || ancestor.join(".git").exists()
                })
                .unwrap_or(cwd.as_path())
                .to_path_buf();
            return Self::open(found, scope);
        };
        if root.is_relative() {
            Self::open(cwd.join(root), scope)
        } else {
            Self::open(root, scope)
        }
    }

    /// The working tree, scope and settings of the open.
    pub fn identity(&self) -> &RepositoryIdentity {
        &self.inner.identity
    }

    /// The canonical working-tree root.
    pub fn working_tree(&self) -> &Utf8Path {
        self.inner.identity.working_tree()
    }

    /// The scope the handle selected at open.
    pub fn scope(&self) -> &ScopeId {
        self.inner.identity.scope()
    }

    /// The settings read once at open.
    pub fn settings(&self) -> &Settings {
        self.inner.identity.settings()
    }

    /// Refuses a request that names another scope, before any read or write.
    #[rule("rule_operation_scope_is_the_opened_scope")]
    pub fn ensure_selected(&self, requested: &ScopeId) -> Result<(), ScopeMismatch> {
        if requested == self.scope() {
            Ok(())
        } else {
            Err(ScopeMismatch {
                selected: self.scope().clone(),
                requested: requested.clone(),
            })
        }
    }

    /// The physical layout, for the path-level APIs: shards, publication,
    /// settings, the dictionary reference and the review watcher.
    pub fn layout(&self) -> &ProvenanceLayout {
        &self.inner.layout
    }

    /// A fresh read of the manifest.
    pub fn manifest(&self) -> anyhow::Result<Manifest> {
        self.store().manifest()
    }

    /// A store over the live state of this repository.
    #[rule("rule_operation_acts_on_opened_repository")]
    pub fn store(&self) -> StateStore {
        StateStore::new(self.inner.layout.clone())
    }
}

/// The manifest scope with the selected id. A scope that the manifest does
/// not declare is refused with the declared scopes, so a mistyped scope never
/// reads as an empty one.
#[rule("rule_graph_report_refuses_undeclared_scope")]
fn declared_scope(manifest: &Manifest, selected: &str) -> Result<ScopeId, OpenFailure> {
    manifest
        .scopes
        .iter()
        .find(|scope| scope.id.as_str() == selected)
        .map(|scope| scope.id.clone())
        .ok_or_else(|| OpenFailure::UnknownScope {
            selected: selected.to_owned(),
            declared: manifest
                .scopes
                .iter()
                .map(|scope| scope.id.as_str().to_owned())
                .collect(),
        })
}

fn canonical_root(path: &Path) -> Result<Utf8PathBuf, OpenFailure> {
    let invalid = |error: std::io::Error| OpenFailure::InvalidRoot {
        path: path.display().to_string(),
        error,
    };
    let canonical = std::fs::canonicalize(path).map_err(invalid)?;
    if !canonical.is_dir() {
        return Err(invalid(std::io::Error::other("is not a directory")));
    }
    Utf8PathBuf::from_path_buf(canonical)
        .map_err(|_| invalid(std::io::Error::other("is not UTF-8")))
}

fn gate_failure(error: anyhow::Error) -> OpenFailure {
    if error.downcast_ref::<GraphNotInitialized>().is_some() {
        OpenFailure::NotInitialized
    } else {
        OpenFailure::Io(error)
    }
}
