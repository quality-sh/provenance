use camino::{Utf8Path, Utf8PathBuf};
use provenance_core::coverage::{AnchorState, ValidationWarning};

use crate::{scan_file, scan_to_coverage, CoverageBaseline, FileScanWithContent, Language};

struct ReconciliationCase {
    path: &'static str,
    language: Language,
    baseline: &'static str,
    current: &'static str,
}

fn scanned(path: &str, language: Language, content: &str) -> FileScanWithContent {
    FileScanWithContent {
        scan: scan_file(Utf8Path::new(path), language, content),
        content: content.to_string(),
    }
}

#[test]
fn scan_to_coverage_reconciles_all_anchor_states_for_supported_language_shapes() {
    let cases = [
        ReconciliationCase {
            path: "src/rules.rs",
            language: Language::Rust,
            baseline: "// @provenance rule: rule_unchanged\nfn unchanged() {}\n\
                       // @provenance rule: rule_moved\nfn moved() {}\n\
                       // @provenance rule: rule_gone\nfn gone() {}\n",
            current: "// @provenance rule: rule_unchanged\nfn unchanged() {}\n\n\
                      // @provenance rule: rule_moved\nfn moved() {}\n\
                      // @provenance rule: rule_new\nfn new_rule() {}\n",
        },
        ReconciliationCase {
            path: "src/rules.ts",
            language: Language::TypeScript,
            baseline: "// @provenance rule: rule_unchanged\nfunction unchanged() {}\n\
                       // @provenance rule: rule_moved\nfunction moved() {}\n\
                       // @provenance rule: rule_gone\nfunction gone() {}\n",
            current: "// @provenance rule: rule_unchanged\nfunction unchanged() {}\n\n\
                      // @provenance rule: rule_moved\nfunction moved() {}\n\
                      // @provenance rule: rule_new\nfunction newRule() {}\n",
        },
        ReconciliationCase {
            path: "src/rules.py",
            language: Language::Python,
            baseline: "# @provenance rule: rule_unchanged\ndef unchanged(): pass\n\
                       # @provenance rule: rule_moved\ndef moved(): pass\n\
                       # @provenance rule: rule_gone\ndef gone(): pass\n",
            current: "# @provenance rule: rule_unchanged\ndef unchanged(): pass\n\n\
                      # @provenance rule: rule_moved\ndef moved(): pass\n\
                      # @provenance rule: rule_new\ndef new_rule(): pass\n",
        },
    ];

    for case in cases {
        let baseline_files = vec![scanned(case.path, case.language, case.baseline)];
        let baseline = scan_to_coverage(&baseline_files, None, Vec::new(), None).into_scan();
        let current_files = vec![scanned(case.path, case.language, case.current)];
        let coverage = scan_to_coverage(
            &current_files,
            None,
            Vec::new(),
            Some(CoverageBaseline {
                scan: &baseline,
                repo: Utf8Path::new("."),
                scan_path: Utf8Path::new("."),
                validate_rules: false,
            }),
        );

        let state = |rule_id: &str| {
            coverage
                .annotations
                .iter()
                .find(|site| site.rule_id == rule_id)
                .map(|site| site.anchor_state)
        };
        assert_eq!(
            state("rule_unchanged"),
            Some(AnchorState::Unchanged),
            "{}",
            case.path
        );
        assert_eq!(state("rule_new"), Some(AnchorState::New), "{}", case.path);
        assert_eq!(
            state("rule_moved"),
            Some(AnchorState::Moved),
            "{}",
            case.path
        );
        assert_eq!(state("rule_gone"), Some(AnchorState::Gone), "{}", case.path);
    }
}

#[test]
fn scan_to_coverage_keeps_python_and_typescript_body_extents() {
    let cases = [
        (
            "src/rules.py",
            Language::Python,
            "# @provenance rule: rule_extent\ndef f():\n    return 1\nnext_value = 2\n",
            "# @provenance rule: rule_extent\ndef f():\n    return 2\nnext_value = 2\n",
            3,
        ),
        (
            "src/rules.ts",
            Language::TypeScript,
            "// @provenance rule: rule_extent\nfunction f() {\n  return 1;\n}\n",
            "// @provenance rule: rule_extent\nfunction f() {\n  return 2;\n}\n",
            4,
        ),
    ];

    for (path, language, baseline_source, current_source, expected_end) in cases {
        let baseline_files = vec![scanned(path, language, baseline_source)];
        let baseline = scan_to_coverage(&baseline_files, None, Vec::new(), None).into_scan();
        let current_files = vec![scanned(path, language, current_source)];
        let coverage = scan_to_coverage(
            &current_files,
            None,
            Vec::new(),
            Some(CoverageBaseline {
                scan: &baseline,
                repo: Utf8Path::new("."),
                scan_path: Utf8Path::new("."),
                validate_rules: false,
            }),
        );
        let site = &coverage.annotations[0];

        assert_eq!(site.anchor_state, AnchorState::Unchanged, "{path}");
        assert_eq!(
            coverage.site_end_line(Utf8Path::new(path), site.line),
            Some(expected_end),
            "{path}"
        );
    }
}

#[test]
fn scan_to_coverage_projects_parser_warnings_before_existing_warnings() {
    let files = vec![scanned(
        "src/rules.rs",
        Language::Rust,
        "// @provenance rule rule_broken\nfn broken() {}\n",
    )];
    let existing = ValidationWarning {
        rule_id: "rule_existing".to_string(),
        file_path: Some(Utf8PathBuf::from("src/existing.rs")),
        line: Some(9),
        message: "existing validation warning".to_string(),
        binding_finding: false,
    };

    let coverage = scan_to_coverage(&files, None, vec![existing.clone()], None);

    assert_eq!(coverage.warnings.len(), 2);
    assert!(coverage.warnings[0].message.contains("malformed directive"));
    assert_eq!(
        coverage.warnings[0].file_path.as_deref(),
        Some(Utf8Path::new("src/rules.rs"))
    );
    assert_eq!(coverage.warnings[0].line, Some(1));
    assert_eq!(coverage.warnings[1], existing);
}

#[test]
fn scan_to_coverage_owns_projection_state_and_symbol_spans() {
    let content = "// @provenance rule: rule_comment\nfn implements_comment() {}\n\n\
                   #[verifies(\"rule_native\", examples)]\nfn verifies_native() {\n    assert!(true);\n}";
    let files = vec![scanned("src/rules.rs", Language::Rust, content)];

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
    assert_eq!(coverage.scanned_files[0].content, content);
}
