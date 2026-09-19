use crate::output;
use crate::store::Store;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, Manifest};
use provenance_macros::rule;
use provenance_porcelain::check::{Category, CheckInput, CheckPort, Finding, PortFuture, Status};
use provenance_store::dictionary_reference::{resolve_project_dictionary, DictionaryResolution};
use std::collections::BTreeSet;

mod index;
mod references;
mod scope;
mod statement_report;

use index::CheckIndex;

#[derive(Clone, Copy)]
pub(super) struct Selectors {
    pub graph: bool,
    pub statements: bool,
    pub bindings: bool,
}

impl Selectors {
    fn input(self) -> CheckInput {
        let mut categories = Vec::new();
        if self.graph {
            categories.push(Category::Graph);
        }
        if self.statements {
            categories.push(Category::Statements);
        }
        if self.bindings {
            categories.push(Category::Bindings);
        }
        CheckInput::new(categories)
    }
}

#[derive(Clone)]
pub struct RepositoryCheckPort {
    repo: Utf8PathBuf,
    strict: bool,
    base: Option<String>,
}

impl RepositoryCheckPort {
    pub const fn new(repo: Utf8PathBuf, strict: bool, base: Option<String>) -> Self {
        Self { repo, strict, base }
    }

    fn compute(&self, category: Category, scope: Option<&str>) -> Result<Vec<Finding>, String> {
        match category {
            Category::Graph => self.graph_findings(scope),
            Category::Statements => self.statement_findings(scope),
            Category::Bindings => self.binding_findings(scope),
        }
    }

    fn graph_findings(&self, scope: Option<&str>) -> Result<Vec<Finding>, String> {
        let store = Store::open(&self.repo);
        match store.with_repository_publication(|| {
            let mut manifest = store.manifest()?;
            if let Some(scope) = scope {
                manifest
                    .scopes
                    .retain(|candidate| candidate.id.as_str() == scope);
                anyhow::ensure!(!manifest.scopes.is_empty(), "scope {scope} does not exist");
            }
            validate_locked(&store, &manifest, scope.is_none())
        }) {
            Ok(()) => Ok(Vec::new()),
            Err(error) if error.downcast_ref::<std::io::Error>().is_some() => {
                Err(format!("{error:#}"))
            }
            Err(error) => Ok(vec![Finding::new(format!("{error:#}"))]),
        }
    }

    fn statement_findings(&self, scope: Option<&str>) -> Result<Vec<Finding>, String> {
        let store = Store::open(&self.repo);
        store
            .with_repository_publication(|| {
                let mut manifest = store.manifest()?;
                if let Some(scope) = scope {
                    manifest
                        .scopes
                        .retain(|candidate| candidate.id.as_str() == scope);
                    anyhow::ensure!(!manifest.scopes.is_empty(), "scope {scope} does not exist");
                }
                if self.strict {
                    ensure_strict_dictionary_index(store.layout())?;
                    let analysis = statement_report::changed_statements_from_commits(
                        &self.repo,
                        &manifest,
                        self.base.as_deref(),
                    )?;
                    Ok(analysis.diagnostics)
                } else {
                    statement_report::changed_statements_from_head(&store, &self.repo, &manifest)
                }
            })
            .map(|diagnostics| {
                diagnostics
                    .into_iter()
                    .map(|diagnostic| {
                        Finding::with_detail(
                            &diagnostic.message,
                            serde_json::to_value(&diagnostic)
                                .expect("statement diagnostic is JSON"),
                        )
                    })
                    .collect()
            })
            .map_err(|error| format!("{error:#}"))
    }

    fn binding_findings(&self, selected_scope: Option<&str>) -> Result<Vec<Finding>, String> {
        let store = Store::open(&self.repo);
        let manifest = store.manifest().map_err(|error| format!("{error:#}"))?;
        let mut findings = Vec::new();
        for scope in manifest
            .scopes
            .into_iter()
            .filter(|scope| selected_scope.is_none_or(|selected| scope.id.as_str() == selected))
        {
            let report =
                super::coverage::coverage_scan(&self.repo, &self.repo, scope.id.as_str(), true)
                    .map_err(|error| format!("{error:#}"))?;
            findings.extend(report.warnings.iter().map(|warning| {
                Finding::with_detail(
                    &warning.message,
                    serde_json::to_value(warning).expect("coverage warning is JSON"),
                )
            }));
        }
        Ok(findings)
    }
}

impl CheckPort for RepositoryCheckPort {
    fn run<'a>(&'a self, category: Category, scope: Option<&'a str>) -> PortFuture<'a> {
        let port = self.clone();
        let scope = scope.map(str::to_owned);
        Box::pin(async move {
            tokio::task::spawn_blocking(move || port.compute(category, scope.as_deref()))
                .await
                .map_err(|error| format!("check worker failed: {error}"))?
        })
    }

    fn context(&self, category: Category) -> Option<serde_json::Value> {
        (self.strict && category == Category::Statements)
            .then(|| {
                statement_report::committed_statement_context(&self.repo, self.base.as_deref())
                    .ok()
                    .and_then(|context| serde_json::to_value(context).ok())
            })
            .flatten()
    }
}

