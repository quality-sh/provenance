//! How `check --bindings` reads TypeScript production code: a comment connects
//! the code to a Rule, and a call to a function named `rule` does not.

use assert_cmd::Command;
use provenance_macros::verifies;
use serde_json::Value;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

/// Initializes a repository, writes `source` as `src/pay.ts`, and returns the
/// report that `check --bindings` writes for that directory.
fn scan_typescript(source: &str) -> Value {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path();
    provenance()
        .args(["init", "--path", repo.to_str().unwrap()])
        .args(["--scope", "default", "--path-prefix", "."])
        .assert()
        .success();
    std::fs::create_dir(repo.join("src")).unwrap();
    std::fs::write(repo.join("src/pay.ts"), source).unwrap();
    let report = repo.join("report.json");
    provenance()
        .args(["check", "--bindings", "--path", "src", "--scope", "default"])
        .arg("--output")
        .arg(&report)
        .current_dir(repo)
        .assert()
        .success();
    serde_json::from_str(&std::fs::read_to_string(&report).unwrap()).unwrap()
}

fn sites(report: &Value, key: &str, name: &str) -> Vec<(String, String)> {
    report[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|site| {
            (
                site["rule_id"].as_str().unwrap().to_owned(),
                site[name].as_str().unwrap_or_default().to_owned(),
            )
        })
        .collect()
}

#[test]
#[verifies("rule_prov_annot_012", examples)]
fn a_comment_connects_the_exported_const_below_it() {
    let report = scan_typescript(
        "// @provenance rule: rule_overtime\n\
         export const paysOvertime = (hours: number): boolean => hours > 38;\n",
    );

    assert_eq!(
        sites(&report, "annotations", "function_name"),
        [("rule_overtime".to_owned(), "paysOvertime".to_owned())],
        "{report}"
    );
}

#[test]
// Implementation aid: production TypeScript code connects to a Rule with a
// comment only, so a call to a function named `rule` is ordinary code. No Rule
// in the graph states this absence.
fn a_rule_call_is_not_an_implementation_binding() {
    let report = scan_typescript(
        "import { rule } from \"./rules.js\";\n\
         export const paysOvertime = rule(\"rule_overtime\", (hours: number) => hours > 38);\n",
    );

    assert!(sites(&report, "bindings", "item_name").is_empty(), "{report}");
    assert!(sites(&report, "annotations", "function_name").is_empty(), "{report}");
}
