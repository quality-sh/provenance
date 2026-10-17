//! Public-command regressions for #442; these examples have no graph Rule binding.
use assert_cmd::Command;
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
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "coverage",
            "scan",
            "--repo",
            repo.to_str().unwrap(),
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "demo",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

#[test]
fn issue_442_finds_both_typescript_annotations() {
    let report = scan(&[
        ("src/a.ts", include_str!("fixtures/coverage_regex/src/a.ts.txt")),
        ("src/b.ts", include_str!("fixtures/coverage_regex/src/b.ts.txt")),
    ]);
    assert_eq!(report["total_annotations"], 2, "{report}");
    assert_eq!(report["warnings"], serde_json::json!([]), "{report}");
}

#[test]
fn regex_quotes_classes_and_escapes_do_not_hide_annotations() {
    for extension in ["ts", "js", "tsx", "jsx"] {
        for expression in [
            r#"/`/"#,
            r#"/"`/"#,
            r#"/'`/"#,
            r#"/[/]`/"#,
            r#"/[\]`/]/"#,
            r#"/\/(?:`)/gi"#,
            r#"/`\/`/"#,
        ] {
            let source = format!("const pattern = {expression};\n// @provenance rule: rule_regex\nfunction f() {{}}\n");
            let report = scan(&[(&format!("src/input.{extension}"), &source)]);
            assert_eq!(
                report["total_annotations"], 1,
                "{extension}: {expression}: {report}"
            );
            assert_eq!(report["warnings"], serde_json::json!([]), "{report}");
        }
    }
}

#[test]
fn expression_context_distinguishes_regex_from_division() {
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
        "function f() { return /`/; }",
        "function f() { throw /`/; }",
        "const n = (/`/);",
        "const n = [/`/];",
        "const n = true ? /`/ : /`/;",
        "const n =\n /`/;",
    ] {
        let source = format!("{source}\n// @provenance rule: rule_context\nfunction g() {{}}\n");
        let report = scan(&[("src/context.ts", &source)]);
        assert_eq!(report["total_annotations"], 1, "{source}: {report}");
        assert_eq!(
            report["warnings"],
            serde_json::json!([]),
            "{source}: {report}"
        );
    }
}

#[test]
fn unterminated_literals_report_the_source_line() {
    for literal in ["/`", "/[`/", "`template", "\"quote", "'quote"] {
        let report = scan(&[(
            "src/broken.ts",
            &format!("const ok = 1;\nconst broken = {literal}\n// @provenance rule: rule_later\n"),
        )]);
        let warnings = report["warnings"].as_array().unwrap();
        assert_eq!(warnings.len(), 1, "{literal}: {report}");
        assert_eq!(warnings[0]["line"], 2, "{report}");
        assert!(
            warnings[0]["message"]
                .as_str()
                .unwrap()
                .contains("unterminated"),
            "{report}"
        );
        assert!(
            warnings[0]["file_path"]
                .as_str()
                .unwrap()
                .ends_with("src/broken.ts"),
            "{report}"
        );
    }
}

#[test]
fn regex_quotes_do_not_turn_template_contents_into_annotations() {
    for literal in [r#"/"/"#, r"/'/", r"/[/]/"] {
        let source = format!(
            "const pattern = {literal}; const text = `\n// @provenance rule: rule_fake\n`;\n// @provenance rule: rule_real\nfunction f() {{}}\n"
        );
        let report = scan(&[("src/template.js", &source)]);
        assert_eq!(report["total_annotations"], 1, "{report}");
        assert_eq!(report["annotations"][0]["rule_id"], "rule_real", "{report}");
        assert_eq!(report["warnings"], serde_json::json!([]), "{report}");
    }
}

#[test]
fn regex_contents_are_not_helper_bindings_or_block_comments() {
    let report = scan(&[(
        "src/helpers.ts",
        "const pattern = /rule(\"rule_fake\", value)[/*]`/;\nrule(\"rule_real\", value);\n// @provenance rule: rule_annotation\nfunction f() {}\n",
    )]);
    assert_eq!(report["total_annotations"], 1, "{report}");
    assert_eq!(report["bindings"].as_array().unwrap().len(), 1, "{report}");
    assert_eq!(report["bindings"][0]["rule_id"], "rule_real", "{report}");
    assert_eq!(report["warnings"], serde_json::json!([]), "{report}");
}
