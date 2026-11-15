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

fn named(pairs: &[(&str, Option<&str>)]) -> Vec<(String, Option<String>)> {
    pairs
        .iter()
        .map(|(rule, name)| ((*rule).to_owned(), name.map(str::to_owned)))
        .collect()
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_rust_binding_names_the_function_or_type_below_it() {
    let report = scan("src/pay.rs", include_str!("fixtures/names/rs.txt"));

    let expected = named(&[
        ("rule_struct", Some("Payroll")),
        ("rule_impl_block", None),
        ("rule_enum", Some("Shift")),
        ("rule_generic_struct", Some("Ledger")),
        ("rule_const_fn", Some("limit")),
        ("rule_async_fn", Some("pays_overtime")),
        ("rule_trait", Some("Rates")),
        ("rule_alias", Some("Hours")),
        ("rule_attribute_enum", Some("Kind")),
        ("rule_attribute_struct", Some("Entry")),
        ("rule_attribute_impl", None),
        ("rule_const_then_doc", None),
        ("rule_static", None),
        ("rule_mod", None),
        ("rule_long_derive", Some("Wide")),
        ("rule_attribute_long_derive", Some("WideEntry")),
        ("rule_raw_identifier", Some("match")),
        ("rule_block_comment", Some("hidden_after_block")),
        ("rule_inline_comment", Some("inline_after_comment")),
        ("rule_deref_statement", None),
        ("rule_nested_comment", Some("after_nested")),
        ("rule_end", None),
    ]);
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_python_binding_names_the_function_or_class_below_it() {
    let report = scan("src/pay.py", include_str!("fixtures/names/py.txt"));

    let expected = named(&[
        ("rule_class", Some("Payroll")),
        ("rule_method", Some("pays")),
        ("rule_decorated_async", Some("test_pays")),
        ("rule_decorated_class", Some("Ledger")),
        ("rule_assignment", None),
        ("rule_if_block", None),
        ("rule_long_decorator", Some("test_long")),
        ("rule_decorator", Some("pays_holiday")),
        ("rule_star_assignment", None),
    ]);
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_go_binding_names_the_function_or_type_below_it() {
    let report = scan("src/pay.go", include_str!("fixtures/names/go.txt"));

    let expected = named(&[
        ("rule_struct", Some("Payroll")),
        ("rule_method", Some("Pays")),
        ("rule_interface", Some("Rates")),
        ("rule_generic_func", Some("Max")),
        ("rule_type_group", None),
        ("rule_var", None),
    ]);
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_java_binding_names_the_method_or_type_below_it() {
    let report = scan("src/Pay.java", include_str!("fixtures/names/java.txt"));

    let expected = named(&[
        ("rule_final_class", Some("Payroll")),
        ("rule_field", None),
        ("rule_long_annotation", Some("cast")),
        ("rule_static_method", Some("paysOvertime")),
        ("rule_annotated_test", Some("paysOvertimeExamples")),
        ("rule_constructor", Some("Payroll")),
        ("rule_doc_inline", Some("pays")),
        ("rule_synchronized", None),
        ("rule_generic_call", None),
        ("rule_default_arrow", None),
        ("rule_default_label", None),
        ("rule_wrapped_generic", Some("max")),
        ("rule_annotation_type", Some("Audited")),
        ("rule_record", Some("Shift")),
        ("rule_enum", Some("Color")),
        ("rule_interface", Some("Shape")),
    ]);
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_typescript_binding_names_the_exported_declaration_below_it() {
    let report = scan("src/pay.ts", include_str!("fixtures/names/ts.txt"));

    let expected = named(&[
        ("rule_class", Some("Token")),
        ("rule_interface", Some("Shape")),
        ("rule_alias", Some("Hours")),
        ("rule_default_class", Some("Ledger")),
        ("rule_decorated_method", Some("total")),
        ("rule_arrow_property", None),
        ("rule_describe", None),
        ("rule_async_function", Some("paysOvertime")),
        ("rule_const_enum", Some("Shift")),
        ("rule_abstract_class", Some("Base")),
        ("rule_generator_method", Some("shifts")),
        ("rule_getter", Some("total")),
        ("rule_setter", Some("total")),
        ("rule_optional_method", None),
        ("rule_parameter_properties", Some("constructor")),
        ("rule_ternary", None),
        ("rule_call_then_block", None),
        ("rule_internal_doc", Some("hidden")),
        ("rule_export_decorated", Some("Sealed")),
        ("rule_default_decorated", Some("SealedDefault")),
    ]);
    assert_eq!(item_names(&report), expected, "{report}");
}

#[test]
#[verifies("rule_site_item_name_is_the_declaration_below", examples)]
fn a_javascript_binding_names_the_exported_declaration_below_it() {
    let report = scan("src/pay.js", include_str!("fixtures/names/js.txt"));

    let expected = named(&[
        ("rule_class", Some("Token")),
        ("rule_default_function", Some("pays")),
        ("rule_generator", Some("shifts")),
        ("rule_test_callback", None),
        ("rule_arrow_test", None),
        ("rule_anonymous_default", None),
        ("rule_generator_method", Some("hours")),
        ("rule_let", Some("first")),
        ("rule_var", Some("counter")),
    ]);
    assert_eq!(item_names(&report), expected, "{report}");
}
