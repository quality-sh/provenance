//! A `// @provenance rule:` comment connects production code to a Rule through
//! `check --bindings`, and a second comment for the same Rule is a finding that
//! names the Rule in a form that is true for each binding form. The comment
//! ends above the first line that is not a comment line of its language.
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

/// The Rule id and the item name of each site in a scan report, in line order.
fn item_names(report: &Value) -> Vec<(String, Option<String>)> {
    let mut sites = ["annotations", "bindings"]
        .iter()
        .flat_map(|key| report[key].as_array().unwrap())
        .map(|site| {
            let name = site["function_name"]
                .as_str()
                .or_else(|| site["item_name"].as_str());
            (
                site["line"].as_u64().unwrap(),
                site["rule_id"].as_str().unwrap().to_owned(),
                name.map(str::to_owned),
            )
        })
        .collect::<Vec<_>>();
    sites.sort();
    sites
        .into_iter()
        .map(|(_, rule, name)| (rule, name))
        .collect()
}

/// The Rule id and the item name of each site in the names file `names`, in
/// line order. A `-` is no item name.
fn names_file(names: &str) -> Vec<(String, Option<String>)> {
    names
        .lines()
        .map(|line| {
            let (rule, name) = line.split_once(' ').unwrap();
            (rule.to_owned(), (name != "-").then(|| name.to_owned()))
        })
        .collect()
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_rust_binding_names_the_function_or_type_below_it() {
    let report = scan("src/pay.rs", include_str!("fixtures/names/rs.txt"));

    let expected = names_file(include_str!("fixtures/names/rs.names"));
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_python_binding_names_the_function_or_class_below_it() {
    let report = scan("src/pay.py", include_str!("fixtures/names/py.txt"));

    let expected = names_file(include_str!("fixtures/names/py.names"));
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_go_binding_names_the_function_or_type_below_it() {
    let report = scan("src/pay.go", include_str!("fixtures/names/go.txt"));

    let expected = names_file(include_str!("fixtures/names/go.names"));
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_java_binding_names_the_method_or_type_below_it() {
    let report = scan("src/Pay.java", include_str!("fixtures/names/java.txt"));

    let expected = names_file(include_str!("fixtures/names/java.names"));
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_typescript_binding_names_the_exported_declaration_below_it() {
    let report = scan("src/pay.ts", include_str!("fixtures/names/ts.txt"));

    let expected = names_file(include_str!("fixtures/names/ts.names"));
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_javascript_binding_names_the_exported_declaration_below_it() {
    let report = scan("src/pay.js", include_str!("fixtures/names/js.txt"));

    let expected = names_file(include_str!("fixtures/names/js.names"));
    assert_eq!(item_names(&report), expected, "{report}");
}

/// The line, the Rule id, the verification method and the item name of each
/// site in a scan report, in line order. No method is an implementation site.
type Site = (u64, String, Option<String>, Option<String>);

fn sites(report: &Value) -> Vec<Site> {
    let text = |site: &Value, key: &str| site[key].as_str().map(str::to_owned);
    let mut sites = ["annotations", "bindings"]
        .iter()
        .flat_map(|key| report[key].as_array().unwrap())
        .map(|site| {
            (
                site["line"].as_u64().unwrap(),
                text(site, "rule_id").unwrap(),
                text(site, "verification"),
                text(site, "function_name").or_else(|| text(site, "item_name")),
            )
        })
        .collect::<Vec<_>>();
    sites.sort();
    sites
}

fn site(line: u64, rule: &str, method: Option<&str>, name: Option<&str>) -> Site {
    (
        line,
        rule.to_owned(),
        method.map(str::to_owned),
        name.map(str::to_owned),
    )
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_rust_verifies_attribute_below_a_rule_comment_is_a_second_site() {
    let source = "// @provenance rule: rule_paid_hours\n\
                  #[verifies(\"rule_overtime\", examples)]\n\
                  fn overtime_is_paid() {}\n";
    let report = scan("src/pay.rs", source);

    let expected = [
        site(1, "rule_paid_hours", None, Some("overtime_is_paid")),
        site(
            2,
            "rule_overtime",
            Some("examples"),
            Some("overtime_is_paid"),
        ),
    ];
    assert_eq!(sites(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_rust_rule_attribute_below_a_rule_comment_is_a_second_site() {
    let source = "// @provenance rule: rule_paid_hours\n\
                  #[rule(\"rule_overtime\")]\n\
                  pub fn pays_overtime() {}\n";
    let report = scan("src/pay.rs", source);

    let expected = [
        site(1, "rule_paid_hours", None, Some("pays_overtime")),
        site(2, "rule_overtime", None, Some("pays_overtime")),
    ];
    assert_eq!(sites(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_python_hash_line_below_a_rule_comment_is_part_of_the_comment() {
    let source = "# @provenance rule: rule_overtime\n\
                  # @provenance verification: examples\n\
                  def test_overtime_is_paid():\n    assert True\n";
    let report = scan("tests/test_pay.py", source);

    let expected = [site(
        1,
        "rule_overtime",
        Some("examples"),
        Some("test_overtime_is_paid"),
    )];
    assert_eq!(sites(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_javascript_private_field_below_a_rule_comment_is_code() {
    let source = "export class Counter {\n\
                  \x20 // @provenance rule: rule_count_starts_at_zero\n\
                  \x20 #count = 0;\n\
                  \n\
                  \x20 increment() {\n\
                  \x20   this.#count += 1;\n\
                  \x20 }\n\
                  }\n";
    let report = scan("src/counter.js", source);

    let expected = [site(2, "rule_count_starts_at_zero", None, Some("count"))];
    assert_eq!(sites(&report), expected, "{report}");
}
