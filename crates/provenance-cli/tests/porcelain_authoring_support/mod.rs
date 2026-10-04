use assert_cmd::Command;
use provenance_transport::{fixture::local_host::LocalHostFixture, local_host::LocalHostIdentity};
use serde_json::Value;

pub fn provenance() -> Command {
    Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
}

pub fn initialized_repo() -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_string_lossy().into_owned();
    provenance()
        .args([
            "init",
            "--path",
            &repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
        ])
        .assert()
        .success();
    (directory, repo)
}

pub fn json_output(arguments: &[&str]) -> Value {
    let output = provenance().args(arguments).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[allow(dead_code)]
pub fn json_stdin_output(arguments: &[&str], input: &Value) -> Value {
    let output = provenance()
        .args(arguments)
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

#[allow(dead_code)]
pub fn allow_reviewer(repo: &str) {
    let layout = provenance_store::layout::ProvenanceLayout::new(repo);
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

#[allow(dead_code)]
pub fn local_host(repo: &str) -> LocalHostFixture {
    LocalHostFixture::start(std::path::Path::new(repo), "default", "local")
}

#[allow(dead_code)]
pub fn local_host_with_identity(
    repo: &str,
    response: impl FnOnce(LocalHostIdentity) -> Value,
) -> LocalHostFixture {
    LocalHostFixture::start_with_identity(std::path::Path::new(repo), "default", "local", response)
}
