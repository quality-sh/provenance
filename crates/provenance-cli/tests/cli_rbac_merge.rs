//! The merge-driver claim transport and the rbac merge closure, proved with
//! the real documented `git config merge.provenance-jsonl.driver` setup and
//! real `git merge` runs.

use assert_cmd::Command;
use std::path::Path;

fn edge(id: &str, edge_type: &str, from_type: &str, from: &str, to_type: &str, to: &str) -> String {
    format!(
        "{{\"schema_version\":1,\"scope_id\":\"default\",\"id\":\"{id}\",\
         \"edge_type\":\"{edge_type}\",\"from_type\":\"{from_type}\",\"from_id\":\"{from}\",\
         \"to_type\":\"{to_type}\",\"to_id\":\"{to}\"}}\n"
    )
}

fn valid_edge(id: &str, to: &str) -> String {
    edge(
        id,
        "references",
        "source",
        "source_policy",
        "requirement",
        to,
    )
}

const GRANTS: &str = r#"{
  "schema_version": 1,
  "scopes": [{"id": "default", "path_prefix": "."}],
  "disposition_actor_ids": [],
  "rbac": {"assignments": [
    {"actor_id": "merger", "identity_type": "human", "capabilities": ["edit"], "scopes": ["default"]}
  ]}
}"#;

fn git(repository: &Path, arguments: &[&str]) {
    let output = Command::new("git")
        .args(arguments)
        .current_dir(repository)
        .output()
        .expect("run git");
    assert!(
        output.status.success(),
        "git {arguments:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Configures the documented driver, optionally carrying a literal
/// `--actor-id <id>` argument in the clone-local command template.
fn configure_driver(repository: &Path, actor_id: Option<&str>) {
    let binary = assert_cmd::cargo::cargo_bin("provenance")
        .to_str()
        .unwrap()
        .replace('\\', "/");
    let mut driver = format!("'{binary}' merge-jsonl %O %A %B --output %A --path %P");
    if let Some(actor_id) = actor_id {
        driver = format!("{driver} --actor-id {actor_id}");
    }
    git(
        repository,
        &["config", "merge.provenance-jsonl.driver", &driver],
    );
}

/// A git repository on `main` with the rbac manifest, the attributes line,
/// the committed edges shard, and a `theirs` branch adding `edge_theirs`.
fn merge_fixture(actor_id: Option<&str>) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repository = directory.path();
    git(repository, &["init", "--initial-branch", "main"]);
    git(repository, &["config", "user.email", "fixture@example.com"]);
    git(repository, &["config", "user.name", "Fixture"]);
    let state = repository.join(".provenance/state");
    std::fs::create_dir_all(state.join("edges")).unwrap();
    std::fs::write(
        repository.join(".gitattributes"),
        ".provenance/state/**/*.jsonl merge=provenance-jsonl\n",
    )
    .unwrap();
    std::fs::write(manifest_path(repository), GRANTS).unwrap();
    let base_edge = valid_edge("edge_base", "req_base");
    std::fs::write(state.join("edges/edges-00.jsonl"), &base_edge).unwrap();
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", "base"]);
    git(repository, &["checkout", "-b", "theirs"]);
    std::fs::write(
        state.join("edges/edges-00.jsonl"),
        format!("{base_edge}{}", valid_edge("edge_theirs", "req_theirs")),
    )
    .unwrap();
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", "theirs"]);
    git(repository, &["checkout", "main"]);
    std::fs::write(
        state.join("edges/edges-00.jsonl"),
        format!("{base_edge}{}", valid_edge("edge_ours", "req_ours")),
    )
    .unwrap();
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", "ours"]);
    configure_driver(repository, actor_id);
    directory
}

fn manifest_path(repository: &Path) -> std::path::PathBuf {
    repository.join(".provenance/state/manifest.json")
}

fn shard(repository: &Path) -> std::path::PathBuf {
    repository.join(".provenance/state/edges/edges-00.jsonl")
}

fn run_merge(repository: &Path) -> std::process::Output {
    std::process::Command::new("git")
        .args(["merge", "theirs", "-m", "merge"])
        .current_dir(repository)
        .env_remove("GIT_DIR")
        .output()
        .expect("run git merge")
}

#[test]
fn the_driver_command_without_a_claim_fails_and_leaves_the_shard_unmerged() {
    let directory = merge_fixture(None);
    let repository = directory.path();

    let output = run_merge(repository);
    assert!(
        !output.status.success(),
        "git must not commit a merge the driver refused on an rbac repository"
    );
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(repository)
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(
        status.contains("edges-00.jsonl"),
        "the shard must stay unmerged: {status}"
    );
    let merged = std::fs::read_to_string(shard(repository)).unwrap();
    assert!(!merged.contains("edge_theirs"), "{merged}");
}

#[test]
fn the_driver_command_with_an_unauthorized_id_fails_the_same_way() {
    let directory = merge_fixture(Some("intruder"));
    let repository = directory.path();

    let output = run_merge(repository);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not hold capability edit on scope default")
            || stderr.contains("unmerged"),
        "refusal should name the authorization failure: {stderr}"
    );
}

