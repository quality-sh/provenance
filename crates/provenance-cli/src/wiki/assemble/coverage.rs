use crate::wiki::model::{CodeScan, ImplementationBinding, VerificationSite};
use provenance_core::coverage::{CoverageReport, CoverageSite, SiteRole};

use super::context::Assembler;

fn native_first(report: &CoverageReport) -> Vec<CoverageSite<'_>> {
    let mut sites = report.sites().collect::<Vec<_>>();
    sites.sort_by_key(|site| matches!(site, CoverageSite::Annotation(_)));
    sites
}

impl Assembler<'_> {
    /// The scan this build read, so a page can say which code it looked at
    /// and a page built without a scan can say that instead.
    pub(super) fn code_scan(&self) -> Option<CodeScan> {
        self.coverage.map(|report| CodeScan {
            commit: report.commit.clone(),
        })
    }

    pub(super) fn implementations(&self, rule_id: &str) -> Vec<ImplementationBinding> {
        let scanned = self.coverage.and_then(|report| {
            native_first(report).into_iter().find(|site| {
                let core = site.core();
                core.rule_id == rule_id
                    && matches!(site.role(), SiteRole::Implementation)
                    && site.is_current()
            })
        });
        let mut implementations = Vec::new();
        if let Some(site) = scanned {
            implementations.push(ImplementationBinding {
                symbol: site.symbol().map(str::to_string),
                location: self.site_location(site),
            });
        }
        for binding in self
            .state
            .implementation_bindings
            .iter()
            .filter(|binding| binding.rule_id.as_str() == rule_id)
        {
            let matches_scan = scanned.is_some_and(|site| {
                site.core().file_path == binding.file
                    && site.symbol() == Some(binding.symbol.as_str())
            });
            if !matches_scan {
                implementations.push(ImplementationBinding {
                    symbol: Some(binding.symbol.clone()),
                    location: self.resolver.resolve_at(binding.file.as_str(), None),
                });
            }
        }
        implementations
    }

    pub(super) fn verification_sites(&self, rule_id: &str) -> Vec<VerificationSite> {
        let implementation_file = self.coverage.and_then(|report| {
            native_first(report)
                .into_iter()
                .find(|site| {
                    site.core().rule_id == rule_id
                        && matches!(site.role(), SiteRole::Implementation)
                        && site.is_current()
                })
                .map(|site| &site.core().file_path)
        });
        let mut sites = self
            .coverage
            .into_iter()
            .flat_map(native_first)
            .filter(|site| site.core().rule_id == rule_id && site.is_current())
            .filter_map(|site| {
                let SiteRole::Verification(method) = site.role() else {
                    return None;
                };
                Some(VerificationSite {
                    method: method.to_string(),
                    symbol: site.symbol().map(str::to_string),
                    location: self.site_location(site),
                    outside_implementation_module: implementation_file
                        .is_some_and(|file| file != &site.core().file_path),
                })
            })
            .collect::<Vec<_>>();
        for binding in self
            .state
            .verification_bindings
            .iter()
            .filter(|binding| binding.rule_id.as_str() == rule_id)
        {
            let typed = VerificationSite {
                method: binding.method.to_string(),
                symbol: binding.symbol.clone(),
                location: self.resolver.resolve_at(binding.file.as_str(), None),
                outside_implementation_module: implementation_file
                    .is_some_and(|file| file != &binding.file),
            };
            if !sites.iter().any(|site| {
                site.method == typed.method
                    && site.symbol == typed.symbol
                    && site.location.label == typed.location.label
            }) {
                sites.push(typed);
            }
        }
        sites
    }

    fn site_location(&self, site: CoverageSite<'_>) -> crate::wiki::links::EvidenceRef {
        let site = site.core();
        let reference = format!("{}:{}", site.file_path, site.line);
        self.resolver.resolve_at(
            &reference,
            self.coverage.and_then(|report| report.commit.as_deref()),
        )
    }
}
