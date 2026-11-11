//! Repository initialization before a repository can be opened.
//!
//! This module plans the manifest, recovers an interrupted publication,
//! validates the planned manifest against the state already on disk, and
//! gives init the one hold of the publication lock in which it checks and
//! writes.

use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use crate::storage::{Atomicity, StateKey, StateView, WriteSet};
use camino::Utf8PathBuf;
use provenance_core::{Manifest, RepoPathPrefix, Scope, ScopeId};
use std::sync::Arc;

/// Plans the scope and reviewer settings before initialization writes files.
pub fn plan_manifest(
    existing: Option<Manifest>,
    scope: Option<&str>,
    path_prefix: Option<Utf8PathBuf>,
    actor_ids: Vec<String>,
    clear_actors: bool,
    selected_actor: Option<String>,
) -> anyhow::Result<Manifest> {
    if actor_ids.iter().any(|id| id.trim().is_empty()) {
        return Err(crate::refusal::refuse!(
            (crate::write_error::WriteFailure::invalid_field("actor_ids")),
            "disposition actor IDs must not be empty"
        ));
    }
    let is_new = existing.is_none();
    let mut manifest = match existing {
        Some(manifest) => manifest,
        None => Manifest::default_with_scope(
            ScopeId::new(scope.unwrap_or("default"))?,
            RepoPathPrefix::new(
                path_prefix
                    .clone()
                    .unwrap_or_else(|| Utf8PathBuf::from(".")),
            ),
        ),
    };
    if !is_new {
        if let Some(scope) = scope {
            let id = ScopeId::new(scope)?;
            if let Some(existing) = manifest.scopes.iter_mut().find(|item| item.id == id) {
                if let Some(prefix) = path_prefix {
                    existing.path_prefix = RepoPathPrefix::new(prefix);
                }
            } else {
                manifest.scopes.push(Scope {
                    id,
                    path_prefix: RepoPathPrefix::new(
                        path_prefix.unwrap_or_else(|| Utf8PathBuf::from(".")),
                    ),
                });
            }
        }
    }
    if clear_actors {
        manifest.disposition_actor_ids.clear();
    } else if !actor_ids.is_empty() {
        manifest.disposition_actor_ids = actor_ids;
    } else if is_new {
        manifest.disposition_actor_ids.extend(selected_actor);
    }
    Ok(manifest)
}

/// Finishes an interrupted publication before initialization reads the
/// repository. A directory with no `.provenance` has nothing to recover.
pub fn recover_before_init(layout: &ProvenanceLayout) -> anyhow::Result<()> {
    match std::fs::symlink_metadata(layout.provenance_dir()) {
        Ok(_) => crate::publication::with_repository_publication(layout, |_| Ok(())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Runs `validate` over the state on disk and the planned manifest.
///
/// An existing `.provenance` is validated under the publication lock, after
/// recovery; a new repository is validated read-only.
pub fn validate_planned_manifest<R>(
    layout: &ProvenanceLayout,
    manifest: &Manifest,
    validate: impl FnOnce(&StateStore, &Manifest) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    match std::fs::symlink_metadata(layout.provenance_dir()) {
        Ok(_) => StateStore::new(layout.clone())
            .with_repository_publication(|held| validate(held, manifest)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::publication::with_read_only_validation(layout, |store| validate(store, manifest))
        }
        Err(error) => Err(error.into()),
    }
}

/// The key of the manifest in a view of the canonical state.
const MANIFEST_KEY: &str = "manifest.json";

/// One hold of the publication lock of an initialized repository.
///
/// Init reads the manifest, validates its plan against the graph and writes
/// in it, so no other write of the graph lands between the check and the
/// write.
pub struct InitSection<'a> {
    store: &'a StateStore,
    base: Arc<StateView>,
}

/// Runs `body` in one hold of the publication lock of `layout`.
///
/// The hold finishes an interrupted publication and reads the canonical
/// state before `body` starts. The repository must have a `.provenance`
/// directory.
pub fn with_init_section<R>(
    layout: &ProvenanceLayout,
    body: impl FnOnce(&InitSection<'_>) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    crate::publication::with_repository_publication(layout, |store| {
        let base = Arc::new(store.storage().read_view()?);
        body(&InitSection { store, base })
    })
}

impl InitSection<'_> {
    /// The bytes of the manifest when the hold started, or `None` when the
    /// repository has no manifest.
    pub fn manifest_bytes(&self) -> anyhow::Result<Option<&[u8]>> {
        Ok(self.base.get(&StateKey::new(MANIFEST_KEY)?))
    }

    /// The store that holds the section, for the validation of the planned
    /// manifest.
    pub const fn store(&self) -> &StateStore {
        self.store
    }

    /// Publishes `bytes` as the manifest in a one-key per-key write set.
    ///
    /// The write is one rename over `manifest.json`, with no backup step, so
    /// a stop leaves the old manifest or the new manifest. The store refuses
    /// the set when the canonical state is not the state that the hold read
    /// at its start.
    pub fn publish_manifest(&self, bytes: Vec<u8>) -> anyhow::Result<()> {
        let writes = WriteSet::new(Atomicity::PerKey)
            .put(StateKey::new(MANIFEST_KEY)?, bytes)
            .on_base(Arc::clone(&self.base));
        self.store.publish_through(writes, |writes| self.store.storage().publish(writes))
    }
}
