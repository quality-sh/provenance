use serde_json::json;
use std::{io::Write as _, path::Path};

/// Creates a proposed Resolution through the public CLI.
pub fn create_resolution(root: &Path, owner: Option<&str>) {
    let output = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "requirements",
            "create",
            "--repo",
            root.to_str().unwrap(),
            "--id",
            "req_reject",
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
            "resolutions",
            "create",
            "--repo",
            root.to_str().unwrap(),
            "--stdin",
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
                "id":"res_reject", "title":"Keep the result",
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
