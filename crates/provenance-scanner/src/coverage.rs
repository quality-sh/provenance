use provenance_core::coverage::{AnchorState, AnnotationResult, BindingResult, SiteCore};

use crate::FileScan;

pub struct CoverageResults {
    pub annotations: Vec<AnnotationResult>,
    pub bindings: Vec<BindingResult>,
}

/// Converts scanner sites to the stable coverage report records.
pub fn coverage_results(scans: &[FileScan]) -> CoverageResults {
    let annotations = scans
        .iter()
        .flat_map(|scan| &scan.annotations)
        .map(|location| AnnotationResult {
            site: SiteCore {
                rule_id: location.annotation.rule.clone(),
                file_path: location.file_path.clone(),
                line: location.line,
                verification: location
                    .annotation
                    .verification
                    .map(|method| method.to_string()),
                anchor: Some(location.anchor.clone()),
                anchor_state: AnchorState::New,
                original_line: None,
                original_file_path: None,
            },
            function_name: location.function_name.clone(),
            coverage: location.annotation.coverage.to_string(),
            confidence: location.annotation.confidence,
        })
        .collect();
    let bindings = scans
        .iter()
        .flat_map(|scan| &scan.bindings)
        .map(|binding| BindingResult {
            site: SiteCore {
                rule_id: binding.rule_id.clone(),
                file_path: binding.file_path.clone(),
                line: binding.line,
                verification: binding.verification.map(|method| method.to_string()),
                anchor: Some(binding.anchor.clone()),
                anchor_state: AnchorState::New,
                original_line: None,
                original_file_path: None,
            },
            item_name: binding.item_name.clone(),
        })
        .collect();
    CoverageResults {
        annotations,
        bindings,
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use provenance_core::coverage::AnchorState;

    use crate::{coverage_results, scan_file, Language};

    #[test]
    fn coverage_results_preserve_scanner_site_data() {
        let scan = scan_file(
            Utf8Path::new("src/rules.rs"),
            Language::Rust,
            "// @provenance rule: rule_comment\nfn implements_comment() {}\n\n\
             #[verifies(\"rule_native\", examples)]\nfn verifies_native() {}",
        );

        let results = coverage_results(&[scan]);

        let annotation = &results.annotations[0];
        assert_eq!(annotation.rule_id, "rule_comment");
        assert_eq!(
            annotation.function_name.as_deref(),
            Some("implements_comment")
        );
        assert_eq!(annotation.anchor_state, AnchorState::New);
        assert!(annotation.anchor.is_some());
        let binding = &results.bindings[0];
        assert_eq!(binding.rule_id, "rule_native");
        assert_eq!(binding.item_name.as_deref(), Some("verifies_native"));
        assert_eq!(binding.verification.as_deref(), Some("examples"));
        assert_eq!(binding.anchor_state, AnchorState::New);
        assert!(binding.anchor.is_some());
    }
}
