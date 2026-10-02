#![cfg(unix)]

use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
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

fn start(root: &std::path::Path) -> (Child, Value) {
    let mut child = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args([
            "review",
            "--repo",
            root.to_str().unwrap(),
            "--repository-id",
            "A",
            "--scope",
            "default",
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

#[test]
fn runtime_file_contains_the_host_location_and_is_removed_on_shutdown() {
    let repo = repository();
    let runtime = repo
        .path()
        .join(".provenance/cache/review-hosts/default.json");
    let (mut child, config) = start(repo.path());

    let published: Value = serde_json::from_slice(&std::fs::read(&runtime).unwrap()).unwrap();
    assert_eq!(published["endpoint"], config["endpoint"]);
    assert_eq!(published["repositoryId"], "A");
    assert_eq!(published["scope"], "default");
    assert!(published.get("bearer").is_none());

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
    assert!(!runtime.exists());
}
