use assert_cmd::Command;

#[test]
fn unsupported_format_is_rejected_before_input_is_read() {
    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args([
            "report",
            "render",
            "--input",
            "does-not-exist.json",
            "--format",
            "table",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("invalid value 'table'") && stderr.contains("json, markdown"),
        "format error must list only supported values: {stderr}"
    );
}
