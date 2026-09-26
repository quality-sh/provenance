use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::coverage::{
    AnchorState, AnnotationResult, BindingResult, CoverageReport, CoverageScan, ScannedFile,
    SiteCore, ValidationWarning,
};

use crate::{FileScanWithContent, Language};

/// A prior report and the paths that define its scan area.
pub struct CoverageBaseline<'a> {
    pub scan: &'a CoverageScan,
    pub repo: &'a Utf8Path,
    pub scan_path: &'a Utf8Path,
    pub validate_rules: bool,
}

/// A complete coverage report with source spans for each scanned site.
pub struct ScannedCoverage {
    scan: CoverageScan,
    spans: BTreeMap<(Utf8PathBuf, usize), usize>,
}

impl ScannedCoverage {
    pub fn site_end_line(&self, path: &Utf8Path, line: usize) -> Option<usize> {
        self.spans.get(&(path.to_path_buf(), line)).copied()
    }

    pub fn into_scan(self) -> CoverageScan {
        self.scan
    }
}

impl Deref for ScannedCoverage {
    type Target = CoverageScan;

    fn deref(&self) -> &Self::Target {
        &self.scan
    }
}

impl DerefMut for ScannedCoverage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.scan
    }
}

/// Build one coverage report from the files that the scanner read.
pub fn scan_to_coverage(
    files: &[FileScanWithContent],
    commit: Option<String>,
    mut warnings: Vec<ValidationWarning>,
    baseline: Option<CoverageBaseline<'_>>,
) -> ScannedCoverage {
    let annotations = files
        .iter()
        .flat_map(|file| &file.scan.annotations)
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
    let bindings = files
        .iter()
        .flat_map(|file| &file.scan.bindings)
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
    warnings.splice(0..0, parse_warnings(files));
    let scanned_files = files
        .iter()
        .map(|file| ScannedFile {
            file_path: file.scan.file_path.clone(),
            content: file.content.clone(),
        })
        .collect();
    let mut scan = CoverageScan {
        report: CoverageReport::new(commit, files.len(), annotations, bindings, warnings),
        scanned_files,
    };
    if let Some(baseline) = baseline {
        crate::coverage_anchors::reconcile(
            &mut scan,
            baseline.scan,
            baseline.repo,
            baseline.scan_path,
            baseline.validate_rules,
        );
    }
    ScannedCoverage {
        spans: site_spans(files),
        scan,
    }
}

fn parse_warnings(files: &[FileScanWithContent]) -> Vec<ValidationWarning> {
    files
        .iter()
        .flat_map(|file| {
            file.scan.warnings.iter().map(|warning| ValidationWarning {
                rule_id: String::new(),
                file_path: Some(file.scan.file_path.clone()),
                line: Some(warning.line),
                message: warning.message.clone(),
                binding_finding: false,
            })
        })
        .collect()
}

fn site_spans(files: &[FileScanWithContent]) -> BTreeMap<(Utf8PathBuf, usize), usize> {
    let mut spans = BTreeMap::new();
    for file in files {
        let lines = file.content.lines().collect::<Vec<_>>();
        for site in &file.scan.annotations {
            let end = symbol_end(
                &lines,
                site.line,
                site.function_name.as_deref(),
                file.scan.language,
            );
            spans.insert((site.file_path.clone(), site.line), end);
        }
        for site in &file.scan.bindings {
            let end = symbol_end(
                &lines,
                site.line,
                site.item_name.as_deref(),
                file.scan.language,
            );
            spans.insert((site.file_path.clone(), site.line), end);
        }
    }
    spans
}

fn symbol_end(
    lines: &[&str],
    marker_line: usize,
    symbol: Option<&str>,
    language: Language,
) -> usize {
    let Some(symbol) = symbol else {
        return marker_line;
    };
    let marker_index = marker_line.saturating_sub(1);
    let declaration = lines
        .iter()
        .enumerate()
        .skip(marker_index)
        .take(8)
        .find(|(_, line)| line.contains(symbol))
        .map(|(index, _)| index);
    let Some(declaration) = declaration else {
        return marker_line;
    };
    if language == Language::Python {
        return python_symbol_end(lines, marker_line, declaration);
    }
    brace_symbol_end(lines, marker_line, declaration)
}

fn brace_symbol_end(lines: &[&str], marker_line: usize, declaration: usize) -> usize {
    let mut depth = 0usize;
    let mut opened = false;
    for (index, line) in lines.iter().enumerate().skip(declaration) {
        for character in line.chars() {
            match character {
                '{' => {
                    opened = true;
                    depth += 1;
                }
                '}' if opened => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if (opened && depth == 0) || (!opened && line.trim_end().ends_with(';')) {
            return index + 1;
        }
    }
    marker_line.max(declaration + 1)
}

fn python_symbol_end(lines: &[&str], marker_line: usize, declaration: usize) -> usize {
    let indentation = lines[declaration]
        .chars()
        .take_while(|character| character.is_whitespace())
        .count();
    let mut end = declaration + 1;
    for (index, line) in lines.iter().enumerate().skip(declaration + 1) {
        if line.trim().is_empty() {
            continue;
        }
        let current = line
            .chars()
            .take_while(|character| character.is_whitespace())
            .count();
        if current <= indentation {
            break;
        }
        end = index + 1;
    }
    marker_line.max(end)
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;
    use provenance_core::coverage::AnchorState;

    use crate::{scan_file, scan_to_coverage, FileScanWithContent, Language};

    #[test]
    fn scan_to_coverage_owns_projection_state_and_symbol_spans() {
        let scan = scan_file(
            Utf8Path::new("src/rules.rs"),
            Language::Rust,
            "// @provenance rule: rule_comment\nfn implements_comment() {}\n\n\
             #[verifies(\"rule_native\", examples)]\nfn verifies_native() {\n    assert!(true);\n}",
        );
        let files = vec![FileScanWithContent {
            scan,
            content: "// @provenance rule: rule_comment\nfn implements_comment() {}\n\n\
                      #[verifies(\"rule_native\", examples)]\nfn verifies_native() {\n    assert!(true);\n}"
                .to_string(),
        }];

        let coverage = scan_to_coverage(&files, Some("abc123".to_string()), Vec::new(), None);

        let annotation = &coverage.annotations[0];
        assert_eq!(annotation.rule_id, "rule_comment");
        assert_eq!(
            annotation.function_name.as_deref(),
            Some("implements_comment")
        );
        assert_eq!(annotation.anchor_state, AnchorState::New);
        assert!(annotation.anchor.is_some());
        let binding = &coverage.bindings[0];
        assert_eq!(binding.rule_id, "rule_native");
        assert_eq!(binding.item_name.as_deref(), Some("verifies_native"));
        assert_eq!(binding.verification.as_deref(), Some("examples"));
        assert_eq!(binding.anchor_state, AnchorState::New);
        assert!(binding.anchor.is_some());
        assert_eq!(
            coverage.site_end_line(Utf8Path::new("src/rules.rs"), binding.line),
            Some(7)
        );
        assert_eq!(coverage.commit.as_deref(), Some("abc123"));
        assert_eq!(coverage.files_scanned, 1);
        assert_eq!(
            coverage.scanned_files,
            vec![provenance_core::coverage::ScannedFile {
                file_path: "src/rules.rs".into(),
                content: files[0].content.clone(),
            }]
        );
    }
}