#[rule("rule_ste_strict_committed_statement_gate")]
pub(super) async fn check(
    repo: Utf8PathBuf,
    strict: bool,
    base: Option<String>,
    json: bool,
    selectors: Selectors,
) -> anyhow::Result<()> {
    let input = selectors.input();
    let binding_error_policy = if input.categories().contains(&Category::Bindings) {
        matches!(
            provenance_store::settings::Settings::load(
                &provenance_store::layout::ProvenanceLayout::new(&repo),
            )?
            .coverage
            .binding_findings,
            provenance_store::settings::BindingFindingsSeverity::Error
        )
    } else {
        false
    };
    let service =
        provenance_porcelain::Porcelain::new(RepositoryCheckPort::new(repo, strict, base));
    let report = service.check(input).await;
    if json {
        output::print_json(&report)?;
    } else {
        println!("{}", provenance_cli::porcelain::render_check(&report));
    }
    let graph_failed = report
        .categories
        .iter()
        .any(|category| category.category == Category::Graph && category.status != Status::Passed);
    let unavailable = report
        .categories
        .iter()
        .any(|category| category.status == Status::Unavailable);
    let strict_findings = strict
        && report.categories.iter().any(|category| {
            category.category == Category::Statements && category.status == Status::Findings
        });
    let binding_refused = binding_error_policy
        && report.categories.iter().any(|category| {
            category.category == Category::Bindings
                && category.findings.iter().any(|finding| {
                    finding
                        .detail
                        .as_ref()
                        .and_then(|detail| detail.get("binding_finding"))
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
                })
        });
    if graph_failed || unavailable || strict_findings || binding_refused {
        let details = report
            .categories
            .iter()
            .filter(|category| category.status != Status::Passed)
            .flat_map(|category| {
                category
                    .findings
                    .iter()
                    .map(|finding| finding.message.as_str())
                    .chain(category.unavailable_reason.as_deref())
            })
            .collect::<Vec<_>>()
            .join("; ");
        anyhow::bail!(details);
    }
    Ok(())
}

pub(super) fn validate_repository(repo: Utf8PathBuf) -> anyhow::Result<()> {
    let store = Store::open(repo);
    store.with_repository_publication(|| {
        let manifest = store.manifest()?;
        validate_locked(&store, &manifest, true)
    })
}

pub(super) fn validate_repository_with_manifest(
    repo: &Utf8Path,
    manifest: &Manifest,
) -> anyhow::Result<()> {
    let store = Store::open(repo);
    match std::fs::symlink_metadata(store.layout().provenance_dir()) {
        Ok(_) => store.with_repository_publication(|| validate_locked(&store, manifest, true)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            provenance_store::publication::with_read_only_validation(store.layout(), || {
                validate_locked(&store, manifest, true)
            })
        }
        Err(error) => Err(error.into()),
    }
}

pub(super) fn recover_repository_before_init(repo: &Utf8Path) -> anyhow::Result<()> {
    let store = Store::open(repo);
    match std::fs::symlink_metadata(store.layout().provenance_dir()) {
        Ok(_) => store.with_repository_publication(|| Ok(())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

/// Fails a strict check when the committed dictionary reference has no
/// loadable index, instead of silently downgrading to rules-only checking.
#[rule("rule_ste_strict_dictionary_index_gate")]
fn ensure_strict_dictionary_index(
    layout: &provenance_store::layout::ProvenanceLayout,
) -> anyhow::Result<()> {
    match resolve_project_dictionary(layout) {
        DictionaryResolution::NoReference | DictionaryResolution::Loaded(_) => Ok(()),
        DictionaryResolution::Unavailable {
            directory, reason, ..
        } => {
            let directory = directory.map_or_else(
                || "the machine data directory".to_owned(),
                |directory| directory.display().to_string(),
            );
            anyhow::bail!(
                "the committed dictionary reference has no loadable index in {directory}: \
                 {reason}. Run `provenance dictionary import` with the local Issue 9 PDF \
                 on this machine, or set PROVENANCE_STE100_INDEX_DIR to the directory \
                 that holds the index"
            );
        }
    }
}

fn validate_locked(
    store: &Store,
    manifest: &Manifest,
    repository_wide: bool,
) -> anyhow::Result<()> {
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

    store.validate_canonical_ids_unique(&manifest.scopes)?;

    let scope_directory_findings = if repository_wide {
        store
            .list_scope_directories()?
            .into_iter()
            .filter(|directory| !manifest_scopes.contains(directory))
            .map(|directory| format!("scope directory {directory} is absent from manifest"))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

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
    use provenance_store::layout::ProvenanceLayout;

    #[test]
    #[provenance_macros::verifies("rule_init_validates_planned_repository", examples)]
    fn planned_manifest_validation_runs_publication_recovery_before_reading_state() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.scopes_dir()).unwrap();
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

    #[tokio::test]
    async fn scoped_check_does_not_read_another_scope() {
        let directory = tempfile::tempdir().unwrap();
        let repo = Utf8PathBuf::from_path_buf(directory.path().to_path_buf()).unwrap();
        let layout = ProvenanceLayout::new(repo.clone());
        std::fs::create_dir_all(layout.state_dir()).unwrap();
        let mut manifest = Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        );
        manifest.scopes.push(provenance_core::Scope {
            id: ScopeId::new("other").unwrap(),
            path_prefix: RepoPathPrefix::new("other"),
        });
        std::fs::write(
            layout.manifest_path(),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let foreign = layout
            .scopes_dir()
            .join("other")
            .join("requirements")
            .join("req.jsonl");
        std::fs::create_dir_all(foreign.parent().unwrap()).unwrap();
        std::fs::write(foreign, "not JSON\n").unwrap();

        let port = RepositoryCheckPort::new(repo, false, None);
        let findings = port.run(Category::Graph, Some("default")).await.unwrap();

        assert!(findings.is_empty());
    }
}
