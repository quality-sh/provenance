//! Implementation aids: hold operating-system locks through the public CLI to
//! reproduce waits that have no deadline. No Rule approves a wait duration.

use assert_cmd::Command;
use fs2::FileExt;
use std::{fs::OpenOptions, path::Path, time::Duration};

fn provenance() -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin!("provenance"));
    command.timeout(Duration::from_secs(15));
    command
}

fn hold(path: &Path) -> std::fs::File {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .unwrap();
    file.lock_exclusive().unwrap();
    file
}

#[test]
fn a_held_publication_lock_refuses_a_read_within_the_command_limit() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    provenance()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", "off")
        .args(["init", "--path", repo, "--scope", "default", "--path-prefix", ".",
            "--disposition-actor-id", "reviewer"])
        .assert()
        .success();
    let layout = provenance_store::layout::ProvenanceLayout::new(repo);
    let _held = hold(layout.publication_lock_path().as_std_path());

    provenance()
        .args(["requirements", "list", "--repo", repo, "--format", "json"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("timed out"));
}

#[test]
fn a_held_asset_lock_does_not_stop_initialization() {
    let directory = tempfile::tempdir().unwrap();
    let assets = directory.path().join("assets");
    let _held = hold(&assets.join("issue-9.pdf.lock"));
    let repo = directory.path().join("repo");

    provenance()
        .env("PROVENANCE_STE100_ASSET_DIR", &assets)
        .env("PROVENANCE_TEST_STE100_ASSET_URL", "http://127.0.0.1:9/issue.pdf")
        .args(["init", "--path", repo.to_str().unwrap(), "--scope", "default", "--path-prefix", ".",
            "--disposition-actor-id", "reviewer"])
        .assert()
        .success()
        .stderr(predicates::str::contains("timed out"));
    assert!(repo.join(".provenance/state/manifest.json").is_file());
}
