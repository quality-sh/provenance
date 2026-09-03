use super::super::{build_corpus_with_coverage, load_coverage_report, repository_relative_path};
use super::fixtures::*;
use crate::wiki::links::LinkResolver;
use crate::wiki::render::render_rule;
use camino::Utf8PathBuf;
use provenance_core::coverage::{
    AnchorState, AnnotationResult, BindingResult, CoverageReport, CoverageScan, ScannedFile,
};
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    ImplementationBinding as CanonicalImplementationBinding, ScopeId, StableId,
    VerificationBinding, VerificationMethod,
};
use std::fmt::Write as _;

fn binding(
    file_path: &str,
    line: usize,
    item_name: &str,
    verification: Option<&str>,
) -> BindingResult {
    BindingResult {
        rule_id: "rule_001".to_string(),
        file_path: Utf8PathBuf::from(file_path),
        line,
        item_name: Some(item_name.to_string()),
        verification: verification.map(str::to_string),
        anchor: None,
        anchor_state: AnchorState::Unchanged,
        original_line: None,
        original_file_path: None,
    }
}

fn annotation(
    file_path: &str,
    line: usize,
    function_name: &str,
    verification: Option<&str>,
) -> AnnotationResult {
    AnnotationResult {
        rule_id: "rule_001".to_string(),
        file_path: Utf8PathBuf::from(file_path),
        line,
        function_name: Some(function_name.to_string()),
        coverage: "full".to_string(),
        confidence: 1.0,
        verification: verification.map(str::to_string),
        anchor: None,
        anchor_state: AnchorState::Unchanged,
        original_line: None,
        original_file_path: None,
    }
}

fn typed_binding() -> VerificationBinding {
    VerificationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new("verification_binding_expiry_examples").unwrap(),
        rule_id: StableId::new("rule_001").unwrap(),
        key: "share-link-expiry".to_string(),
        method: VerificationMethod::Examples,
        declared_by: "ci://typescript".to_string(),
        retired: false,
        file: Utf8PathBuf::from("tests/share-links.test.ts"),
        symbol: Some("share links expire".to_string()),
    }
}

fn typed_implementation(file: &str, symbol: &str) -> CanonicalImplementationBinding {
    CanonicalImplementationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new("implementation_binding_rule_001").unwrap(),
        rule_id: StableId::new("rule_001").unwrap(),
        declared_by: "spec://typescript/payroll".to_string(),
        retired: false,
        file: Utf8PathBuf::from(file),
        symbol: symbol.to_string(),
    }
}

#[test]
fn typed_implementation_is_visible_without_a_code_scan() {
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));
    let mut state = fixture_state();
    state
        .implementation_bindings
        .push(typed_implementation("src/payroll.ts", "calculatePayroll"));

    let corpus = build_corpus_with_coverage(&state, &resolver, None);
    let page = rule_page(&corpus, "rule_001");
    let html = render_rule("default", page);

    assert!(page.code_scan.is_none());
    assert!(html.contains(">Implementation</h2>"), "{html}");
    assert!(html.contains("calculatePayroll"), "{html}");
    assert!(html.contains("src/payroll.ts"), "{html}");
}

#[test]
fn retired_typed_implementation_is_not_presented_as_current() {
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));
    let mut state = fixture_state();
    let mut retired = typed_implementation("src/payroll.ts", "calculatePayroll");
    retired.retired = true;
    state.implementation_bindings.push(retired);

    let corpus = build_corpus_with_coverage(&state, &resolver, None);
    let page = rule_page(&corpus, "rule_001");
    let html = render_rule("default", page);

    assert!(page.implementations.is_empty());
    assert!(!html.contains("calculatePayroll"), "{html}");
}

#[test]
fn matching_scanner_and_typed_implementation_render_once() {
    let report = CoverageScan {
        report: CoverageReport::new(
            Some("abc1234".to_string()),
            1,
            Vec::new(),
            vec![binding("src/payroll.ts", 7, "calculatePayroll", None)],
            Vec::new(),
        ),
        scanned_files: Vec::new(),
    };
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));
    let mut state = fixture_state();
    state
        .implementation_bindings
        .push(typed_implementation("src/payroll.ts", "calculatePayroll"));

    let corpus = build_corpus_with_coverage(&state, &resolver, Some(&report));
    let html = render_rule("default", rule_page(&corpus, "rule_001"));

    assert_eq!(html.matches("calculatePayroll").count(), 1, "{html}");
    assert!(html.contains("src/payroll.ts:7"), "{html}");
}

