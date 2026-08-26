use anyhow::Context;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{Manifest, RepoPathPrefix, Scope, ScopeId};
use provenance_store::layout::ProvenanceLayout;

pub(super) fn init(
    path: &Utf8Path,
    scope: Option<String>,
    path_prefix: Option<Utf8PathBuf>,
    disposition_actor_ids: Vec<String>,
    clear_disposition_actors: bool,
) -> anyhow::Result<()> {
    let layout = ProvenanceLayout::new(path.to_path_buf());
    let manifest_exists = layout.manifest_path().exists();
    anyhow::ensure!(
        manifest_exists || scope.is_some(),
        "--scope is required when initializing a new repository"
    );
    std::fs::create_dir_all(layout.scopes_dir())?;
    std::fs::create_dir_all(layout.edges_dir())?;
    std::fs::create_dir_all(layout.cache_dir())?;
    anyhow::ensure!(
        disposition_actor_ids.iter().all(|id| !id.trim().is_empty()),
        "disposition actor IDs must not be empty"
    );
    let mut manifest = if manifest_exists {
        read_manifest(&layout)?
            .ok_or_else(|| anyhow::anyhow!("manifest disappeared during init"))?
    } else {
        let scope = scope.as_deref().ok_or_else(|| {
            anyhow::anyhow!("--scope is required when initializing a new repository")
        })?;
        Manifest::default_with_scope(
            ScopeId::new(scope)?,
            RepoPathPrefix::new(
                path_prefix
                    .clone()
                    .unwrap_or_else(|| Utf8PathBuf::from(".")),
            ),
        )
    };

    if manifest_exists {
        update_scope(&mut manifest, scope, path_prefix)?;
    }
    if clear_disposition_actors {
        manifest.disposition_actor_ids.clear();
    } else if !disposition_actor_ids.is_empty() {
        manifest.disposition_actor_ids = disposition_actor_ids;
    }
    std::fs::write(
        layout.manifest_path(),
        format!("{}\n", serde_json::to_string_pretty(&manifest)?),
    )?;
    crate::onboarding::install(path)?;
    crate::gitignore::ensure_ignored(path, ".provenance/cache/")
        .context("failed to ignore the Provenance cache")?;
    super::check::validate_repository(path.to_path_buf())
        .context("the initialized Provenance state is not valid")?;
    Ok(())
}

pub(super) fn scope_path_prefix(
    path: &Utf8Path,
    scope: &str,
) -> anyhow::Result<Option<Utf8PathBuf>> {
    let layout = ProvenanceLayout::new(path.to_path_buf());
    let Some(manifest) = read_manifest(&layout)? else {
        return Ok(None);
    };
    let scope_id = ScopeId::new(scope)?;
    Ok(manifest
        .scopes
        .iter()
        .find(|item| item.id == scope_id)
        .map(|item| item.path_prefix.as_path().to_path_buf()))
}

fn read_manifest(layout: &ProvenanceLayout) -> anyhow::Result<Option<Manifest>> {
    if !layout.manifest_path().exists() {
        return Ok(None);
    }
    let manifest = serde_json::from_slice::<Manifest>(&std::fs::read(layout.manifest_path())?)?;
    provenance_core::ensure_supported_schema_version("manifest", manifest.schema_version)?;
    Ok(Some(manifest))
}

fn update_scope(
    manifest: &mut Manifest,
    scope: Option<String>,
    path_prefix: Option<Utf8PathBuf>,
) -> anyhow::Result<()> {
    let Some(scope) = scope else {
        return Ok(());
    };
    let scope_id = ScopeId::new(scope)?;
    if let Some(existing) = manifest.scopes.iter_mut().find(|item| item.id == scope_id) {
        if let Some(path_prefix) = path_prefix {
            existing.path_prefix = RepoPathPrefix::new(path_prefix);
        }
    } else {
        manifest.scopes.push(Scope {
            id: scope_id,
            path_prefix: RepoPathPrefix::new(path_prefix.unwrap_or_else(|| Utf8PathBuf::from("."))),
        });
    }
    Ok(())
}
