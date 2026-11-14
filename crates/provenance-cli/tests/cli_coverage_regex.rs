//! Two-file command regression for GitHub issue #442.
use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;
use std::path::Path;

fn scan(files: &[(&str, &str)]) -> Value {
    let repo = tempfile::tempdir().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            repo.path().to_str().unwrap(),
            "--scope",
            "demo",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    for (name, source) in files {
        let path = repo.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, source).unwrap();
    }
    scan_command(repo.path())
}

fn scan_command(repo: &Path) -> Value {
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("report.json");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--bindings",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "demo",
            "--output",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    serde_json::from_slice(&std::fs::read(output).unwrap()).unwrap()
}

#[test]
#[verifies("rule_prov_annot_regex", examples)]
fn issue_442_finds_both_typescript_annotations() {
    let report = scan(&[
        (
            "src/a.ts",
            include_str!("fixtures/coverage_regex/src/a.ts.txt"),
        ),
        (
            "src/b.ts",
            include_str!("fixtures/coverage_regex/src/b.ts.txt"),
        ),
    ]);
    assert_eq!(report["total_annotations"], 2, "{report}");
    let warnings = report["warnings"].as_array().unwrap();
    // The regex literals give no parse warning.
    assert!(
        warnings.iter().all(|warning| warning["rule_id"] != ""),
        "{report}"
    );
    // The bindings check always validates Rules, which `coverage scan` did only with
    // --validate-rules: the Rule that the graph lacks is reported at each site.
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning["message"] == "unknown rule id `rule_demo_a`")
            .count(),
        2,
        "{report}"
    );
}
