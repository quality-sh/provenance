use camino::Utf8Path;
use provenance_scanner::{scan_file, CoverageLevel, Language, Verification};

#[test]
fn scans_python_decorator_annotation_on_decorated_function() {
    let scan = scan_file(
        Utf8Path::new("payroll.py"),
        Language::Python,
        r"# @provenance coverage: partial
# @provenance rule: SCHADS-PAY-001
@cached
def pays_overtime():
    pass
",
    );

    assert_eq!(scan.annotations.len(), 1);
    assert_eq!(
        scan.annotations[0].annotation.coverage,
        CoverageLevel::Partial
    );
    assert_eq!(
        scan.annotations[0].function_name.as_deref(),
        Some("pays_overtime")
    );
}

#[test]
fn scans_go_receiver_method_after_block_comment() {
    let scan = scan_file(
        Utf8Path::new("payroll.go"),
        Language::Go,
        r"/*
 * @provenance rule: SCHADS-PAY-002
 */
func (s *Service) PaysOvertime() bool { return true }
",
    );

    assert_eq!(scan.annotations.len(), 1);
    assert_eq!(
        scan.annotations[0].function_name.as_deref(),
        Some("PaysOvertime")
    );
}

#[test]
fn reports_malformed_directive_location() {
    let scan = scan_file(
        Utf8Path::new("payroll.rs"),
        Language::Rust,
        "// @provenance rule SCHADS-PAY-001\nfn pays_overtime() {}\n",
    );

    assert_eq!(scan.annotations.len(), 0);
    assert_eq!(scan.warnings[0].line, 1);
    assert!(scan.warnings[0].message.contains("malformed directive"));
}

#[test]
fn scans_javascript_rule_call_bound_to_const() {
    let scan = scan_file(
        Utf8Path::new("rules.js"),
        Language::JavaScript,
        r#"export const paysOvertime = rule("rule_overtime", (hours) => hours > 38);"#,
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "rule_overtime");
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("paysOvertime"));
    assert_eq!(scan.bindings[0].verification, None);
}

#[test]
fn scans_javascript_verifies_call_in_named_function() {
    let scan = scan_file(
        Utf8Path::new("rules.test.js"),
        Language::JavaScript,
        r#"function overtimeExamples() {
  const hours = 39;
  verifies("rule_overtime", "examples");
}"#,
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(
        scan.bindings[0].item_name.as_deref(),
        Some("overtimeExamples")
    );
    assert_eq!(scan.bindings[0].verification, Some(Verification::Examples));
}

#[test]
fn scans_typescript_sdk_rule_and_verifies_helpers() {
    let scan = scan_file(
        Utf8Path::new("rules.ts"),
        Language::TypeScript,
        r#"import { rule, verifies } from "@quality-sh/provenance/rules";

export const paysOvertime = rule("rule_overtime", (hours: number) => hours > 38);
const overtimeProperty = () => {
  verifies("rule_overtime", "property");
};"#,
    );

    assert_eq!(scan.bindings.len(), 2);
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("paysOvertime"));
    assert_eq!(scan.bindings[0].verification, None);
    assert_eq!(
        scan.bindings[1].item_name.as_deref(),
        Some("overtimeProperty")
    );
    assert_eq!(scan.bindings[1].verification, Some(Verification::Property));
}

#[test]
fn scans_python_rule_decorator_above_function() {
    let scan = scan_file(
        Utf8Path::new("rules.py"),
        Language::Python,
        "@rule('rule_overtime')\ndef pays_overtime(hours):\n    return hours > 38",
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "rule_overtime");
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("pays_overtime"));
    assert_eq!(scan.bindings[0].verification, None);
}

#[test]
fn scans_go_rule_wrapper_bound_to_variable() {
    let scan = scan_file(
        Utf8Path::new("rules.go"),
        Language::Go,
        r#"var paysOvertime = rule("rule_overtime", func(hours int) bool { return hours > 38 })"#,
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "rule_overtime");
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("paysOvertime"));
    assert_eq!(scan.bindings[0].verification, None);
}

#[test]
fn scans_java_static_helper_bound_to_field() {
    let scan = scan_file(
        Utf8Path::new("PayrollRules.java"),
        Language::Java,
        r#"private static final IntPredicate PAYS_OVERTIME =
    ProvenanceRules.rule("rule_overtime", hours -> hours > 38);"#,
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "rule_overtime");
    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("PAYS_OVERTIME"));
    assert_eq!(scan.bindings[0].verification, None);
}

