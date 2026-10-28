//! What the parser noticed while reading the source reaches the coverage
//! report.
//!
//! The scan collected these all along and then dropped them, so a repository
//! still on the legacy Statesman marker was never told to move off it.

use serde_json::Value;

mod support {
    pub mod review_paths;
}
use support::review_paths::{init, provenance};

/// The coverage report for one source file, scanned without Rule
/// validation.
fn scan(source: &str) -> Value {
    let (directory, repo) = init();
    let code = directory.path().join("src");
    std::fs::create_dir_all(&code).unwrap();
    std::fs::write(code.join("payroll.rs"), source).unwrap();
    let output = provenance()
        .args([
            "coverage",
            "scan",
            "--repo",
            &repo,
            "--path",
            code.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn warnings(report: &Value) -> &Vec<Value> {
    report["warnings"].as_array().unwrap()
}

// Implementation aid: keeps parser warnings in the coverage report.
#[test]
fn a_legacy_statesman_block_warns_that_the_marker_is_deprecated() {
    let report = scan("// @statesman rule: rule_overtime\nfn pays_overtime() {}\n");

    let warnings = warnings(&report);
    assert_eq!(warnings.len(), 1, "{report}");
    assert!(
        warnings[0]["message"]
            .as_str()
            .unwrap()
            .contains("legacy marker"),
        "{report}"
    );
    assert!(
        warnings[0]["file_path"]
            .as_str()
            .unwrap()
            .ends_with("payroll.rs"),
        "{report}"
    );
    assert!(warnings[0]["line"].is_u64(), "{report}");
}

// Implementation aid: keeps malformed directive warnings in the report.
#[test]
fn a_malformed_directive_surfaces_too() {
    let report = scan("// @provenance rule: rule_overtime\n// @provenance nonsense\nfn f() {}\n");

    let warnings = warnings(&report);
    assert_eq!(warnings.len(), 1, "{report}");
    assert!(
        warnings[0]["message"]
            .as_str()
            .unwrap()
            .contains("malformed directive: expected `key: value`"),
        "{report}"
    );
}

/// A parse warning is about the file, not about a rule: a directive too
/// malformed to read never says which rule it meant.
// Implementation aid: keeps parse warnings free of a Rule id.
#[test]
fn a_parse_warning_names_no_rule() {
    let report = scan("// @statesman rule: rule_overtime\nfn pays_overtime() {}\n");

    assert_eq!(warnings(&report)[0]["rule_id"], "", "{report}");
}

/// The graph is not consulted for any of this, so the warnings do not wait
/// on `--validate-rules`.
// Implementation aid: keeps parse warnings independent of Rule validation.
#[test]
fn parse_warnings_do_not_need_rule_validation() {
    let report = scan("// @provenance confidence: 4.2\n// @provenance rule: rule_overtime\n");

    assert!(
        warnings(&report)
            .iter()
            .any(|warning| warning["message"].as_str().unwrap().contains("confidence")),
        "{report}"
    );
}

/// A file the parser had nothing to say about contributes no warnings.
// Implementation aid: keeps a clean file free of warnings.
#[test]
fn a_clean_file_warns_about_nothing() {
    let report = scan("// @provenance rule: rule_overtime\nfn pays_overtime() {}\n");

    assert!(warnings(&report).is_empty(), "{report}");
}
