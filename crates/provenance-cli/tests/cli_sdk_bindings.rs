use assert_cmd::Command;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use serde_json::{json, Value};
use std::process::Command as ProcessCommand;

fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

#[test]
fn repeated_verification_runs_reuse_one_durable_binding() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    provenance()
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    std::fs::create_dir_all(directory.path().join("tests")).unwrap();
    std::fs::write(
        directory.path().join("tests/share-links.test.ts"),
        "export const checkExpiry = true;\n",
    )
    .unwrap();
    git(repo, &["init"]);
    git(repo, &["add", "tests/share-links.test.ts"]);
    git(
        repo,
        &[
            "-c",
            "user.name=Provenance Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "add verification",
        ],
    );
    let head = git(repo, &["rev-parse", "HEAD"]);
    let spec = json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "share-links",
        "declared_by": "spec://typescript",
        "requirements": [{"key": "sharing", "statement": "Users can share"}],
        "rules": [{
            "key": "expiry", "requirement": "sharing", "statement": "Links expire"
        }]
    });
    let applied = invoke(repo, "apply", &spec);
    let rule_id = applied["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "rule")
        .unwrap()["id"]
        .as_str()
        .unwrap();
    #[cfg(unix)]
    let verification_file = {
        let alias = directory.path().join("repo-alias");
        std::os::unix::fs::symlink(".", &alias).unwrap();
        alias.join("tests/share-links.test.ts")
    };
    #[cfg(not(unix))]
    let verification_file = directory.path().join("tests/share-links.test.ts");
    let request = json!({
        "rule": rule_id, "key": "share-link-expiry", "method": "examples",
        "declared_by": "ci://node-test", "file": verification_file,
        "symbol": "share links expire"
    });

    let first = invoke(repo, "begin-verification", &request);
    let second = invoke(repo, "begin-verification", &request);

    assert_ne!(first["id"], second["id"]);
    assert_eq!(first["binding_id"], second["binding_id"]);
    assert_eq!(first["commit"], head);
    assert_eq!(first["file"], "tests/share-links.test.ts");
    assert_eq!(first["symbol"], "share links expire");
    let output = provenance()
        .args([
            "sdk",
            "verification-bindings",
            "--repo",
            repo,
            "--scope",
            "default",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let bindings: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(bindings.as_array().unwrap().len(), 1);
    assert_eq!(bindings[0]["rule_id"], rule_id);
}

fn git(repo: &str, args: &[&str]) -> String {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn invoke(repo: &str, command: &str, input: &Value) -> Value {
    let output = provenance()
        .args([
            "sdk", command, "--repo", repo, "--scope", "default", "--format", "json",
        ])
        .write_stdin(serde_json::to_vec(input).unwrap())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
