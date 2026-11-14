//! A `// @provenance rule:` comment connects production code to a Rule through
//! `check --bindings`, and a second comment for the same Rule is a finding that
//! names the Rule in a form that is true for each binding form.
use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;

fn scan(path: &str, source: &str) -> Value {
    let repo = tempfile::tempdir().unwrap();
    let root = repo.path().to_str().unwrap();
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "init",
            "--path",
            root,
            "--scope",
            "demo",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    let file = repo.path().join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, source).unwrap();
    let output = repo.path().join("report.json");
    Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "check",
            "--bindings",
            "--repo",
            root,
            "--path",
            root,
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
#[verifies("rule_prov_annot_012", examples)]
fn a_comment_connects_the_exported_const_below_it() {
    let source = "// @provenance rule: rule_overtime\n\
                  export const paysOvertime = (hours: number): boolean => hours > 38;\n";
    let report = scan("src/pay.ts", source);

    let sites = report["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|site| (site["rule_id"].as_str(), site["function_name"].as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        sites,
        [(Some("rule_overtime"), Some("paysOvertime"))],
        "{report}"
    );
}

#[test]
// Implementation aid: no Rule states the text of this finding. The text names
// the Rule by its id, so it is true for a comment, an attribute and a call.
fn a_second_comment_for_one_rule_names_the_rule_in_the_finding() {
    let source = "// @provenance rule: rule_overtime\n\
                  export const paysOvertime = (hours: number): boolean => hours > 38;\n\
                  \n\
                  // @provenance rule: rule_overtime\n\
                  export const paysLateOvertime = (hours: number): boolean => hours > 40;\n";
    let report = scan("src/pay.ts", source);

    let duplicates = report["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|warning| {
            warning["message"]
                .as_str()
                .unwrap()
                .contains("more than one")
        })
        .map(|warning| (warning["line"].as_u64(), warning["message"].as_str()))
        .collect::<Vec<_>>();
    assert_eq!(
        duplicates,
        [(
            Some(4),
            Some("more than one primary implementation binding was found for rule `rule_overtime`")
        )],
        "{report}"
    );
}
