use crate::output::{self, OutputFormat};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, Manifest};
use provenance_macros::rule;
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
use std::collections::BTreeSet;

mod edges;
mod index;
mod references;
mod scope;
mod statement_report;

use index::CheckIndex;

#[rule("rule_ste_strict_committed_statement_gate")]
pub(super) fn check(
    repo: &Utf8Path,
    strict: bool,
    base: Option<&str>,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let store = StateStore::new(ProvenanceLayout::new(repo.to_path_buf()));
    let report = store.with_repository_publication(|| {
        let manifest = store.manifest()?;
        collect_report_locked(&store, repo, &manifest, strict, base)
    })?;
    let has_findings = !report.diagnostics.is_empty();
    output::print(format, &report)?;
    anyhow::ensure!(
        !strict || !has_findings,
        "strict statement check found ASD-STE100 findings"
    );
    Ok(())
}

#[derive(serde::Serialize)]
struct CheckReport {
    status: &'static str,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    commits: Option<CommitRange>,
    diagnostics: Vec<provenance_store::statement_analysis::StatementDiagnostic>,
}

#[derive(serde::Serialize)]
struct CommitRange {
    candidate_commit: String,
    base_commit: Option<String>,
}

pub(super) fn validate_repository(repo: Utf8PathBuf) -> anyhow::Result<()> {
    let store = StateStore::new(ProvenanceLayout::new(repo));
    store.with_repository_publication(|| {
        let manifest = store.manifest()?;
        validate_locked(&store, &manifest)
    })
}

pub(super) fn validate_repository_with_manifest(
    repo: &Utf8Path,
    manifest: &Manifest,
) -> anyhow::Result<()> {
    let layout = ProvenanceLayout::new(repo.to_path_buf());
    let store = StateStore::new(layout.clone());
    match std::fs::symlink_metadata(layout.provenance_dir()) {
        Ok(_) => store.with_repository_publication(|| validate_locked(&store, manifest)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            provenance_store::publication::with_read_only_validation(&layout, || {
                validate_locked(&store, manifest)
            })
        }
        Err(error) => Err(error.into()),
    }
}

pub(super) fn recover_repository_before_init(repo: &Utf8Path) -> anyhow::Result<()> {
    let layout = ProvenanceLayout::new(repo.to_path_buf());
    match std::fs::symlink_metadata(layout.provenance_dir()) {
        Ok(_) => StateStore::new(layout).with_repository_publication(|| Ok(())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn collect_report_locked(
    store: &StateStore,
    repo: &Utf8Path,
    manifest: &Manifest,
    strict: bool,
    base: Option<&str>,
) -> anyhow::Result<CheckReport> {
    validate_locked(store, manifest)?;
    if strict {
        let analysis = statement_report::changed_statements_from_commits(store, repo, base)?;
        let status = if analysis.diagnostics.is_empty() {
            "ok"
        } else {
            "findings"
        };
        return Ok(CheckReport {
            status,
            commits: Some(CommitRange {
                candidate_commit: analysis.candidate_commit,
                base_commit: analysis.base_commit,
            }),
            diagnostics: analysis.diagnostics,
        });
    }
    Ok(CheckReport {
        status: "ok",
        commits: None,
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
    #[provenance_macros::verifies("rule_init_validates_planned_repository", examples)]
    fn planned_manifest_validation_runs_publication_recovery_before_reading_state() {
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

        let error = validate_repository_with_manifest(&repo, &manifest).unwrap_err();

        assert!(format!("{error:#}").contains("expected ident"));
        assert!(layout.publication_lock_path().exists());
        assert!(layout.import_transactions_dir().exists());
    }

    #[cfg(unix)]
    #[test]
    #[provenance_macros::verifies("rule_init_validates_planned_repository", examples)]
    fn planned_manifest_validation_refuses_a_symlinked_publication_cache() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
        let outside = directory.path().join("outside");
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.provenance_dir()).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        symlink(&outside, layout.cache_dir()).unwrap();
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );

        let error = validate_repository_with_manifest(&repo, &manifest).unwrap_err();

        assert!(format!("{error:#}").contains("symlink component"));
    }

    #[test]
    fn planned_manifest_validation_locks_an_existing_state_tree() {
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

        assert!(layout.publication_lock_path().exists());
    }

    #[test]
    fn planned_manifest_validation_keeps_a_new_repository_read_only() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        let manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );

        validate_repository_with_manifest(&repo, &manifest).unwrap();

        assert!(!layout.provenance_dir().exists());
    }
}
