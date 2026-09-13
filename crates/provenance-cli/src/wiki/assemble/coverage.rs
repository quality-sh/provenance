use crate::wiki::model::{CodeScan, ImplementationBinding, VerificationSite};
use provenance_core::coverage::{AnchorState, AnnotationResult, BindingResult};

use super::context::Assembler;

impl Assembler<'_> {
    /// The scan this build read, so a page can say which code it looked at
    /// and a page built without a scan can say that instead.
    pub(super) fn code_scan(&self) -> Option<CodeScan> {
        self.coverage.map(|report| CodeScan {
            commit: report.commit.clone(),
        })
    }

    pub(super) fn implementations(&self, rule_id: &str) -> Vec<ImplementationBinding> {
        let scanned_binding = self.coverage.and_then(|report| {
            report.bindings.iter().find(|binding| {
                binding.rule_id == rule_id
                    && binding.verification.is_none()
                    && binding.anchor_state != AnchorState::Gone
            })
        });
        let scanned_annotation = self.coverage.and_then(|report| {
            report.annotations.iter().find(|annotation| {
                annotation.rule_id == rule_id
                    && annotation.verification.is_none()
                    && annotation.anchor_state != AnchorState::Gone
            })
        });
        let mut implementations = Vec::new();
        if let Some(binding) = scanned_binding {
            implementations.push(ImplementationBinding {
                symbol: binding.item_name.clone(),
                location: self.binding_location(binding),
            });
        } else if let Some(annotation) = scanned_annotation {
            implementations.push(ImplementationBinding {
                symbol: annotation.function_name.clone(),
                location: self.annotation_location(annotation),
            });
        }
        for binding in self
            .state
            .implementation_bindings
            .iter()
            .filter(|binding| binding.rule_id.as_str() == rule_id)
        {
            let matches_scan = scanned_binding.map_or_else(
                || {
                    scanned_annotation.is_some_and(|scanned| {
                        scanned.file_path == binding.file
                            && scanned.function_name.as_deref() == Some(binding.symbol.as_str())
                    })
                },
                |scanned| {
                    scanned.file_path == binding.file
                        && scanned.item_name.as_deref() == Some(binding.symbol.as_str())
                },
            );
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
        let implementation_file = self
            .coverage
            .into_iter()
            .flat_map(|report| &report.bindings)
            .find(|binding| {
                binding.rule_id == rule_id
                    && binding.verification.is_none()
                    && binding.anchor_state != AnchorState::Gone
            })
            .map(|binding| &binding.file_path)
            .or_else(|| {
                self.coverage.and_then(|report| {
                    report
                        .annotations
                        .iter()
                        .find(|annotation| {
                            annotation.rule_id == rule_id
                                && annotation.verification.is_none()
                                && annotation.anchor_state != AnchorState::Gone
                        })
                        .map(|annotation| &annotation.file_path)
                })
            });
        let mut sites = self
            .coverage
            .into_iter()
            .flat_map(|report| &report.bindings)
            .filter(|binding| binding.rule_id == rule_id)
            .filter(|binding| binding.anchor_state != AnchorState::Gone)
            .filter_map(|binding| {
                binding
                    .verification
                    .as_ref()
                    .map(|method| VerificationSite {
                        method: method.clone(),
                        symbol: binding.item_name.clone(),
                        location: self.binding_location(binding),
                        outside_implementation_module: implementation_file
                            .is_some_and(|file| file != &binding.file_path),
                    })
            })
            .collect::<Vec<_>>();
        sites.extend(
            self.coverage
                .into_iter()
                .flat_map(|report| &report.annotations)
                .filter(|annotation| annotation.rule_id == rule_id)
                .filter(|annotation| annotation.anchor_state != AnchorState::Gone)
                .filter_map(|annotation| {
                    annotation
                        .verification
                        .as_ref()
                        .map(|method| VerificationSite {
                            method: method.clone(),
                            symbol: annotation.function_name.clone(),
                            location: self.annotation_location(annotation),
                            outside_implementation_module: implementation_file
                                .is_some_and(|file| file != &annotation.file_path),
                        })
                }),
        );
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

    fn binding_location(&self, binding: &BindingResult) -> crate::wiki::links::EvidenceRef {
        let reference = format!("{}:{}", binding.file_path, binding.line);
        self.resolver.resolve_at(
            &reference,
            self.coverage.and_then(|report| report.commit.as_deref()),
        )
    }

    fn annotation_location(
        &self,
        annotation: &AnnotationResult,
    ) -> crate::wiki::links::EvidenceRef {
        let reference = format!("{}:{}", annotation.file_path, annotation.line);
        self.resolver.resolve_at(
            &reference,
            self.coverage.and_then(|report| report.commit.as_deref()),
        )
    }
}
