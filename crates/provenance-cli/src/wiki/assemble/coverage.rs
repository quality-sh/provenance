use crate::wiki::model::{CodeScan, ImplementationBinding, VerificationSite};
use provenance_core::coverage::{
    AnnotationResult, BindingResult, CoverageReport, SiteCore, SiteRole,
};

use super::context::Assembler;

#[derive(Clone, Copy)]
enum ScannedSite<'a> {
    Annotation(&'a AnnotationResult),
    Binding(&'a BindingResult),
}

impl<'a> ScannedSite<'a> {
    const fn core(self) -> &'a SiteCore {
        match self {
            Self::Annotation(site) => &site.site,
            Self::Binding(site) => &site.site,
        }
    }

    fn symbol(self) -> Option<&'a str> {
        match self {
            Self::Annotation(site) => site.function_name.as_deref(),
            Self::Binding(site) => site.item_name.as_deref(),
        }
    }

    fn location(self, assembler: &Assembler<'_>) -> crate::wiki::links::EvidenceRef {
        let site = self.core();
        let reference = format!("{}:{}", site.file_path, site.line);
        assembler.resolver.resolve_at(
            &reference,
            assembler
                .coverage
                .and_then(|report| report.commit.as_deref()),
        )
    }
}

fn scanned_sites(report: &CoverageReport) -> impl Iterator<Item = ScannedSite<'_>> {
    report
        .bindings
        .iter()
        .map(ScannedSite::Binding)
        .chain(report.annotations.iter().map(ScannedSite::Annotation))
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
            scanned_sites(report).find(|site| {
                let core = site.core();
                core.rule_id == rule_id
                    && core.role() == SiteRole::Implementation
                    && core.is_current()
            })
        });
        let mut implementations = Vec::new();
        if let Some(site) = scanned {
            implementations.push(ImplementationBinding {
                symbol: site.symbol().map(str::to_string),
                location: site.location(self),
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
            scanned_sites(report)
                .map(ScannedSite::core)
                .find(|site| {
                    site.rule_id == rule_id
                        && site.role() == SiteRole::Implementation
                        && site.is_current()
                })
                .map(|site| &site.file_path)
        });
        let mut sites = self
            .coverage
            .into_iter()
            .flat_map(scanned_sites)
            .filter(|site| {
                let core = site.core();
                core.rule_id == rule_id
                    && core.role() == SiteRole::Verification
                    && core.is_current()
            })
            .map(|site| VerificationSite {
                method: site
                    .core()
                    .verification
                    .clone()
                    .expect("verification sites have a method"),
                symbol: site.symbol().map(str::to_string),
                location: site.location(self),
                outside_implementation_module: implementation_file
                    .is_some_and(|file| file != &site.core().file_path),
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
}
