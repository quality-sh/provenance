//! Repository initialization before a repository can be opened: manifest
//! planning, recovery of an interrupted publication, and validation of the
//! planned manifest against the state already on disk.

use crate::layout::ProvenanceLayout;
use crate::state_store::StateStore;
use camino::Utf8PathBuf;
use provenance_core::{Manifest, RepoPathPrefix, Scope, ScopeId};

/// Plans the scope and reviewer settings before initialization writes files.
pub fn plan_manifest(
    existing: Option<Manifest>,
    scope: Option<&str>,
    path_prefix: Option<Utf8PathBuf>,
    actor_ids: Vec<String>,
    clear_actors: bool,
    selected_actor: Option<String>,
) -> anyhow::Result<Manifest> {
    anyhow::ensure!(
        actor_ids.iter().all(|id| !id.trim().is_empty()),
        "disposition actor IDs must not be empty"
    );
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
        Ok(_) => crate::publication::with_repository_publication(layout, || Ok(())),
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
    let store = StateStore::new(layout.clone());
    match std::fs::symlink_metadata(layout.provenance_dir()) {
        Ok(_) => store.with_repository_publication(|| validate(&store, manifest)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            crate::publication::with_read_only_validation(layout, || validate(&store, manifest))
        }
        Err(error) => Err(error.into()),
    }
}
