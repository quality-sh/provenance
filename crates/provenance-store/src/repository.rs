//! The opened repository: the one door to canonical state.
//!
//! A command or a host opens one handle for a root and a scope. The handle
//! holds the canonical layout, the selected scope and the settings read at
//! open, and gives out stores. An operation takes the handle; it does not
//! discover, canonicalize or re-read settings itself.

#[cfg(feature = "host")]
use crate::layout::with_initialized_graph;
use crate::layout::{GraphNotInitialized, ProvenanceLayout};
use crate::publication::SectionHandle;
use crate::settings::{Settings, SettingsError};
#[cfg(feature = "host")]
use crate::stale::revision_scans::KeptScans;
use crate::state_store::StateStore;
#[cfg(feature = "host")]
use crate::storage::local::{LocalSectionLock, LocalStateStorage};
use crate::storage::{SectionLock, StateKey, StateStorage};
use crate::write_error::{SourceFailure, WriteError, WriteFailure};
use camino::Utf8Path;
#[cfg(feature = "host")]
use camino::Utf8PathBuf;
use provenance_core::protocol::failure::{InvalidInputReason, OperationFailure};
use provenance_core::{Manifest, Scope, ScopeId, Stamp};
use provenance_macros::{rule, verifies};
#[cfg(feature = "host")]
use std::path::Path;
use std::sync::Arc;

mod identity;
pub use identity::{RepositoryHistory, RepositoryIdentity};

/// One opened repository. A clone shares the open; one open per command or host.
#[derive(Clone, Debug)]
#[rule("rule_repository_handle_stays_a_handle")]
#[verifies("rule_operation_acts_on_opened_repository", construction)]
pub struct Repository {
    inner: Arc<Inner>,
    /// Set only on the repository of a section guard.
    section: Option<SectionHandle>,
}

#[derive(Debug)]
struct Inner {
    identity: RepositoryIdentity,
    layout: ProvenanceLayout,
    storage: Arc<dyn StateStorage>,
    section_lock: Arc<dyn SectionLock>,
    /// Whether `storage` is the local adapter of `layout`, which `open`
    /// makes.
    local: bool,
    /// The source scans of the last commit range that a stale or evidence
    /// query of this open read. Each clone of the handle shares them.
    #[cfg(feature = "host")]
    kept_scans: KeptScans,
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
    /// manifest must declare the scope, and the settings file and the
    /// project dictionary are read once.
    #[rule("rule_repository_reads_settings_once_at_open")]
    #[cfg(feature = "host")]
    pub fn open(root: impl AsRef<Path>, scope: &str) -> Result<Self, OpenFailure> {
        let root = canonical_root(root.as_ref())?;
        let layout = ProvenanceLayout::new(root.clone());
        let manifest = with_initialized_graph(&layout, Ok).map_err(gate_failure)?;
        let scope = declared_scope(&manifest, scope)?.id.clone();
        let settings = Settings::load(&layout)?;
        let dictionary = crate::dictionary_reference::load_project_dictionary(&layout);
        let history = Arc::new(crate::state_store::GitHistory::new(root.clone()));
        Ok(Self::with_parts(
            RepositoryIdentity::new(root, scope, settings)
                .with_dictionary(dictionary)
                .with_history(history),
            Arc::new(LocalStateStorage::new(layout.clone())),
            Arc::new(LocalSectionLock::new(layout)),
            true,
        ))
    }

    /// Opens a repository over `storage`, whose sections `lock` serializes.
    ///
    /// The manifest comes from a view of the storage and must declare the
    /// scope of `identity`. The repository reads and writes canonical state
    /// only through the storage; the working tree of the identity names the
    /// layout of the paths that the engine keys state by.
    #[rule("rule_engine_runs_on_any_storage")]
    pub fn with_storage(
        identity: RepositoryIdentity,
        storage: Arc<dyn StateStorage>,
        lock: Arc<dyn SectionLock>,
    ) -> Result<Self, OpenFailure> {
        let view = storage
            .read_view()
            .map_err(|error| OpenFailure::Io(error.into()))?;
        let manifest_key = StateKey::new("manifest.json").expect("the manifest key is valid");
        let Some(manifest) = view.get(&manifest_key) else {
            return Err(OpenFailure::NotInitialized);
        };
        let manifest = crate::state_store::manifest_from_bytes(manifest)
            .map_err(|error| OpenFailure::Io(error.context("failed to parse the manifest")))?;
        declared_scope(&manifest, identity.scope().as_str())?;
        Ok(Self::with_parts(identity, storage, lock, false))
    }

