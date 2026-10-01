use assert_cmd::cargo::cargo_bin;
use predicates::prelude::*;
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
};

#[test]
fn review_startup_warns_when_no_reviewer_is_configured() {
    let repo = tempfile::tempdir().unwrap();
    let layout = provenance_store::layout::ProvenanceLayout::new(repo.path().to_str().unwrap());
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

    let mut child = Command::new(cargo_bin("provenance"))
        .args([
            "review",
            "--repo",
            repo.path().to_str().unwrap(),
            "--repository-id",
            "A",
            "--scope",
            "default",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut warning = String::new();
    BufReader::new(child.stderr.take().unwrap())
        .read_line(&mut warning)
        .unwrap();
    child.kill().unwrap();
    child.wait().unwrap();

    assert!(predicate::str::contains(format!(
        "Warning: No reviewer is configured. The review page will be read-only. Add a reviewer with `provenance init --path {} --disposition-actor-id <reviewer-id>`.",
        repo.path().display()
    ))
    .eval(&warning));
}
