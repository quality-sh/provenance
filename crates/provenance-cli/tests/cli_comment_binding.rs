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

/// A site of a scan report: its line, its Rule id, its verification method
/// and its item name. No method is an implementation site.
type Site = (u64, String, Option<String>, Option<String>);

/// The sites of a scan report, in line order.
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

/// The Rule id and the item name of each site in a scan report, in line order.
fn item_names(report: &Value) -> Vec<(String, Option<String>)> {
    sites(report)
        .into_iter()
        .map(|(_, rule, _, name)| (rule, name))
        .collect()
}

/// The line, the Rule id and the verification method of each site in a scan
/// report, in line order.
fn places(report: &Value) -> Vec<(u64, String, Option<String>)> {
    sites(report)
        .into_iter()
        .map(|(line, rule, method, _)| (line, rule, method))
        .collect()
}

fn place(line: u64, rule: &str, method: Option<&str>) -> (u64, String, Option<String>) {
    (line, rule.to_owned(), method.map(str::to_owned))
}

fn named(rule: &str, name: &str) -> (String, Option<String>) {
    (rule.to_owned(), Some(name.to_owned()))
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

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_rust_verifies_attribute_below_a_rule_comment_is_a_second_site() {
    let report = scan("src/pay.rs", RUST_VERIFIES_BELOW_A_COMMENT);

    let expected = [
        place(1, "rule_paid_hours", None),
        place(2, "rule_overtime", Some("examples")),
    ];
    assert_eq!(places(&report), expected, "{report}");
}

const RUST_VERIFIES_BELOW_A_COMMENT: &str = "// @provenance rule: rule_paid_hours\n\
                                             #[verifies(\"rule_overtime\", examples)]\n\
                                             fn overtime_is_paid() {}\n";

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_rust_rule_attribute_below_a_rule_comment_is_a_second_site() {
    let report = scan("src/pay.rs", RUST_RULE_BELOW_A_COMMENT);

    let expected = [
        place(1, "rule_paid_hours", None),
        place(2, "rule_overtime", None),
    ];
    assert_eq!(places(&report), expected, "{report}");
}

const RUST_RULE_BELOW_A_COMMENT: &str = "// @provenance rule: rule_paid_hours\n\
                                         #[rule(\"rule_overtime\")]\n\
                                         pub fn pays_overtime() {}\n";

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_rust_attribute_and_the_rule_comment_above_it_name_one_function() {
    let verifies = scan("src/pay.rs", RUST_VERIFIES_BELOW_A_COMMENT);
    let rule = scan("src/pay.rs", RUST_RULE_BELOW_A_COMMENT);

    let paid = |name| vec![named("rule_paid_hours", name), named("rule_overtime", name)];
    assert_eq!(
        item_names(&verifies),
        paid("overtime_is_paid"),
        "{verifies}"
    );
    assert_eq!(item_names(&rule), paid("pays_overtime"), "{rule}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_python_hash_line_below_a_rule_comment_is_part_of_the_comment() {
    let source = "# @provenance rule: rule_overtime\n\
                  # @provenance verification: examples\n\
                  def test_overtime_is_paid():\n    assert True\n";
    let report = scan("tests/test_pay.py", source);

    let expected = [place(1, "rule_overtime", Some("examples"))];
    assert_eq!(places(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_javascript_private_field_below_a_rule_comment_ends_the_comment() {
    let source = "export class Counter {\n\
                  \x20 // @provenance rule: rule_count_starts_at_zero\n\
                  \x20 #count = 0;\n\
                  \x20 // @provenance rule: rule_step_is_one\n\
                  \x20 #step = 1;\n\
                  }\n";
    let report = scan("src/counter.js", source);

    let expected = [
        place(2, "rule_count_starts_at_zero", None),
        place(4, "rule_step_is_one", None),
    ];
    assert_eq!(places(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_javascript_generator_method_below_a_rule_comment_ends_the_comment() {
    let source = "export class Items {\n\
                  \x20 // @provenance rule: rule_items\n\
                  \x20 *items() { yield 1; }\n\
                  \x20 // @provenance rule: rule_size\n\
                  \x20 size() { return 1; }\n\
                  }\n";
    let report = scan("src/items.js", source);

    let expected = [place(2, "rule_items", None), place(4, "rule_size", None)];
    assert_eq!(places(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_rust_dereference_below_a_rule_comment_ends_the_comment() {
    let source = "pub fn bump(x: &mut u32) {\n\
                  \x20   // @provenance rule: rule_bump\n\
                  \x20   *x += 1;\n\
                  \x20   // @provenance rule: rule_log\n\
                  \x20   log(*x);\n\
                  }\n";
    let report = scan("src/bump.rs", source);

    let expected = [place(2, "rule_bump", None), place(4, "rule_log", None)];
    assert_eq!(places(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_python_star_argument_below_a_rule_comment_ends_the_comment() {
    let source = "def call(\n\
                  \x20   # @provenance rule: rule_args\n\
                  \x20   *args,\n\
                  \x20   # @provenance rule: rule_kwargs\n\
                  \x20   **kwargs,\n\
                  ):\n\
                  \x20   pass\n";
    let report = scan("src/call.py", source);

    let expected = [place(2, "rule_args", None), place(4, "rule_kwargs", None)];
    assert_eq!(places(&report), expected, "{report}");
}

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_python_floor_division_below_a_rule_comment_ends_the_comment() {
    let source = "half = (\n\
                  \x20   total\n\
                  \x20   # @provenance rule: rule_half\n\
                  \x20   // 2\n\
                  \x20   # @provenance rule: rule_whole\n\
                  )\n";
    let report = scan("src/half.py", source);

    let expected = [place(3, "rule_half", None), place(5, "rule_whole", None)];
    assert_eq!(places(&report), expected, "{report}");
}

/// A Rule comment in a documentation block above a generator method, and a
/// line comment below the method.
const JAVASCRIPT_DOC_BLOCK: &str = "export class Items {\n\
                                    \x20 /**\n\
                                    \x20  * Gives each item.\n\
                                    \x20  * @provenance rule: rule_items\n\
                                    \x20  */\n\
                                    \x20 *items() { yield 1; }\n\
                                    \x20 // @provenance rule: rule_size\n\
                                    \x20 size() { return 1; }\n\
                                    }\n";

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_rule_comment_in_a_doc_block_ends_at_the_close_of_the_block() {
    let report = scan("src/items.js", JAVASCRIPT_DOC_BLOCK);

    let expected = [place(4, "rule_items", None), place(7, "rule_size", None)];
    assert_eq!(places(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_rule_comment_in_a_doc_block_names_the_method_below_the_block() {
    let report = scan("src/items.js", JAVASCRIPT_DOC_BLOCK);

    let expected = [named("rule_items", "items"), named("rule_size", "size")];
    assert_eq!(item_names(&report), expected, "{report}");
}

/// Block comments that close on the line of the code below them.
const JAVASCRIPT_BLOCK_BEFORE_CODE: &str =
    "/* @provenance rule: rule_total */ export const total = 0;\n\
                                            /**\n\
                                            \x20* @provenance rule: rule_count\n\
                                            \x20*/ export const count = 0;\n\
                                            // @provenance rule: rule_step\n\
                                            export const step = 1;\n";

#[test]
#[verifies("rule_comment_ends_above_the_first_code_line", examples)]
fn a_block_comment_that_code_follows_on_its_last_line_ends_on_that_line() {
    let report = scan("src/count.js", JAVASCRIPT_BLOCK_BEFORE_CODE);

    let expected = [
        place(1, "rule_total", None),
        place(3, "rule_count", None),
        place(5, "rule_step", None),
    ];
    assert_eq!(places(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_block_comment_that_code_follows_names_the_declaration_after_it() {
    let report = scan("src/count.js", JAVASCRIPT_BLOCK_BEFORE_CODE);

    let expected = [
        named("rule_total", "total"),
        named("rule_count", "count"),
        named("rule_step", "step"),
    ];
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_prov_annot_014", examples)]
fn a_marker_after_a_rust_dereference_on_its_line_does_not_bind() {
    let source = "pub fn bump(x: &mut u32) {\n\
                  \x20   *x += 1; // @provenance rule: rule_bump\n\
                  }\n";
    let report = scan("src/bump.rs", source);

    assert_eq!(places(&report), [], "{report}");
}

#[test]
// Implementation aid: no Rule states where a Python string starts. In Python,
// `//` is floor division and starts no comment, so the string after it is open.
fn a_python_string_after_floor_division_holds_no_binding() {
    let source = "half = total // 2; doc = \"\"\"\n\
                  @rule(\"rule_in_string\")\n\
                  def fake(): pass\n\
                  \"\"\"\n";
    let report = scan("src/half.py", source);

    assert_eq!(places(&report), [], "{report}");
}