    fn with_parts(
        identity: RepositoryIdentity,
        storage: Arc<dyn StateStorage>,
        section_lock: Arc<dyn SectionLock>,
        local: bool,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                layout: ProvenanceLayout::new(identity.working_tree().to_owned()),
                identity,
                storage,
                section_lock,
                local,
                #[cfg(feature = "host")]
                kept_scans: KeptScans::new(),
            }),
            section: None,
        }
    }

    /// Opens one explicit root, or the nearest enclosing project of the
    /// current directory: the first ancestor with a manifest or a `.git`.
    #[rule("rule_sdk_project_discovery")]
    #[cfg(feature = "host")]
    pub fn discover(root: Option<Utf8PathBuf>, scope: &str) -> Result<Self, OpenFailure> {
        if let Some(root) = root.as_ref().filter(|root| root.is_absolute()) {
            return Self::open(root, scope);
        }
        let cwd = std::env::current_dir().map_err(|error| OpenFailure::Io(error.into()))?;
        let cwd = Utf8PathBuf::from_path_buf(cwd).map_err(|path| {
            OpenFailure::Io(anyhow::anyhow!(
                "current directory is not UTF-8: {}",
                path.display()
            ))
        })?;
        if let Some(root) = root {
            return Self::open(cwd.join(root), scope);
        }
        let found = cwd
            .ancestors()
            .find(|ancestor| {
                ancestor
                    .join(".provenance")
                    .join("state")
                    .join("manifest.json")
                    .is_file()
                    || ancestor.join(".git").exists()
            })
            .unwrap_or(cwd.as_path())
            .to_path_buf();
        Self::open(found, scope)
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

    /// A fresh read of the manifest, under the publication lock.
    #[rule("rule_preparation_reads_published_state")]
    pub fn manifest(&self) -> anyhow::Result<Manifest> {
        self.store().manifest()
    }

    /// A store over the live state of this repository.
    ///
    /// The store is a trusted internal interface. Its public reads and writes
    /// take a scope id and do not check it against the scope that this handle
    /// selected. The catalog's `ScopedWrite` operations and
    /// [`Repository::ensure_selected`] enforce the scope.
    #[rule("rule_operation_acts_on_opened_repository")]
    pub fn store(&self) -> StateStore {
        StateStore::live(
            self.inner.identity.clone(),
            self.inner.layout.clone(),
            self.section.clone(),
            Arc::clone(self.storage()),
            Arc::clone(self.section_lock()),
            self.has_local_storage(),
        )
    }

    /// Whether the repository keeps its state in the local adapter of its
    /// working tree.
    pub(crate) fn has_local_storage(&self) -> bool {
        self.inner.local
    }

    /// The storage that holds the canonical state of this repository.
    pub(crate) fn storage(&self) -> &Arc<dyn StateStorage> {
        &self.inner.storage
    }

    /// The source scans that the stale and evidence queries of this open
    /// keep between calls.
    #[cfg(feature = "host")]
    pub(crate) fn kept_scans(&self) -> &KeptScans {
        &self.inner.kept_scans
    }

    /// The lock that serializes the sections of this repository.
    pub(crate) fn section_lock(&self) -> &Arc<dyn SectionLock> {
        &self.inner.section_lock
    }

    /// A store over one consistent view of the canonical state.
    ///
    /// The view is read in the publication section, after recovery; a handle
    /// that holds a section reads in that section and takes no second lock.
    /// The store answers every list and the manifest from the view, with no
    /// disk access, and refuses every write and stage. Derived state, such as
    /// verification runs, is not in the view.
    #[rule("rule_canonical_state_is_read_as_one_view")]
    pub fn view(&self) -> anyhow::Result<StateStore> {
        let view = self
            .store()
            .with_repository_publication(|_| Ok(self.storage().read_view()?))?;
        Ok(StateStore::over_view(
            self.inner.identity.clone(),
            self.inner.layout.clone(),
            Arc::new(view),
            None,
            Arc::clone(self.storage()),
            Arc::clone(self.section_lock()),
            self.has_local_storage(),
        ))
    }

    /// The section that this handle holds, if it came from a section guard.
    pub(crate) const fn section(&self) -> Option<&SectionHandle> {
        self.section.as_ref()
    }

    /// This repository as a handle that holds `section`.
    pub(crate) fn within_section(&self, section: SectionHandle) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
            section: Some(section),
        }
    }

    /// Runs `prepare` on a stage of the live state, then publishes the
    /// changes of the stage as one whole set, or drops them on a dry run or
    /// an error.
    ///
    /// The body borrows a store that keeps this repository's identity and
    /// reads and writes the overlay of the stage. The live section holds the
    /// section lock; each read and write of the staged store takes the lock
    /// that the stage owns, so workers of the body run their staged sections
    /// one at a time. A panic in a staged section stops the publication.
    /// While the body runs, live access to this repository on the same
    /// thread refuses with
    /// [`LiveAccessDuringStage`](crate::publication::LiveAccessDuringStage).
    /// Records written in the body carry `stamp`, or the working tree's
    /// commit when it is `None`.
    #[rule("rule_staged_write_keeps_live_repository_identity")]
    pub fn stage<R>(
        &self,
        dry_run: bool,
        stamp: Option<Stamp>,
        prepare: impl FnOnce(&StateStore) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let record_stamp = stamp.map_or(
            crate::state_store::RecordStamp::ResolveFromRepository,
            crate::state_store::RecordStamp::Fixed,
        );
        self.store().with_repository_publication(|held| {
            crate::publication::with_staged_state(held, dry_run, record_stamp, prepare)
        })
    }
}

