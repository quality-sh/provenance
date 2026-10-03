#![cfg(unix)]

use provenance_macros::verifies;
use serde_json::Value;
use std::{
    fs::Permissions,
    io::{BufRead, BufReader},
    os::unix::fs::{MetadataExt, PermissionsExt},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let layout = provenance_store::layout::ProvenanceLayout::new(dir.path().to_str().unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    let manifest = provenance_core::Manifest::default_with_scope(
        provenance_core::ScopeId::new("default").unwrap(),
        provenance_core::RepoPathPrefix::new("."),
    );
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    dir
}

fn start(root: &std::path::Path, repository_id: &str, scope: &str) -> (Child, Value) {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args([
            "review",
            "--repo",
            root.to_str().unwrap(),
            "--repository-id",
            repository_id,
            "--scope",
            scope,
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        BufReader::new(stdout).read_line(&mut line).unwrap();
        let _ = send.send(line);
    });
    let line = receive.recv_timeout(Duration::from_secs(15)).unwrap();
    (child, serde_json::from_str(&line).unwrap())
}

fn stop(child: &mut Child) {
    assert!(Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap()
        .success());
    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "review host did not stop");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn runtime_file_contains_the_host_location_and_is_removed_on_shutdown() {
    let repo = repository();
    let runtime = repo
        .path()
        .join(".provenance/cache/review-hosts/default.json");
    let (mut child, config) = start(repo.path(), "A", "default");

    let published: Value = serde_json::from_slice(&std::fs::read(&runtime).unwrap()).unwrap();
    assert_eq!(published["hosts"][0]["endpoint"], config["endpoint"]);
    assert_eq!(published["hosts"][0]["repositoryId"], "A");
    assert_eq!(published["hosts"][0]["scope"], "default");
    assert_eq!(
        published["hosts"][0]["instanceNonce"],
        config["instanceNonce"]
    );
    assert_eq!(
        std::fs::metadata(runtime.parent().unwrap()).unwrap().mode() & 0o077,
        0
    );
    assert_eq!(std::fs::metadata(&runtime).unwrap().mode() & 0o077, 0);

    stop(&mut child);
    assert!(!runtime.exists());
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn public_identity_matches_the_runtime() {
    let repo = repository();
    let (mut child, config) = start(repo.path(), "A", "default");
    let identity: Value = serde_json::from_str(
        &ureq::get(&format!(
            "{}/review-host-identity",
            config["endpoint"].as_str().unwrap()
        ))
        .call()
        .unwrap()
        .into_string()
        .unwrap(),
    )
    .unwrap();

    assert_eq!(identity["repositoryId"], "A");
    assert_eq!(identity["scope"], "default");
    assert_eq!(identity["instanceNonce"], config["instanceNonce"]);
    stop(&mut child);
}

#[test]
#[verifies("rule_review_link_opens_repository_host_only", examples)]
fn publication_replaces_permissive_modes_with_owner_only_modes() {
    let repo = repository();
    let directory = repo.path().join(".provenance/cache/review-hosts");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::set_permissions(&directory, Permissions::from_mode(0o777)).unwrap();
    let runtime = directory.join("default.json");
    std::fs::write(&runtime, b"{}").unwrap();
    std::fs::set_permissions(&runtime, Permissions::from_mode(0o666)).unwrap();

    let (mut child, _) = start(repo.path(), "A", "default");
    assert_eq!(std::fs::metadata(&directory).unwrap().mode() & 0o077, 0);
    assert_eq!(std::fs::metadata(&runtime).unwrap().mode() & 0o077, 0);
    stop(&mut child);
}
