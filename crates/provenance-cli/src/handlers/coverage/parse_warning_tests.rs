//! What the parser noticed while reading the source reaches the report.
//!
//! The scan collected these all along and then dropped them, so a repository
//! still on the legacy Statesman marker was never told to move off it.

use super::coverage_scan;
use camino::Utf8PathBuf;

/// A tree holding one file of the given source.
fn tree(source: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("payroll.rs"), source).unwrap();
    dir
}

fn scan(source: &str) -> provenance_core::coverage::CoverageScan {
    let dir = tree(source);
    let path = Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    coverage_scan(&path, &path, "default", false).unwrap()
}

#[test]
fn a_legacy_statesman_block_warns_that_the_marker_is_deprecated() {
    let report = scan("// @statesman rule: rule_overtime\nfn pays_overtime() {}\n");

    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0].message.contains("legacy marker"));
    assert_eq!(
        report.warnings[0]
            .file_path
            .as_ref()
            .unwrap()
            .file_name()
            .unwrap(),
        "payroll.rs"
    );
    assert!(report.warnings[0].line.is_some());
}

#[test]
fn a_malformed_directive_surfaces_too() {
    let report = scan("// @provenance rule: rule_overtime\n// @provenance nonsense\nfn f() {}\n");

    assert_eq!(report.warnings.len(), 1);
    assert!(report.warnings[0]
        .message
        .contains("malformed directive: expected `key: value`"));
}

/// A parse warning is about the file, not about a rule: a directive too
/// malformed to read never says which rule it meant.
#[test]
fn a_parse_warning_names_no_rule() {
    let report = scan("// @statesman rule: rule_overtime\nfn pays_overtime() {}\n");

    assert_eq!(report.warnings[0].rule_id, "");
}

/// The graph is not consulted for any of this, so the warnings do not wait
/// on `--validate-rules`.
#[test]
fn parse_warnings_do_not_need_rule_validation() {
    let report = scan("// @provenance confidence: 4.2\n// @provenance rule: rule_overtime\n");

    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.message.contains("confidence")));
}

/// A file the parser had nothing to say about contributes no warnings.
#[test]
fn a_clean_file_warns_about_nothing() {
    let report = scan("// @provenance rule: rule_overtime\nfn pays_overtime() {}\n");

    assert_eq!(
        report.warnings,
        [] as [provenance_scanner::ValidationWarning; 0]
    );
}