/// The manifest scope with the selected id. A scope that the manifest does
/// not declare is refused with the declared scopes, so a mistyped scope never
/// reads as an empty one.
#[rule("rule_graph_report_refuses_undeclared_scope")]
pub fn declared_scope<'a>(
    manifest: &'a Manifest,
    selected: &str,
) -> Result<&'a Scope, OpenFailure> {
    manifest
        .scopes
        .iter()
        .find(|scope| scope.id.as_str() == selected)
        .ok_or_else(|| OpenFailure::UnknownScope {
            selected: selected.to_owned(),
            declared: manifest
                .scopes
                .iter()
                .map(|scope| scope.id.as_str().to_owned())
                .collect(),
        })
}

/// The resolved root. On Windows the root has no verbatim `\\?\` prefix
/// where the plain form names the same directory, because the root goes
/// into paths that a user reads. The root keeps the prefix where the plain
/// form names a different path: a root of 260 characters or more, a
/// reserved name such as `CON`, or a name that ends with a dot or a space.
#[rule("rule_opened_root_is_resolved_working_tree")]
#[cfg(feature = "host")]
fn canonical_root(path: &Path) -> Result<Utf8PathBuf, OpenFailure> {
    let invalid = |error: std::io::Error| OpenFailure::InvalidRoot {
        path: path.display().to_string(),
        error,
    };
    let canonical = dunce::canonicalize(path).map_err(invalid)?;
    if !canonical.is_dir() {
        return Err(invalid(std::io::Error::other("is not a directory")));
    }
    Utf8PathBuf::from_path_buf(canonical)
        .map_err(|_| invalid(std::io::Error::other("is not UTF-8")))
}

#[cfg(feature = "host")]
fn gate_failure(error: anyhow::Error) -> OpenFailure {
    if error.downcast_ref::<GraphNotInitialized>().is_some() {
        OpenFailure::NotInitialized
    } else {
        OpenFailure::Io(error)
    }
}

#[cfg(all(test, feature = "host"))]
mod stage_recovery_tests;