#[test]
fn the_driver_command_with_a_granted_id_merges() {
    let directory = merge_fixture(Some("merger"));
    let repository = directory.path();

    let output = run_merge(repository);
    assert!(
        output.status.success(),
        "a granted merge should commit: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let merged = std::fs::read_to_string(shard(repository)).unwrap();
    assert!(merged.contains("edge_theirs"), "{merged}");
    assert!(merged.contains("edge_ours"), "{merged}");
}

#[test]
fn a_manifestless_repository_merges_exactly_as_before() {
    let directory = merge_fixture(Some("intruder"));
    let repository = directory.path();
    // No manifest at all: no rbac regime, the driver is not consulted for
    // authorization even though the command carries an id.
    std::fs::remove_file(manifest_path(repository)).unwrap();
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", "drop manifest"]);

    let output = run_merge(repository);
    assert!(output.status.success(), "pre-existing posture stands");
}

#[test]
fn an_rbac_repository_refuses_a_pathless_merge_output() {
    let directory = tempfile::tempdir().unwrap();
    let sides = directory.path().join("sides");
    std::fs::create_dir_all(&sides).unwrap();
    std::fs::create_dir_all(directory.path().join(".provenance/state")).unwrap();
    std::fs::write(
        directory.path().join(".provenance/state/manifest.json"),
        GRANTS,
    )
    .unwrap();

    // Hand-run from inside the repository root: no --path, so no family.
    std::fs::write(
        sides.join("base.jsonl"),
        valid_edge("edge_base", "req_base"),
    )
    .unwrap();
    std::fs::write(
        sides.join("ours.jsonl"),
        format!(
            "{}{}",
            valid_edge("edge_base", "req_base"),
            valid_edge("edge_ours", "req_ours")
        ),
    )
    .unwrap();
    std::fs::write(
        sides.join("theirs.jsonl"),
        format!(
            "{}{}",
            valid_edge("edge_base", "req_base"),
            valid_edge("edge_theirs", "req_theirs")
        ),
    )
    .unwrap();

    Command::cargo_bin("provenance")
        .unwrap()
        .current_dir(directory.path())
        .args([
            "merge-jsonl",
            sides.join("base.jsonl").to_str().unwrap(),
            sides.join("ours.jsonl").to_str().unwrap(),
            sides.join("theirs.jsonl").to_str().unwrap(),
            "--output",
            sides.join("merged.jsonl").to_str().unwrap(),
            "--actor-id",
            "merger",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "rbac: a merge on an rbac-managed repository requires --path",
        ));
    assert!(
        !sides.join("merged.jsonl").exists(),
        "nothing may be written"
    );
}