#[test]
fn ignores_call_shaped_text_in_comments() {
    for (language, source) in [
        (Language::JavaScript, r#"// rule("rule_not_bound", fn)"#),
        (
            Language::TypeScript,
            "/*\nverifies(\"rule_not_bound\", examples)\n*/",
        ),
        (Language::Go, r#"// rule("rule_not_bound", fn)"#),
        (Language::Java, r#"// Rules.rule("rule_not_bound", fn)"#),
    ] {
        let scan = scan_file(Utf8Path::new("comments.txt"), language, source);

        assert!(scan.bindings.is_empty(), "parsed {language:?} comment");
    }
}

#[test]
fn ignores_call_shaped_text_in_strings_and_trailing_comments() {
    let scan = scan_file(
        Utf8Path::new("fixtures.ts"),
        Language::TypeScript,
        r#"const fixture = 'verifies("rule_not_bound", "examples")';
const value = 1; // rule("rule_not_bound", implementation)"#,
    );

    assert!(scan.bindings.is_empty());
}

#[test]
fn scans_calls_before_trailing_comments() {
    let scan = scan_file(
        Utf8Path::new("rules.js"),
        Language::JavaScript,
        r#"const paysOvertime = rule("rule_overtime", implementation); // binding"#,
    );

    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("paysOvertime"));
}

#[test]
fn python_ordinary_rule_method_call_is_not_a_decorator_binding() {
    let scan = scan_file(
        Utf8Path::new("registry.py"),
        Language::Python,
        "registry.rule('rule_not_bound')\ndef unrelated():\n    pass",
    );

    assert!(scan.bindings.is_empty());
}

#[test]
fn typed_const_and_go_var_bindings_use_variable_names() {
    for (language, source, expected) in [
        (
            Language::TypeScript,
            r#"const paysOvertime: RuleFn = rule("rule_overtime", implementation);"#,
            "paysOvertime",
        ),
        (
            Language::Go,
            r#"var paysOvertime func(int) bool = rule("rule_overtime", implementation)"#,
            "paysOvertime",
        ),
    ] {
        let scan = scan_file(Utf8Path::new("typed.txt"), language, source);

        assert_eq!(scan.bindings[0].item_name.as_deref(), Some(expected));
    }
}

#[test]
fn verifies_call_uses_exported_async_function_name() {
    let scan = scan_file(
        Utf8Path::new("rules.test.ts"),
        Language::TypeScript,
        r#"export async function overtimeProperty() {
  verifies("rule_overtime", "property");
}"#,
    );

    assert_eq!(
        scan.bindings[0].item_name.as_deref(),
        Some("overtimeProperty")
    );
}

#[test]
fn scans_binding_after_unicode_source_text() {
    let scan = scan_file(
        Utf8Path::new("rules.ts"),
        Language::TypeScript,
        r#"const réglage = 1; const paysOvertime = rule("rule_overtime", implementation);"#,
    );

    assert_eq!(scan.bindings[0].rule_id, "rule_overtime");
}

#[test]
fn ignores_annotations_inside_string_literals() {
    let scan = scan_file(
        Utf8Path::new("fixture.ts"),
        Language::TypeScript,
        r#"const fixture = "@provenance rule: rule_not_bound";"#,
    );

    assert!(scan.annotations.is_empty());
}

#[test]
fn ignores_bindings_inside_multiline_strings() {
    for (language, source) in [
        (
            Language::TypeScript,
            "const fixture = `\nconst fake = rule(\"rule_not_bound\", fn);\n`;",
        ),
        (
            Language::Go,
            "var fixture = `\nvar fake = rule(\"rule_not_bound\", fn)\n`",
        ),
        (
            Language::Python,
            "fixture = \"\"\"\n@rule(\"rule_not_bound\")\ndef fake(): pass\n\"\"\"",
        ),
        (
            Language::Java,
            "String fixture = \"\"\"\nvar fake = rule(\"rule_not_bound\", fn);\n\"\"\";",
        ),
        (
            Language::Rust,
            "const FIXTURE: &str = r#\"\n#[rule(\"rule_not_bound\")]\nfn fake() {}\n\"#;",
        ),
    ] {
        let scan = scan_file(Utf8Path::new("fixture.txt"), language, source);

        assert!(scan.bindings.is_empty(), "parsed {language:?} string");
    }
}

#[test]
fn scans_binding_after_closed_block_comment() {
    let scan = scan_file(
        Utf8Path::new("rules.js"),
        Language::JavaScript,
        r#"/* decision */ const paysOvertime = rule("rule_overtime", implementation);"#,
    );

    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("paysOvertime"));
}

