use crate::output;
use crate::store::Store;
use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::{ensure_supported_schema_version, Manifest};
use provenance_macros::rule;
use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryRun, CheckInput, CheckPort, Finding,
    PortFuture, Refusal, Status,
};
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
        provenance_cli::porcelain::check_input_from_selectors(
            self.graph,
            self.statements,
            self.bindings,
        )
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

    fn compute(&self, category: Category, scope: Option<&str>) -> Result<CategoryRun, String> {
        match category {
            Category::Graph => self.graph_run(scope),
            Category::Statements => self.statement_run(scope),
            Category::Bindings => self.binding_run(scope),
        }
    }

    fn graph_run(&self, scope: Option<&str>) -> Result<CategoryRun, String> {
        let store = Store::open(&self.repo);
        let result =
            provenance_store::layout::with_initialized_graph(store.layout(), |mut manifest| {
                if let Some(scope) = scope {
                    manifest
                        .scopes
                        .retain(|candidate| candidate.id.as_str() == scope);
                    anyhow::ensure!(!manifest.scopes.is_empty(), "scope {scope} does not exist");
                }
                validate_locked(&store, &manifest, scope.is_none())
            });
        let findings = match result {
            Ok(()) => Vec::new(),
            Err(error)
                if error.downcast_ref::<std::io::Error>().is_some()
                    || error
                        .downcast_ref::<provenance_store::layout::GraphNotInitialized>()
                        .is_some() =>
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
        let store = Store::open(&self.repo);
        let (diagnostics, context) =
            provenance_store::layout::with_initialized_graph(store.layout(), |mut manifest| {
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
                    Ok((analysis.diagnostics, Some(analysis.context)))
                } else {
                    statement_report::changed_statements_from_head(&store, &self.repo, &manifest)
                        .map(|diagnostics| (diagnostics, None))
                }
            })
            .map_err(|error| format!("{error:#}"))?;
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
        let store = Store::open(&self.repo);
        provenance_store::layout::require_initialized_graph(store.layout())
            .map_err(|error| format!("{error:#}"))?;
        let scanned = provenance_scanner::scan_path_with_content(&self.repo)
            .map_err(|error| format!("{error:#}"))?;
        self.binding_run_from_scanned(selected_scope, &scanned)
    }

    fn binding_run_from_scanned(
        &self,
        selected_scope: Option<&str>,
        scanned: &[provenance_scanner::FileScanWithContent],
    ) -> Result<CategoryRun, String> {
        let store = Store::open(&self.repo);
        let policy = provenance_store::settings::Settings::load(store.layout())
            .map_err(|error| format!("{error:#}"))?
            .coverage
            .binding_findings;
        let warnings = provenance_store::layout::with_initialized_graph(store.layout(), |manifest| {
            let scopes = manifest
                .scopes
                .into_iter()
                .filter(|scope| selected_scope.is_none_or(|selected| scope.id.as_str() == selected))
                .collect::<Vec<_>>();
            if let Some(scope) = selected_scope {
                anyhow::ensure!(!scopes.is_empty(), "scope {scope} does not exist");
            }
            let mut warnings = Vec::new();
            for scope in scopes {
                let report = super::coverage::coverage_scan_from_scanned(
                    &self.repo,
                    &self.repo,
                    scope.id.as_str(),
                    scanned,
                )?;
                warnings.extend(report.report.warnings);
            }
            Ok(warnings)
        })
        .map_err(|error| format!("{error:#}"))?;
        let refusal = if policy == provenance_store::settings::BindingFindingsSeverity::Error
            && warnings.iter().any(|warning| warning.binding_finding)
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
    repo: Utf8PathBuf,
    strict: bool,
    base: Option<String>,
    json: bool,
    selectors: Selectors,
) -> anyhow::Result<()> {
    let input = selectors.input();
    let service =
        provenance_porcelain::Porcelain::new(RepositoryCheckPort::new(repo, strict, base));
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

/// Validates graph records without treating missing code bindings as graph errors.
#[rule("rule_porcelain_missing_binding_not_invalid")]
fn validate_locked(
    store: &Store,
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
