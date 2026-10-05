//! Manifest planning shared by repository initialization and its test support.

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