#[test]
fn unassigned_rule_call_does_not_borrow_an_unrelated_assignment() {
    let scan = scan_file(
        Utf8Path::new("rules.go"),
        Language::Go,
        "var unrelated = 1\n\nreturn rule(\"rule_overtime\", implementation)",
    );

    assert_eq!(scan.bindings[0].item_name, None);
}

#[test]
fn finds_verification_function_beyond_twelve_lines() {
    let padding = "\n".repeat(13);
    let source = format!(
        "function overtimeExamples() {{{padding}verifies(\"rule_overtime\", \"examples\");\n}}"
    );
    let scan = scan_file(
        Utf8Path::new("rules.test.js"),
        Language::JavaScript,
        &source,
    );

    assert_eq!(
        scan.bindings[0].item_name.as_deref(),
        Some("overtimeExamples")
    );
}

#[test]
fn annotation_block_does_not_hide_following_binding() {
    let scan = scan_file(
        Utf8Path::new("rules.go"),
        Language::Go,
        "/*\n * @provenance rule: rule_comment\n */\nvar native = rule(\"rule_native\", implementation)",
    );

    assert_eq!(scan.bindings[0].rule_id, "rule_native");
}

#[test]
fn multiline_delimiters_inside_regular_strings_do_not_hide_bindings() {
    for (language, source) in [
        (
            Language::JavaScript,
            "const punctuation = \"`\";\nconst native = rule(\"rule_native\", implementation);",
        ),
        (
            Language::Python,
            "punctuation = '\"\"\"'\n@rule(\"rule_native\")\ndef native(): pass",
        ),
    ] {
        let scan = scan_file(Utf8Path::new("rules.txt"), language, source);

        assert_eq!(scan.bindings[0].rule_id, "rule_native");
    }
}

#[test]
fn scans_binding_after_multiline_string_closes_on_same_line() {
    let scan = scan_file(
        Utf8Path::new("rules.ts"),
        Language::TypeScript,
        "const fixture = `\ntext\n`; const native = rule(\"rule_native\", implementation);",
    );

    assert_eq!(scan.bindings[0].rule_id, "rule_native");
}

#[test]
fn top_level_verifies_call_does_not_borrow_completed_function_name() {
    let scan = scan_file(
        Utf8Path::new("rules.test.js"),
        Language::JavaScript,
        "function completed() {\n}\nverifies(\"rule_overtime\", \"examples\");",
    );

    assert_eq!(scan.bindings[0].item_name, None);
}

#[test]
fn multiline_delimiters_in_comments_do_not_hide_later_bindings() {
    for (language, source) in [
        (
            Language::Python,
            "# use \"\"\" for docs\n@rule(\"rule_native\")\ndef native(): pass",
        ),
        (
            Language::JavaScript,
            "/*\n`\n*/\nconst native = rule(\"rule_native\", implementation);",
        ),
    ] {
        let scan = scan_file(Utf8Path::new("rules.txt"), language, source);

        assert_eq!(scan.bindings[0].rule_id, "rule_native");
    }
}

#[test]
fn rust_string_ending_in_r_does_not_open_a_phantom_raw_string() {
    let scan = scan_file(
        Utf8Path::new("threads.rs"),
        Language::Rust,
        "fn check() { assert_eq!(actor, \"reviewer\"); }\n\n#[verifies(\"rule_thread_siblings_archived\", examples)]\nfn posting_reuses_thread() {}\n",
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "rule_thread_siblings_archived");
}

#[test]
fn rust_rule_id_ending_in_r_still_binds() {
    let scan = scan_file(
        Utf8Path::new("rules.rs"),
        Language::Rust,
        "#[rule(\"rule_after\")]\nfn decide() {}\n",
    );

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "rule_after");
}

#[test]
fn rust_annotation_after_string_ending_in_r_still_scans() {
    let scan = scan_file(
        Utf8Path::new("payroll.rs"),
        Language::Rust,
        "fn setup() { let name = \"walker\"; }\n// @provenance rule: rule_real\nfn decide() {}\n",
    );

    assert_eq!(scan.annotations.len(), 1);
}

#[test]
fn go_short_variable_declaration_names_the_binding() {
    let scan = scan_file(
        Utf8Path::new("rules.go"),
        Language::Go,
        "paysOvertime := rule(\"rule_overtime\", func(h int) bool { return h > 38 })\n",
    );

    assert_eq!(scan.bindings[0].item_name.as_deref(), Some("paysOvertime"));
}

#[test]
fn template_literal_after_inline_block_comment_hides_its_contents() {
    let scan = scan_file(
        Utf8Path::new("rules.js"),
        Language::JavaScript,
        "/* note */ const s = `start\nconst fake = rule(\"rule_not_bound\", fn);\n`;\n",
    );

    assert!(scan.bindings.is_empty());
}
