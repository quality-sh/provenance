use crate::output;
use crate::repo_context::{
    declared_scope_in, initialization_hint, is_not_initialized, RepoContext,
};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, Manifest};
use provenance_macros::rule;
use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryRun, CheckInput, CheckPort, Finding,
    PortFuture, Refusal, Status,
};
use provenance_store::dictionary_reference::{resolve_project_dictionary, DictionaryResolution};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::settings::BindingFindingsSeverity;
use provenance_store::state_store::StateStore;
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
        provenance_cli::porcelain::check_input_from_selectors(
            self.graph,
            self.statements,
            self.bindings,
        )
    }
}

#[derive(Clone)]
pub struct RepositoryCheckPort {
    context: RepoContext,
    strict: bool,
    base: Option<String>,
}

impl RepositoryCheckPort {
    pub fn new(repo: Utf8PathBuf, strict: bool, base: Option<String>) -> Self {
        Self::with_context(RepoContext::new(repo, "default"), strict, base)
    }

    pub const fn with_context(context: RepoContext, strict: bool, base: Option<String>) -> Self {
        Self {
            context,
            strict,
            base,
        }
    }

    fn compute(&self, category: Category, scope: Option<&str>) -> Result<CategoryRun, String> {
        match category {
            Category::Graph => self.graph_run(scope),
            Category::Statements => self.statement_run(scope),
            Category::Bindings => self.binding_run(scope),
        }
    }

    /// The store for a run over the manifest's scopes. A repository-wide run
    /// has no scope to open a repository with, so it reads through the layout.
    fn repository_wide_store(&self) -> (ProvenanceLayout, StateStore) {
        let layout = ProvenanceLayout::new(&self.context.repo);
        let store = StateStore::new(layout.clone());
        (layout, store)
    }

    fn graph_run(&self, scope: Option<&str>) -> Result<CategoryRun, String> {
        let (layout, store) = self.repository_wide_store();
        let result = provenance_store::layout::with_initialized_graph(&layout, |mut manifest| {
            if let Some(scope) = scope {
                declared_scope_in(&manifest, scope)?;
                manifest
                    .scopes
                    .retain(|candidate| candidate.id.as_str() == scope);
            }
            validate_locked(&store, &manifest, scope.is_none())
        })
        .map_err(|error| initialization_hint(&self.context.repo, error));
        let findings = match result {
            Ok(()) => Vec::new(),
            Err(error)
                if error.downcast_ref::<std::io::Error>().is_some()
                    || is_not_initialized(&error) =>
            {
                return Err(format!("{error:#}"));
            }
            Err(error) => vec![Finding::new(format!("{error:#}"))],
        };
        let refusal = if findings.is_empty() {
            Refusal::None
        } else {
            Refusal::Findings
        };
        Ok(CategoryRun::Graph { findings, refusal })
    }

    fn statement_run(&self, scope: Option<&str>) -> Result<CategoryRun, String> {
        let (layout, store) = self.repository_wide_store();
        let (diagnostics, context) =
            provenance_store::layout::with_initialized_graph(&layout, |mut manifest| {
                if let Some(scope) = scope {
                    declared_scope_in(&manifest, scope)?;
                    manifest
                        .scopes
                        .retain(|candidate| candidate.id.as_str() == scope);
                }
                if self.strict {
                    ensure_strict_dictionary_index(&layout)?;
                    let analysis = statement_report::changed_statements_from_commits(
                        &self.context.repo,
                        &manifest,
                        self.base.as_deref(),
                    )?;
                    Ok((analysis.diagnostics, Some(analysis.context)))
                } else {
                    statement_report::changed_statements_from_head(
                        &store,
                        &self.context.repo,
                        &manifest,
                    )
                    .map(|diagnostics| (diagnostics, None))
                }
            })
            .map_err(|error| format!("{:#}", initialization_hint(&self.context.repo, error)))?;
        let findings = diagnostics
            .into_iter()
            .map(|diagnostic| {
                Finding::with_detail(
                    &diagnostic.message,
                    serde_json::to_value(&diagnostic).expect("statement diagnostic is JSON"),
                )
            })
            .collect::<Vec<_>>();
        let refusal = if self.strict && !findings.is_empty() {
            Refusal::Findings
        } else {
            Refusal::None
        };
        Ok(CategoryRun::Statements {
            findings,
            context,
            refusal,
        })
    }

    /// Finds missing code bindings without running project tests.
    #[rule("rule_porcelain_coverage_does_not_run_tests")]
    fn binding_run(&self, selected_scope: Option<&str>) -> Result<CategoryRun, String> {
        provenance_store::layout::require_initialized_graph(&ProvenanceLayout::new(
            &self.context.repo,
        ))
        .map_err(|error| format!("{:#}", initialization_hint(&self.context.repo, error)))?;
        let scanned = provenance_scanner::scan_path_with_content(&self.context.repo)
            .map_err(|error| format!("{error:#}"))?;
        self.binding_run_from_scanned(selected_scope, &scanned)
    }

