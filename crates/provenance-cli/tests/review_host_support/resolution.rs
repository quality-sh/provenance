use serde_json::json;
use std::{io::Write as _, path::Path};

/// Creates a proposed Resolution through the public CLI.
pub fn create_resolution(root: &Path, owner: Option<&str>) {
    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "req_reject",
            "create",
            "--type",
            "requirement",
            "--repo",
            root.to_str().unwrap(),
            "--statement",
            "The system stores each result.",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut child = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "res_reject",
            "create",
            "--type",
            "resolution",
            "--stdin",
            "--repo",
            root.to_str().unwrap(),
            "--format",
            "json",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(
            json!({
                "title":"Keep the result",
                "position":"The system stores each result.",
                "rationale":"The result must remain available.",
                "status":"proposed", "made_by":owner,
                "requirement_ids":["req_reject"]
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
