//! Markers and attributes inside string and comment contexts.
//!
//! The comment-line gate (`rule_prov_annot_014`) runs through these: a
//! directive binds only on a line that starts as a comment, so a marker
//! trailing code never binds, and string contents never look like comments.

use camino::Utf8Path;
use provenance_scanner::{scan_file, Language};

#[test]
fn ignores_comment_markers_inside_same_line_string_literals() {
    for marker in ["@provenance", "@statesman"] {
        let source = format!(r#"const FIXTURE: &str = "// {marker} rule: string_only";"#);
        let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, &source);

        assert_eq!(scan.annotations, [] as [provenance_scanner::AnnotationLocation; 0]);
        assert_eq!(scan.warnings, [] as [provenance_scanner::ParseWarning; 0]);
    }
}

#[test]
fn a_marker_trailing_code_on_the_same_line_does_not_bind() {
    for (language, path, source) in [
        (
            Language::Rust,
            "fixture.rs",
            "let fixture = \"text\"; // @provenance rule: trailing\nfn real_rule() {}",
        ),
        (
            Language::Python,
            "fixture.py",
            "print('\"')  # @provenance rule: trailing\ndef real_rule(): pass",
        ),
        (
            Language::Go,
            "fixture.go",
            "value := `\"` // @provenance rule: trailing\nfunc realRule() {}",
        ),
    ] {
        let scan = scan_file(Utf8Path::new(path), language, source);

        assert!(scan.annotations.is_empty(), "bound {language:?} marker");
        assert!(scan.warnings.is_empty(), "warned on {language:?} marker");
    }
}

#[test]
fn skips_quoted_and_trailing_markers_but_keeps_a_comment_line_marker() {
    let scan = scan_file(
        Utf8Path::new("fixture.rs"),
        Language::Rust,
        "let fixture = \"@provenance rule: string_only\"; // @provenance rule: trailing\n\
         // @provenance rule: real\n\
         fn real_rule() {}",
    );

    assert_eq!(scan.annotations.len(), 1);
    assert_eq!(scan.annotations[0].annotation.rule, "real");
}

#[test]
fn ignores_attributes_inside_obvious_multiline_string_literals() {
    let source = r##"
const FIXTURE: &str = r#"
#[rule("string_rule")]
fn fixture_rule() {}

#[verifies("string_rule", examples)]
fn fixture_verification() {}
"#;

#[rule("real_rule")]
fn real_rule() {}
"##;
    let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, source);

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "real_rule");
}

#[test]
fn raw_string_hashes_keep_nested_fixture_quotes_from_ending_the_context() {
    let source = r####"
fn builds_fixture() {
    let fixture = r###"
const INNER: &str = r#"
#[rule("string_rule")]
fn fixture_rule() {}
"#;
"###;
}

#[rule("real_rule")]
fn real_rule() {}
"####;
    let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, source);

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "real_rule");
}

#[test]
fn a_double_quote_character_literal_does_not_hide_a_later_comment_marker() {
    let source = "let quote = '\"';\n// @provenance rule: real\nfn real_rule() {}";
    let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, source);

    assert_eq!(scan.annotations.len(), 1);
    assert_eq!(scan.annotations[0].annotation.rule, "real");
}

#[test]
fn ignores_a_marker_inside_a_same_line_raw_string() {
    let source = r##"let fixture = r#"" @provenance rule: string_only "#;"##;
    let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, source);

    assert_eq!(scan.annotations, [] as [provenance_scanner::AnnotationLocation; 0]);
    assert_eq!(scan.warnings, [] as [provenance_scanner::ParseWarning; 0]);
}

#[test]
fn scans_a_marker_on_its_own_line_after_a_multiline_raw_string_closes() {
    let source = r##"let fixture = r#"
fixture body
"#;
// @provenance rule: real
fn real_rule() {}
"##;
    let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, source);

    assert_eq!(scan.annotations.len(), 1);
    assert_eq!(scan.annotations[0].annotation.rule, "real");
}

#[test]
fn ignores_a_quoted_fixture_marker_shown_inside_a_rust_comment() {
    let source = r#"// const FIXTURE: &str = "// @provenance rule: string_only";"#;
    let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, source);

    assert_eq!(scan.annotations, [] as [provenance_scanner::AnnotationLocation; 0]);
    assert_eq!(scan.warnings, [] as [provenance_scanner::ParseWarning; 0]);
}

#[test]
fn a_rust_lifetime_does_not_hide_a_later_multiline_string_context() {
    let source = r#"
fn fixture<'a>() { let source = "it's embedded
#[rule("string_rule")]
";
}

#[rule("real_rule")]
fn real_rule() {}
"#;
    let scan = scan_file(Utf8Path::new("fixture.rs"), Language::Rust, source);

    assert_eq!(scan.bindings.len(), 1);
    assert_eq!(scan.bindings[0].rule_id, "real_rule");
}

#[test]
fn a_single_quoted_double_quote_does_not_hide_a_later_python_comment_marker() {
    let source = "print('\"')\n# @provenance rule: real\ndef real_rule(): pass";
    let scan = scan_file(Utf8Path::new("fixture.py"), Language::Python, source);

    assert_eq!(scan.annotations.len(), 1);
    assert_eq!(scan.annotations[0].annotation.rule, "real");
}

#[test]
fn a_double_quote_inside_a_go_raw_string_does_not_hide_a_later_marker() {
    let source = "value := `\"`\n// @provenance rule: real\nfunc realRule() {}";
    let scan = scan_file(Utf8Path::new("fixture.go"), Language::Go, source);

    assert_eq!(scan.annotations.len(), 1);
    assert_eq!(scan.annotations[0].annotation.rule, "real");
}

#[test]
fn ignores_a_marker_inside_a_same_line_backtick_string() {
    let source = "value := `// @provenance rule: string_only`";
    let scan = scan_file(Utf8Path::new("fixture.go"), Language::Go, source);

    assert_eq!(scan.annotations, [] as [provenance_scanner::AnnotationLocation; 0]);
    assert_eq!(scan.warnings, [] as [provenance_scanner::ParseWarning; 0]);
}
