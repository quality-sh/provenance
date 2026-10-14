//! Runs the client harness against the same CLI binary as the public Rust tests.
use std::{path::Path, process::Command};

#[test]
// Implementation aid: gives the Bun real-client tests a commit-pinned remote Cargo run.
fn codex_adapter_delivers_to_the_real_client() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("npm")
        .current_dir(&workspace)
        .args([
            "exec",
            "--yes",
            "--package=bun@1.3.14",
            "--package=@openai/codex@0.159.1",
            "--",
            "bun",
            "test",
            "tools/agent-notifications/test/codex.test.ts",
        ])
        .env(
            "PROVENANCE_TEST_BIN",
            assert_cmd::cargo::cargo_bin("provenance"),
        )
        .output()
        .expect("npm can start the pinned Bun and Codex test tools");
    assert!(
        output.status.success(),
        "Bun client tests failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
