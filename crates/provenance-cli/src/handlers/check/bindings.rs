//! The bindings check category: the Rule binding scan of each selected scope, with the
//! path, baseline, and report file options that `coverage scan` had.

use super::RepositoryCheckPort;
use crate::output::ReportFormat;
use crate::repo_context::{initialization_hint, RepoContext};
use camino::Utf8PathBuf;
use provenance_macros::rule;
use provenance_porcelain::check::{BindingContext, BindingPolicy, CategoryRun, Finding, Refusal};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::declared_scope;
use provenance_store::settings::BindingFindingsSeverity;

/// The options that only the bindings check takes. The CLI sets them; MCP has none.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// The directory or file to scan. The default is the repository root.
    pub path: Option<Utf8PathBuf>,
    /// An earlier scan report whose anchors this scan resolves.
    pub baseline: Option<Utf8PathBuf>,
    /// The file that gets the scan report of the one selected scope, and its format.
    pub output: Option<(Utf8PathBuf, ReportFormat)>,
}

impl Options {
    /// Whether the caller gave any option of the bindings check.
    pub const fn is_set(&self) -> bool {
        self.path.is_some() || self.baseline.is_some() || self.output.is_some()
    }
}

/// The findings of a scan over the selected scopes.
struct Scan {
    warnings: Vec<provenance_core::coverage::ValidationWarning>,
    governed_finding_count: usize,
    policy: BindingFindingsSeverity,
}

impl RepositoryCheckPort {
    /// Finds missing code bindings without running project tests.
    #[rule("rule_porcelain_coverage_does_not_run_tests")]
    #[rule("rule_check_bindings_scans_selected_path")]
    pub(super) fn binding_run(&self, selected_scope: Option<&str>) -> Result<CategoryRun, String> {
        provenance_store::layout::require_initialized_graph(&ProvenanceLayout::new(
            &self.context.repo,
        ))
        .map_err(|error| format!("{:#}", initialization_hint(&self.context.repo, error)))?;
        let path = self.scan_path();
        let scanned = provenance_scanner::scan_path_with_content(&path)
            .map_err(|error| format!("{error:#}"))?;
        self.binding_run_from_scanned(selected_scope, &scanned)
    }

    fn scan_path(&self) -> Utf8PathBuf {
        self.bindings
            .path
            .clone()
            .unwrap_or_else(|| self.context.repo.clone())
    }

    /// Scans each selected scope through the repository opened for it. The configured
    /// severity refuses the governed findings; `--strict` refuses every finding.
    #[rule("rule_check_strict_refuses_binding_warnings")]
    pub(super) fn binding_run_from_scanned(
        &self,
        selected_scope: Option<&str>,
        scanned: &[provenance_scanner::FileScanWithContent],
    ) -> Result<CategoryRun, String> {
        let scan = self
            .scan_selected_scopes(selected_scope, scanned)
            .map_err(|error| format!("{error:#}"))?;
        let severity = match scan.policy {
            BindingFindingsSeverity::Warning => provenance_scanner::BindingFindingSeverity::Warning,
            BindingFindingsSeverity::Error => provenance_scanner::BindingFindingSeverity::Error,
        };
        let refuses =
            provenance_scanner::binding_findings_fail(severity, scan.governed_finding_count)
                || (self.strict && !scan.warnings.is_empty());
        let refusal = if refuses {
            Refusal::Findings
        } else {
            Refusal::None
        };
        let findings = scan
            .warnings
            .into_iter()
            .map(|warning| {
                Finding::with_detail(
                    &warning.message,
                    serde_json::to_value(&warning).expect("coverage warning is JSON"),
                )
            })
            .collect();
        let policy = match scan.policy {
            BindingFindingsSeverity::Warning => BindingPolicy::Warning,
            BindingFindingsSeverity::Error => BindingPolicy::Error,
        };
        Ok(CategoryRun::Bindings {
            findings,
            context: BindingContext { policy },
            refusal,
        })
    }

    fn scan_selected_scopes(
        &self,
        selected_scope: Option<&str>,
        scanned: &[provenance_scanner::FileScanWithContent],
    ) -> anyhow::Result<Scan> {
        let manifest = provenance_store::layout::with_initialized_graph(
            &ProvenanceLayout::new(&self.context.repo),
            Ok,
        )?;
        let scopes = match selected_scope {
            Some(selected) => vec![declared_scope(&manifest, selected)?],
            None => manifest.scopes.iter().collect(),
        };
        let path = self.scan_path();
        let mut scan = Scan {
            warnings: Vec::new(),
            governed_finding_count: 0,
            policy: BindingFindingsSeverity::default(),
        };
        for scope in scopes {
            let repository =
                RepoContext::new(self.context.repo.clone(), scope.id.as_str()).open()?;
            scan.policy = repository.settings().coverage.binding_findings;
            let outcome = super::super::coverage::coverage_scan_from_scanned(
                &repository,
                &self.context.repo,
                &path,
                scope,
                &manifest,
                scanned,
                self.bindings.baseline.as_deref(),
            )?;
            // The CLI takes --output only with --scope, so one scope writes the file.
            if let Some((output, format)) = &self.bindings.output {
                let rendered = super::super::coverage::render_report(*format, &outcome.scan)?;
                std::fs::write(output, rendered)?;
            }
            scan.governed_finding_count += outcome.governed_finding_count;
            scan.warnings.extend(outcome.scan.report.warnings);
        }
        Ok(scan)
    }
}
