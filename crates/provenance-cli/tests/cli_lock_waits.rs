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
    initialize(repo);
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
        .env(
            "PROVENANCE_TEST_STE100_ASSET_URL",
            "http://127.0.0.1:9/issue.pdf",
        )
        .args([
            "init",
            "--path",
            repo.to_str().unwrap(),
            "--scope",
            "default",
            "--path-prefix",
            ".",
            "--disposition-actor-id",
            "reviewer",
        ])
        .assert()
        .success()
        .stderr(predicates::str::contains("timed out"));
    assert!(repo.join(".provenance/state/manifest.json").is_file());
}

fn initialize(repo: &str) {
    provenance()
        .env("PROVENANCE_TEST_STE100_ASSET_URL", "off")
        .args([
            "init",
            "--path",
            repo,
            "--scope",
            "default",
            "--path-prefix",
            ".",
            "--disposition-actor-id",
            "reviewer",
        ])
        .assert()
        .success();
}

#[test]
fn a_held_cache_close_lock_refuses_a_read_within_the_command_limit() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    initialize(repo);
    let layout = provenance_store::layout::ProvenanceLayout::new(repo);
    let _held = hold(
        layout
            .cache_db_path()
            .with_extension("db.close.lock")
            .as_std_path(),
    );

    provenance()
        .args(["requirements", "list", "--repo", repo, "--format", "json"])
        .assert()
        .code(1);
}

#[test]
fn a_held_topic_shard_lock_refuses_the_update_and_keeps_the_title() {
    use serde_json::json;
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().to_str().unwrap();
    initialize(repo);
    for (collection, body) in [
        (
            "requirements",
            json!({"id":"req_guard", "statement":"The system stores records."}),
        ),
        (
            "topics",
            json!({"id":"topic_guard", "requirement_id":"req_guard", "title":"Guard"}),
        ),
    ] {
        provenance()
            .args([collection, "create", "--repo", repo, "--stdin"])
            .write_stdin(body.to_string())
            .assert()
            .success();
    }
    let before = read_topic(repo);
    let layout = provenance_store::layout::ProvenanceLayout::new(repo);
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let path = provenance_store::shards::topics_path(&layout, &scope);
    let lock_path = layout.state_shard_lock_path(&path).unwrap();
    let held = hold(lock_path.as_std_path());

    let refused = provenance()
        .args([
            "topics",
            "topic_guard",
            "update",
            "--repo",
            repo,
            "--format",
            "json",
            "--if-match",
            before["data"]["edit"]["etag"].as_str().unwrap(),
            "--title",
            "Changed",
        ])
        .assert()
        .code(1);
    let refusal: serde_json::Value = serde_json::from_slice(&refused.get_output().stderr).unwrap();
    assert_eq!(refusal["error"]["kind"], "write_failed");
    drop(held);
    let after = read_topic(repo);
    assert_eq!(after["data"]["title"], "Guard");
    assert_eq!(
        after["data"]["edit"]["etag"],
        before["data"]["edit"]["etag"]
    );
}

fn read_topic(repo: &str) -> serde_json::Value {
    let read = provenance()
        .args([
            "topics",
            "topic_guard",
            "get",
            "--repo",
            repo,
            "--format",
            "json",
        ])
        .assert()
        .success();
    serde_json::from_slice(&read.get_output().stdout).unwrap()
}
