use assert_cmd::Command;
use serde_json::json;
use tempfile::TempDir;

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

#[test]
fn output_flag_writes_the_report_to_a_file() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("envelope.json");
    let output_path = dir.path().join("report.md");
    let envelope = json!({
        "schema_version": 1,
        "repository": "quality-sh/provenance",
        "scope": "default",
        "base_commit": "96373e74a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6",
        "head_commit": "52ccec3fb1c2d3e4f5a6b7c8d9e0f1a2b3c4d5e6",
        "scan": {
            "completeness": "complete",
            "baseline": "compatible",
            "files_scanned": 214
        },
        "policy": { "mode": "warning", "result": "success" },
        "graph_changes": [],
        "findings": [],
        "verification_runs": []
    });
    std::fs::write(&input, serde_json::to_vec_pretty(&envelope).unwrap()).unwrap();

    let output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["report", "render", "--input"])
        .arg(&input)
        .args(["--output"])
        .arg(&output_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let written = std::fs::read_to_string(&output_path).unwrap();
    assert!(written.starts_with("# Provenance report"));
    assert!(written.ends_with("no language model writes this report.\n"));
}