    /// Scans each selected scope through the repository opened for it.
    fn binding_run_from_scanned(
        &self,
        selected_scope: Option<&str>,
        scanned: &[provenance_scanner::FileScanWithContent],
    ) -> Result<CategoryRun, String> {
        let (warnings, governed_finding_count, policy) = self
            .scan_selected_scopes(selected_scope, scanned)
            .map_err(|error| format!("{error:#}"))?;
        let severity = match policy {
            provenance_store::settings::BindingFindingsSeverity::Warning => {
                provenance_scanner::BindingFindingSeverity::Warning
            }
            provenance_store::settings::BindingFindingsSeverity::Error => {
                provenance_scanner::BindingFindingSeverity::Error
            }
        };
        let refusal = if provenance_scanner::binding_findings_fail(severity, governed_finding_count)
        {
            Refusal::Findings
        } else {
            Refusal::None
        };
        let findings = warnings
            .into_iter()
            .map(|warning| {
                Finding::with_detail(
                    &warning.message,
                    serde_json::to_value(&warning).expect("coverage warning is JSON"),
                )
            })
            .collect();
        let policy = match policy {
            provenance_store::settings::BindingFindingsSeverity::Warning => BindingPolicy::Warning,
            provenance_store::settings::BindingFindingsSeverity::Error => BindingPolicy::Error,
        };
        Ok(CategoryRun::Bindings {
            findings,
            context: BindingContext { policy },
            refusal,
        })
    }
}

impl RepositoryCheckPort {
    fn scan_selected_scopes(
        &self,
        selected_scope: Option<&str>,
        scanned: &[provenance_scanner::FileScanWithContent],
    ) -> anyhow::Result<(
        Vec<provenance_core::coverage::ValidationWarning>,
        usize,
        BindingFindingsSeverity,
    )> {
        let manifest = provenance_store::layout::with_initialized_graph(
            &ProvenanceLayout::new(&self.context.repo),
            Ok,
        )?;
        let scopes = match selected_scope {
            Some(selected) => vec![declared_scope_in(&manifest, selected)?],
            None => manifest.scopes.iter().collect(),
        };
        let mut warnings = Vec::new();
        let mut governed_finding_count = 0;
        let mut policy = BindingFindingsSeverity::default();
        for scope in scopes {
            let repository =
                RepoContext::new(self.context.repo.clone(), scope.id.as_str()).open()?;
            policy = repository.settings().coverage.binding_findings;
            let outcome = super::coverage::coverage_scan_from_scanned(
                &repository,
                &self.context.repo,
                &self.context.repo,
                scope,
                &manifest,
                scanned,
            )?;
            governed_finding_count += outcome.governed_finding_count;
            warnings.extend(outcome.scan.report.warnings);
        }
        Ok((warnings, governed_finding_count, policy))
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
}

#[rule("rule_ste_strict_committed_statement_gate")]
pub(super) async fn check(
    context: RepoContext,
    strict: bool,
    base: Option<String>,
    json: bool,
    selectors: Selectors,
) -> anyhow::Result<()> {
    let input = selectors.input();
    let service = provenance_porcelain::Porcelain::new(RepositoryCheckPort::with_context(
        context, strict, base,
    ));
    let report = service.check(input).await;
    if json {
        output::print_json(&report)?;
    } else {
        println!("{}", provenance_cli::porcelain::render_check(&report));
    }
    if report
        .categories
        .iter()
        .any(provenance_porcelain::check::CategoryReport::refuses)
    {
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

/// Validates every scope of a staged repository.
pub(super) fn validate_repository(store: &StateStore) -> anyhow::Result<()> {
    store.with_repository_publication(|| {
        let manifest = store.manifest()?;
        validate_locked(store, &manifest, true)
    })
}

/// Validates the state on disk against the manifest `init` plans to write.
pub(super) fn validate_planned_manifest(
    repo: &Utf8Path,
    manifest: &Manifest,
) -> anyhow::Result<()> {
    provenance_store::repository_init::validate_planned_manifest(
        &ProvenanceLayout::new(repo),
        manifest,
        |store, manifest| validate_locked(store, manifest, true),
    )
}

/// Finishes an interrupted publication before `init` reads the repository.
pub(super) fn recover_before_init(repo: &Utf8Path) -> anyhow::Result<()> {
    provenance_store::repository_init::recover_before_init(&ProvenanceLayout::new(repo))
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

/// Validates graph records without treating missing code bindings as graph errors.
#[rule("rule_porcelain_missing_binding_not_invalid")]
fn validate_locked(
    store: &StateStore,
    manifest: &Manifest,
    repository_wide: bool,
) -> anyhow::Result<()> {
    ensure_supported_schema_version("manifest", manifest.schema_version)?;
    manifest.ensure_has_scopes()?;
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
mod tests;