#[test]
fn distinct_scanner_and_typed_primary_claims_remain_visible() {
    let report = CoverageScan {
        report: CoverageReport::new(
            Some("abc1234".to_string()),
            1,
            Vec::new(),
            vec![binding("src/scanned.ts", 7, "scannedPayroll", None)],
            Vec::new(),
        ),
        scanned_files: Vec::new(),
    };
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));
    let mut state = fixture_state();
    state
        .implementation_bindings
        .push(typed_implementation("src/typed.ts", "typedPayroll"));

    let corpus = build_corpus_with_coverage(&state, &resolver, Some(&report));
    let html = render_rule("default", rule_page(&corpus, "rule_001"));

    assert!(html.contains("scannedPayroll"), "{html}");
    assert!(html.contains("src/scanned.ts:7"), "{html}");
    assert!(html.contains("typedPayroll"), "{html}");
    assert!(html.contains("src/typed.ts"), "{html}");
}

#[test]
fn typed_claim_matching_a_non_rendered_scanner_claim_remains_visible() {
    let report = CoverageScan {
        report: CoverageReport::new(
            Some("abc1234".to_string()),
            1,
            vec![annotation("src/typed.ts", 9, "typedPayroll", None)],
            vec![binding("src/scanned.ts", 7, "scannedPayroll", None)],
            Vec::new(),
        ),
        scanned_files: Vec::new(),
    };
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));
    let mut state = fixture_state();
    state
        .implementation_bindings
        .push(typed_implementation("src/typed.ts", "typedPayroll"));

    let corpus = build_corpus_with_coverage(&state, &resolver, Some(&report));
    let html = render_rule("default", rule_page(&corpus, "rule_001"));

    assert_eq!(html.matches("scannedPayroll").count(), 1, "{html}");
    assert_eq!(html.matches("typedPayroll").count(), 1, "{html}");
}

#[test]
fn coverage_loader_makes_annotation_paths_repository_relative() {
    let dir = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(dir.path().join("repo")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    let absolute_file = repo.join("src/payroll.ts");
    let scan = CoverageScan {
        report: CoverageReport::new(
            Some("abc1234".to_string()),
            1,
            vec![annotation(absolute_file.as_str(), 9, "typedPayroll", None)],
            Vec::new(),
            Vec::new(),
        ),
        scanned_files: Vec::new(),
    };
    let report_path = repo.join("coverage.json");
    std::fs::write(&report_path, serde_json::to_vec(&scan).unwrap()).unwrap();

    let loaded = load_coverage_report(&report_path, &repo).unwrap();

    assert_eq!(
        loaded.report.annotations[0].file_path,
        Utf8PathBuf::from("src/payroll.ts")
    );
}

#[test]
fn typed_verification_binding_is_visible_without_a_code_scan() {
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));
    let mut state = fixture_state();
    state.verification_bindings.push(typed_binding());

    let corpus = build_corpus_with_coverage(&state, &resolver, None);
    let page = rule_page(&corpus, "rule_001");

    assert!(page.code_scan.is_none());
    assert_eq!(page.verifications.len(), 1);
    assert_eq!(page.verifications[0].method, "examples");
    assert_eq!(
        page.verifications[0].symbol.as_deref(),
        Some("share links expire")
    );
    assert_eq!(
        page.verifications[0].location.label,
        "tests/share-links.test.ts"
    );
}

#[test]
fn retired_typed_verification_is_not_presented_as_current() {
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));
    let mut state = fixture_state();
    let mut retired = typed_binding();
    retired.retired = true;
    state.verification_bindings.push(retired);

    let corpus = build_corpus_with_coverage(&state, &resolver, None);
    let page = rule_page(&corpus, "rule_001");

    assert!(page.verifications.is_empty());
}

#[test]
fn comment_annotations_become_implementation_and_verification_sites() {
    let report = CoverageScan {
        report: CoverageReport::new(
            Some("abc1234".to_string()),
            2,
            vec![
                annotation("src/rules.py", 7, "implement_rule", None),
                annotation("tests/test_rules.py", 12, "verify_rule", Some("examples")),
            ],
            Vec::new(),
            Vec::new(),
        ),
        scanned_files: Vec::new(),
    };
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));

    let corpus = build_corpus_with_coverage(&fixture_state(), &resolver, Some(&report));
    let page = rule_page(&corpus, "rule_001");

    assert_eq!(
        page.implementations[0].symbol.as_deref(),
        Some("implement_rule")
    );
    assert_eq!(page.verifications.len(), 1);
    assert_eq!(page.verifications[0].symbol.as_deref(), Some("verify_rule"));
    assert!(page.verifications[0].outside_implementation_module);
}

