//! Public scanner regressions for JavaScript and TypeScript literal handling.
use camino::Utf8Path;
use provenance_scanner::{scan_file, FileScan, Language};

fn scan(path: &str, source: &str) -> FileScan {
    scan_file(
        Utf8Path::new(path),
        Language::from_extension(Utf8Path::new(path).extension().unwrap()).unwrap(),
        source,
    )
}

#[test]
fn regex_quotes_classes_and_escapes_do_not_hide_annotations() {
    for extension in ["ts", "js", "tsx", "jsx"] {
        for expression in [
            r"/`/",
            r#"/"`/"#,
            r"/'`/",
            r"/[/]`/",
            r"/[\]`/]/",
            r"/\/(?:`)/gi",
            r"/`\/`/",
        ] {
            let source = format!("const pattern = {expression};\n// @provenance rule: rule_regex\nfunction f() {{}}\n");
            let report = scan(&format!("input.{extension}"), &source);
            assert_eq!(
                report.annotations.len(),
                1,
                "{extension}: {expression}: {report:?}"
            );
            assert_eq!(report.warnings.len(), 0, "{report:?}");
        }
    }
}

#[test]
fn division_preserves_annotations() {
    for source in [
        "const n = a / b / c;",
        "const n = a / b;",
        "const n = 12 / 2;",
        "const n = f() / 2;",
        "const n = values[0] / 2;",
        "const n = a++ / 2;",
        "const n = a / `text`;",
        "const n = 12 / 2 / 3;",
        "const n = f() / 2 / 3;",
        "const n = values[0] / 2 / 3;",
        "const n = a++ / 2 / 3;",
        "const n = a\n / b / c;",
        "const n = a /\n b / c;",
        "a = b / c; // note",
        "x = y /2/ 1",
        "a / b / c",
    ] {
        let source = format!("{source}\n// @provenance rule: rule_context\nfunction g() {{}}\n");
        let report = scan("context.ts", &source);
        assert_eq!(report.annotations.len(), 1, "{source}: {report:?}");
        assert_eq!(report.warnings.len(), 0, "{source}: {report:?}");
    }
}

#[test]
fn regex_quotes_do_not_turn_template_contents_into_annotations() {
    for literal in [r#"/"/"#, r"/'/", r"/[/]/"] {
        let source = format!(
            "const pattern = {literal}; const text = `\n// @provenance rule: rule_fake\n`;\n// @provenance rule: rule_real\nfunction f() {{}}\n"
        );
        let report = scan("template.js", &source);
        assert_eq!(report.annotations.len(), 1, "{report:?}");
        assert_eq!(
            report.annotations[0].annotation.rule, "rule_real",
            "{report:?}"
        );
        assert_eq!(report.warnings.len(), 0, "{report:?}");
    }
}

#[test]
fn regex_contents_are_not_helper_bindings_or_block_comments() {
    let report = scan(
        "helpers.ts",
        "const pattern = /rule(\"rule_fake\", value)[/*]`/;\nrule(\"rule_real\", value);\n// @provenance rule: rule_annotation\nfunction f() {}\n",
    );
    assert_eq!(report.annotations.len(), 1, "{report:?}");
    assert_eq!(report.bindings.len(), 1, "{report:?}");
    assert_eq!(report.bindings[0].rule_id, "rule_real", "{report:?}");
    assert_eq!(report.warnings.len(), 0, "{report:?}");
}

#[test]
fn expression_context_allows_regex() {
    for source in [
        "function f() { return /`/; }",
        "function f() { throw /`/; }",
        "const n = (/`/);",
        "const n = [/`/];",
        "const n = true ? /`/ : /`/;",
        "const n =\n /`/;",
    ] {
        let source = format!("{source}\n// @provenance rule: rule_context\nfunction g() {{}}\n");
        let report = scan("context.ts", &source);
        assert_eq!(report.annotations.len(), 1, "{source}: {report:?}");
        assert!(report.warnings.is_empty(), "{report:?}");
    }
}

// Characterization examples preserve existing comment and language behavior.
#[test]
fn jsx_inline_snapshot_preserves_annotation() {
    let report = scan("snapshot.tsx", "expect(render(<b>x</b>)).toMatchInlineSnapshot(`\n    <b>x</b>\n  `);\n// @provenance rule: rule_after\nfunction after() {}\n");
    assert_eq!(report.annotations.len(), 1, "{report:?}");
    assert_eq!(report.annotations[0].annotation.rule, "rule_after");
    assert!(report.warnings.is_empty(), "{report:?}");
}

#[test]
fn jsx_text_and_closing_tags_have_no_warning() {
    let report = scan(
        "view.tsx",
        "  </div>\n<p>Don't</p>\n// @provenance rule: rule_after\nfunction after() {}\n",
    );
    assert_eq!(report.annotations.len(), 1, "{report:?}");
    assert!(report.warnings.is_empty(), "{report:?}");
}

#[test]
fn incomplete_literals_preserve_old_annotation_visibility() {
    for (literal, found) in [
        ("/`", false),
        ("/[`/", false),
        ("/missing", true),
        ("\"quote", true),
        ("'quote", true),
        ("`template", false),
    ] {
        let source = format!("const ok = 1;\nconst broken = {literal}\n// @provenance rule: rule_after\nfunction after() {{}}\n");
        let report = scan("broken.ts", &source);
        assert_eq!(
            report.annotations.len(),
            usize::from(found),
            "{literal}: {report:?}"
        );
        if literal.contains('`') {
            assert_eq!(report.warnings.len(), 1, "{literal}: {report:?}");
            assert_eq!(report.warnings[0].line, 2);
            assert_eq!(
                report.warnings[0].message,
                "unterminated template literal; coverage scan can be incomplete"
            );
        } else {
            assert!(report.warnings.is_empty(), "{literal}: {report:?}");
        }
    }
}

#[test]
fn rust_and_python_keep_literal_and_comment_behavior() {
    for (path, source) in [
        (
            "input.rs",
            "let text = r#\"/`/\"#;\n// @provenance rule: rule_after\nfn after() {}\n",
        ),
        (
            "input.py",
            "text = \"/`/\"\n# @provenance rule: rule_after\ndef after(): pass\n",
        ),
    ] {
        let report = scan(path, source);
        assert_eq!(report.annotations.len(), 1, "{report:?}");
        assert_eq!(report.annotations[0].annotation.rule, "rule_after");
        assert!(report.bindings.is_empty(), "{report:?}");
        assert!(report.warnings.is_empty(), "{report:?}");
    }
}

#[test]
fn shebang_is_not_a_literal() {
    for shebang in ["#!/node", "#!/usr/bin/env node", "#!/`node"] {
        let report = scan(
            "script.js",
            &format!("{shebang}\n// @provenance rule: rule_after\nfunction after() {{}}\n"),
        );
        assert_eq!(report.annotations.len(), 1, "{report:?}");
        assert!(report.warnings.is_empty(), "{report:?}");
    }
}
