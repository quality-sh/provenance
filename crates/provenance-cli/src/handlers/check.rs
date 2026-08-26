use crate::output::{self, OutputFormat};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, Manifest};
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
use std::collections::BTreeSet;

mod edges;
mod index;
mod references;
mod scope;
mod statement_report;

use index::CheckIndex;

pub(super) fn check(repo: &Utf8Path, format: OutputFormat) -> anyhow::Result<()> {
    let store = StateStore::new(ProvenanceLayout::new(repo.to_path_buf()));
    let report = store.with_repository_publication(|| {
        let manifest = store.manifest()?;
        collect_report_locked(&store, repo, &manifest)
    })?;
    output::print(format, &report)
}

#[derive(serde::Serialize)]
struct CheckReport {
    status: &'static str,
    diagnostics: Vec<provenance_store::statement_analysis::StatementDiagnostic>,
}

pub(super) fn validate_repository(repo: Utf8PathBuf) -> anyhow::Result<()> {
    let store = StateStore::new(ProvenanceLayout::new(repo));
    store.with_repository_publication(|| {
        let manifest = store.manifest()?;
        validate_locked(&store, &manifest)
    })
}

#[allow(dead_code)] // Called by repository planning once that integration lands.
pub(super) fn validate_repository_with_manifest(
    repo: &Utf8Path,
    manifest: &Manifest,
) -> anyhow::Result<()> {
    let layout = ProvenanceLayout::new(repo.to_path_buf());
    let store = StateStore::new(layout.clone());
    provenance_store::publication::with_read_only_validation(&layout, || {
        validate_locked(&store, manifest)
    })
}

fn collect_report_locked(
    store: &StateStore,
    repo: &Utf8Path,
    manifest: &Manifest,
) -> anyhow::Result<CheckReport> {
    validate_locked(store, manifest)?;
    Ok(CheckReport {
        status: "ok",
        diagnostics: statement_report::changed_statements_from_head(store, repo)?,
    })
}

fn validate_locked(store: &StateStore, manifest: &Manifest) -> anyhow::Result<()> {
    ensure_supported_schema_version("manifest", manifest.schema_version)?;
    anyhow::ensure!(
        !manifest.scopes.is_empty(),
        "manifest must contain at least one scope"
    );
    let manifest_scopes: BTreeSet<_> = manifest
        .scopes
        .iter()
        .map(|scope| scope.id.as_str().to_string())
        .collect();

    let scope_directory_findings = store
        .list_scope_directories()?
        .into_iter()
        .filter(|directory| !manifest_scopes.contains(directory))
        .map(|directory| format!("scope directory {directory} is absent from manifest"))
        .collect::<Vec<_>>();

    let mut index = CheckIndex::default();
    let mut dangling = Vec::new();
    scope::validate(
        store,
        &manifest.scopes,
        &manifest.disposition_actor_ids,
        &manifest_scopes,
        &mut index,
        &mut dangling,
    )?;
    edges::validate(store, &manifest_scopes, &index, &mut dangling)?;

    anyhow::ensure!(
        scope_directory_findings.is_empty(),
        "scope directory finding(s):\n- {}",
        scope_directory_findings.join("\n- ")
    );
    anyhow::ensure!(
        dangling.is_empty(),
        "dangling reference(s):\n- {}",
        dangling.join("\n- ")
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use provenance_core::{Manifest, RepoPathPrefix, ScopeId};

    #[test]
    fn planned_manifest_validation_is_read_only_and_does_not_read_disk_manifest() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.scopes_dir()).unwrap();
        std::fs::create_dir_all(layout.edges_dir()).unwrap();
        std::fs::write(layout.manifest_path(), "not the planned manifest").unwrap();
        std::fs::create_dir_all(layout.cache_dir()).unwrap();
        std::fs::write(layout.publication_marker_path(), "not a publication marker").unwrap();
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );

        validate_repository_with_manifest(&repo, &manifest).unwrap();

        assert_eq!(
            std::fs::read_to_string(layout.publication_marker_path()).unwrap(),
            "not a publication marker"
        );
        assert!(!layout.publication_lock_path().exists());
        assert!(!layout.import_transactions_dir().exists());
    }

    #[test]
    fn planned_manifest_validation_does_not_create_a_cache_directory() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.scopes_dir()).unwrap();
        std::fs::create_dir_all(layout.edges_dir()).unwrap();
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );

        validate_repository_with_manifest(&repo, &manifest).unwrap();

        assert!(!layout.cache_dir().exists());
    }
}
