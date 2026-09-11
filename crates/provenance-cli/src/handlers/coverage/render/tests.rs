//! The markdown render: which verification sites get marked as outside
//! the module that implements the rule, and how a warning with no location
//! is written.

use super::{render_coverage, OUTSIDE_IMPLEMENTATION_MODULE};
use crate::output::OutputFormat;
use camino::Utf8PathBuf;
use provenance_core::coverage::{
    AnchorState, AnnotationResult, BindingResult, CoverageReport, CoverageScan, ScannedFile,
    ValidationWarning,
};

fn binding(rule_id: &str, file_path: &str, verification: Option<&str>) -> BindingResult {
    BindingResult {
        rule_id: rule_id.to_string(),
        file_path: Utf8PathBuf::from(file_path),
        line: 12,
        item_name: None,
        verification: verification.map(ToOwned::to_owned),
        anchor: None,
        anchor_state: AnchorState::Unchanged,
        original_line: None,
        original_file_path: None,
    }
}

fn report(bindings: Vec<BindingResult>, warnings: Vec<ValidationWarning>) -> CoverageScan {
    CoverageScan {
        report: CoverageReport::new(None, 1, Vec::new(), bindings, warnings),
        scanned_files: Vec::new(),
    }
}

fn comment(rule_id: &str, file_path: &str, verification: Option<&str>) -> AnnotationResult {
    AnnotationResult {
        rule_id: rule_id.to_string(),
        file_path: file_path.into(),
        line: 4,
        function_name: Some("portable_site".to_string()),
        coverage: "full".to_string(),
        confidence: 1.0,
        verification: verification.map(ToOwned::to_owned),
        anchor: None,
        anchor_state: AnchorState::Unchanged,
        original_line: None,
        original_file_path: None,
    }
}

#[test]
fn json_output_keeps_scanned_source_content() {
    let mut report = report(Vec::new(), Vec::new());
    report.scanned_files = vec![ScannedFile {
        file_path: "src/rules.rs".into(),
        content: "fn rule() {}\n".to_string(),
    }];

    let json = render_coverage(OutputFormat::Json, &report).unwrap();

    assert!(json.contains("scanned_files"));
    assert!(json.contains("fn rule() {}"));
}

#[test]
fn markdown_reports_moved_and_gone_anchor_states() {
    let mut moved = binding("rule_moved", "src/rules.rs", None);
    moved.line = 18;
    moved.anchor_state = AnchorState::Moved;
    moved.original_line = Some(7);
    let mut gone = binding("rule_gone", "src/rules.rs", Some("examples"));
    gone.anchor_state = AnchorState::Gone;
    let report = report(vec![moved, gone], Vec::new());

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(markdown.contains("at `src/rules.rs`:18 (moved from line 7)"));
    assert!(markdown.contains("at `src/rules.rs`:12 (gone)"));
}

#[test]
fn markdown_reports_cross_file_moves_with_their_old_file() {
    let mut moved = binding("rule_moved", "src/relocated.rs", None);
    moved.line = 3;
    moved.anchor_state = AnchorState::Moved;
    moved.original_line = Some(7);
    moved.original_file_path = Some(Utf8PathBuf::from("src/rules.rs"));
    let report = report(vec![moved], Vec::new());

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(markdown.contains("at `src/relocated.rs`:3 (moved from src/rules.rs:7)"));
}

#[test]
fn markdown_reports_first_seen_sites_as_new() {
    let mut new = binding("rule_new", "src/rules.rs", Some("examples"));
    new.anchor_state = AnchorState::New;
    let report = report(vec![new], Vec::new());

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(markdown.contains("at `src/rules.rs`:12 (new)"));
}

#[test]
fn verification_site_in_another_file_is_marked_outside_the_implementation_module() {
    let report = report(
        vec![
            binding("rule_overtime", "src/payroll.rs", None),
            binding("rule_overtime", "tests/billing.rs", Some("examples")),
        ],
        Vec::new(),
    );

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(markdown.contains(
        "`rule_overtime` verified by examples at `tests/billing.rs`:12 (outside implementation module)"
    ));
}