#[test]
fn coverage_bindings_become_commit_pinned_implementation_and_verification_sites() {
    let report = CoverageScan {
        report: CoverageReport::new(
            Some("abc1234".to_string()),
            2,
            Vec::new(),
            vec![
                binding("src/rules.rs", 7, "decide_rule", None),
                binding(
                    "src/rules.rs",
                    21,
                    "rule_holds_by_exhaustion",
                    Some("exhaustion"),
                ),
                binding(
                    "tests/rules.rs",
                    12,
                    "rule_holds_for_examples",
                    Some("examples"),
                ),
            ],
            Vec::new(),
        ),
        scanned_files: vec![ScannedFile {
            file_path: "src/UseCase.php".into(),
            content: (1..=200).fold(String::new(), |mut content, line| {
                writeln!(content, "line {line}").unwrap();
                content
            }),
        }],
    };
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));

    let corpus = build_corpus_with_coverage(&fixture_state(), &resolver, Some(&report));
    let page = rule_page(&corpus, "rule_001");

    let implementation = &page.implementations[0];
    assert_eq!(implementation.symbol.as_deref(), Some("decide_rule"));
    assert_eq!(implementation.location.label, "src/rules.rs:7");
    assert_eq!(
        implementation.location.href.as_deref(),
        Some("https://github.com/exampleorg/ex-api/blob/abc1234/src/rules.rs#L7")
    );
    assert_eq!(page.verifications.len(), 2);
    assert!(!page.verifications[0].outside_implementation_module);
    assert!(page.verifications[1].outside_implementation_module);
    assert_eq!(
        page.code_scan.as_ref().unwrap().commit.as_deref(),
        Some("abc1234")
    );
    let requirement = requirement_page(&corpus, "req_child");
    assert_eq!(
        requirement.produced_rules[0].evidence[0].href.as_deref(),
        Some("https://github.com/exampleorg/ex-api/blob/abc1234/src/UseCase.php#L59-L69")
    );
    let references = requirement
        .threads
        .iter()
        .flat_map(|thread| &thread.messages)
        .flat_map(|message| &message.refs)
        .collect::<Vec<_>>();
    assert!(!references.is_empty());
    assert!(references.iter().all(|reference| reference
        .href
        .as_deref()
        .is_some_and(|href| href.contains("/blob/abc1234/"))));
}

/// A build given no report must leave `code_scan` unset, so the page can say
/// nothing was scanned instead of reporting an absent binding.
#[test]
fn a_corpus_built_without_a_report_records_no_code_scan() {
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));

    let corpus = build_corpus_with_coverage(&fixture_state(), &resolver, None);
    let page = rule_page(&corpus, "rule_001");

    assert!(page.code_scan.is_none());
    assert!(page.implementations.is_empty());
    assert!(page.verifications.is_empty());
}

#[test]
fn gone_bindings_are_not_presented_as_current_code_sites() {
    let mut gone_rule = binding("src/rules.rs", 7, "decide_rule", None);
    gone_rule.anchor_state = AnchorState::Gone;
    let mut gone_verification = binding(
        "tests/rules.rs",
        12,
        "rule_holds_for_examples",
        Some("examples"),
    );
    gone_verification.anchor_state = AnchorState::Gone;
    let report = CoverageScan {
        report: CoverageReport::new(
            Some("abc1234".to_string()),
            2,
            Vec::new(),
            vec![gone_rule, gone_verification],
            Vec::new(),
        ),
        scanned_files: Vec::new(),
    };
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));

    let corpus = build_corpus_with_coverage(&fixture_state(), &resolver, Some(&report));
    let page = rule_page(&corpus, "rule_001");

    assert!(page.implementations.is_empty());
    assert!(page.verifications.is_empty());
}

#[test]
fn an_uncommitted_scan_is_recorded_without_a_commit() {
    let report = CoverageScan {
        report: CoverageReport::new(
            None,
            1,
            Vec::new(),
            vec![binding("src/rules.rs", 7, "decide_rule", None)],
            Vec::new(),
        ),
        scanned_files: vec![ScannedFile {
            file_path: "src/rules.rs".into(),
            content: "one\ntwo\nthree\nfour\nfive\nsix\nfn decide_rule() {}\n".to_string(),
        }],
    };
    let resolver = LinkResolver::new(Some("git@github.com:exampleorg/ex-api.git"));

    let corpus = build_corpus_with_coverage(&fixture_state(), &resolver, Some(&report));
    let page = rule_page(&corpus, "rule_001");

    let scan = page.code_scan.as_ref().unwrap();
    assert!(scan.commit.is_none());
    let implementation = &page.implementations[0];
    assert!(implementation.location.href.is_none());
    assert_eq!(
        implementation.location.snippet.as_ref().unwrap().content,
        "fn decide_rule() {}"
    );
}

#[test]
fn absolute_scan_paths_are_made_relative_to_the_canonical_repository() {
    let relative = repository_relative_path(
        camino::Utf8Path::new("/work/repo/src/rules.rs"),
        camino::Utf8Path::new("."),
        Some(camino::Utf8Path::new("/work/repo")),
    );

    assert_eq!(relative, Utf8PathBuf::from("src/rules.rs"));
}
