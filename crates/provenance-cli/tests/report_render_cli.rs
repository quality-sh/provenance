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
fn json_format_writes_json_to_stdout_and_a_file() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("envelope.json");
    let output_path = dir.path().join("report.json");
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

    let stdout_output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["report", "render", "--input"])
        .arg(&input)
        .args(["--format", "json"])
        .output()
        .unwrap();

    assert!(
        stdout_output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&stdout_output.stderr)
    );
    let stdout_json: serde_json::Value = serde_json::from_slice(&stdout_output.stdout).unwrap();
    assert!(!stdout_output.stdout.starts_with(b"# Provenance report"));

    let file_output = Command::cargo_bin("provenance")
        .unwrap()
        .args(["report", "render", "--input"])
        .arg(&input)
        .args(["--format", "json"])
        .args(["--output"])
        .arg(&output_path)
        .output()
        .unwrap();

    assert!(
        file_output.status.success(),
        "render failed: {}",
        String::from_utf8_lossy(&file_output.stderr)
    );
    let written = std::fs::read(&output_path).unwrap();
    let file_json: serde_json::Value = serde_json::from_slice(&written).unwrap();
    assert!(!written.starts_with(b"# Provenance report"));
    assert_eq!(file_json, stdout_json);
}