#[test]
fn comment_sites_render_with_their_roles_and_implementation_module() {
    let mut report = report(Vec::new(), Vec::new());
    report.report.annotations = vec![
        comment("rule_overtime", "src/payroll.rs", None),
        comment("rule_overtime", "tests/billing.rs", Some("examples")),
    ];

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(markdown.contains("`rule_overtime` is implemented at `src/payroll.rs`:4"));
    assert!(markdown.contains("`rule_overtime` verified by examples at `tests/billing.rs`:4"));
    assert!(markdown.contains(OUTSIDE_IMPLEMENTATION_MODULE));
}

#[test]
fn verification_site_beside_the_rule_is_not_marked() {
    let report = report(
        vec![
            binding("rule_overtime", "src/payroll.rs", None),
            binding("rule_overtime", "src/payroll.rs", Some("exhaustion")),
        ],
        Vec::new(),
    );

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(!markdown.contains(OUTSIDE_IMPLEMENTATION_MODULE));
}

/// The `#[rule]` line itself is an implementation, not a site leaning on one.
#[test]
fn the_implementation_site_is_never_marked() {
    let report = report(
        vec![binding("rule_overtime", "src/payroll.rs", None)],
        Vec::new(),
    );

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(markdown.contains("`rule_overtime` is implemented at `src/payroll.rs`:12"));
    assert!(!markdown.contains(OUTSIDE_IMPLEMENTATION_MODULE));
}

/// Without a `#[rule]` site in the scanned tree there is no implementation
/// module to be outside of, so the report claims nothing either way.
#[test]
fn a_rule_with_no_scanned_implementation_leaves_its_sites_unmarked() {
    let report = report(
        vec![binding(
            "rule_overtime",
            "tests/billing.rs",
            Some("examples"),
        )],
        Vec::new(),
    );

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(!markdown.contains(OUTSIDE_IMPLEMENTATION_MODULE));
}

#[test]
fn a_warning_without_a_location_is_rendered_without_one() {
    let report = report(
        Vec::new(),
        vec![ValidationWarning {
            rule_id: "rule_overtime".to_string(),
            file_path: None,
            line: None,
            message: "active rule `rule_overtime` has no verification".to_string(),
            binding_finding: true,
        }],
    );

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(markdown
        .contains("- Warning `rule_overtime`: active rule `rule_overtime` has no verification"));
    assert!(!markdown.contains(":0"));
}

/// A parse warning is about the file, so there is no rule to name. Printing
/// an empty pair of backticks would read as a rule whose name went missing.
#[test]
fn a_warning_about_no_rule_is_rendered_without_an_empty_id() {
    let report = report(
        Vec::new(),
        vec![ValidationWarning {
            rule_id: String::new(),
            file_path: Some(Utf8PathBuf::from("src/payroll.rs")),
            line: Some(4),
            message: "legacy marker `@statesman` is deprecated".to_string(),
            binding_finding: false,
        }],
    );

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(markdown
        .contains("- Warning in `src/payroll.rs`:4: legacy marker `@statesman` is deprecated"));
    assert!(!markdown.contains("``"));
}

#[test]
fn a_warning_with_a_location_still_shows_it() {
    let report = report(
        Vec::new(),
        vec![ValidationWarning {
            rule_id: "UNKNOWN".to_string(),
            file_path: Some(Utf8PathBuf::from("src/payroll.rs")),
            line: Some(4),
            message: "unknown rule".to_string(),
            binding_finding: false,
        }],
    );

    let markdown = render_coverage(OutputFormat::Markdown, &report).unwrap();

    assert!(markdown.contains("- Warning `UNKNOWN` in `src/payroll.rs`:4: unknown rule"));
}
