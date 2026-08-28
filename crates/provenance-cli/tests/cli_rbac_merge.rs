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
fn the_merge_driver_refuses_a_malformed_rbac_section() {
    let directory = merge_fixture(Some("merger"));
    let repository = directory.path();
    // The same actor granted `default` twice: a malformed section, which
    // every reader must refuse before consulting any grant.
    std::fs::write(
        manifest_path(repository),
        r#"{
      "schema_version": 1,
      "scopes": [{"id": "default", "path_prefix": "."}],
      "disposition_actor_ids": [],
      "rbac": {"assignments": [
        {"actor_id": "merger", "capabilities": ["edit"], "scopes": ["default"]},
        {"actor_id": "merger", "capabilities": ["read"], "scopes": ["default"]}
      ]}
    }"#,
    )
    .unwrap();

    let output = run_merge(repository);
    assert!(
        !output.status.success(),
        "a malformed section must refuse the merge"
    );
    let merged = std::fs::read_to_string(shard(repository)).unwrap();
    assert!(!merged.contains("edge_theirs"), "{merged}");
}

#[test]
fn the_merge_driver_authorizes_and_writes_inside_the_publication_lock() {
    let directory = merge_fixture(Some("merger"));
    let repository = directory.path().to_path_buf();

    // Hold the repository publication lock from this test process, exactly
    // the way any provenance writer would.
    let layout = provenance_store::layout::ProvenanceLayout::new(
        camino::Utf8PathBuf::from_path_buf(repository.clone()).unwrap(),
    );
    let store = provenance_store::state_store::StateStore::new(layout);
    let (acquired_tx, acquired_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let holder = std::thread::spawn(move || {
        store
            .with_repository_publication(|| {
                acquired_tx.send(()).unwrap();
                release_rx.recv().unwrap();
                Ok(())
            })
            .unwrap();
    });
    acquired_rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the holder acquired the publication lock");

    let merge = std::thread::spawn(move || {
        std::process::Command::new("git")
            .args(["merge", "theirs", "-m", "merge"])
            .current_dir(&repository)
            .env_remove("GIT_DIR")
            .output()
            .expect("run git merge")
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
    assert!(
        !merge.is_finished(),
        "the merge driver must authorize and write inside the publication lock, \
         not against manifest bytes a concurrent writer may move"
    );

    release_tx.send(()).unwrap();
    holder.join().unwrap();
    let output = merge.join().unwrap();
    assert!(
        output.status.success(),
        "the granted merge completes once the lock frees: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn source(id: &str) -> String {
    format!(
        "{{\"schema_version\":1,\"scope_id\":\"default\",\"id\":\"{id}\",\
         \"declared_by\":null,\"declaration_address\":null,\"retired\":false,\
         \"name\":\"Policy\",\"source_type\":\"policy\",\"url\":null,\"reference\":null,\
         \"commit_pin\":null,\"effective_date\":null,\"review_date\":null,\
         \"superseded_by\":null,\"origin_thread\":null,\"origin_message\":null}}\n"
    )
}

/// A git repository whose conflicting shard is a scoped sources shard, so
/// the rbac gate must authorize the merge against the scope the shard
/// actually sits under — `.provenance/state/scopes/default`.
fn scoped_sources_fixture(actor_id: Option<&str>, grants: &str) -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    let repository = directory.path();
    git(repository, &["init", "--initial-branch", "main"]);
    git(repository, &["config", "user.email", "fixture@example.com"]);
    git(repository, &["config", "user.name", "Fixture"]);
    let shard_path = repository.join(".provenance/state/scopes/default/sources/source.jsonl");
    std::fs::create_dir_all(shard_path.parent().unwrap()).unwrap();
    std::fs::write(
        repository.join(".gitattributes"),
        ".provenance/state/**/*.jsonl merge=provenance-jsonl\n",
    )
    .unwrap();
    std::fs::write(manifest_path(repository), grants).unwrap();
    let base_source = source("source_base");
    std::fs::write(&shard_path, &base_source).unwrap();
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", "base"]);
    git(repository, &["checkout", "-b", "theirs"]);
    std::fs::write(
        &shard_path,
        format!("{base_source}{}", source("source_theirs")),
    )
    .unwrap();
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", "theirs"]);
    git(repository, &["checkout", "main"]);
    std::fs::write(
        &shard_path,
        format!("{base_source}{}", source("source_ours")),
    )
    .unwrap();
    git(repository, &["add", "."]);
    git(repository, &["commit", "-m", "ours"]);
    configure_driver(repository, actor_id);
    directory
}

fn sources_shard(repository: &Path) -> std::path::PathBuf {
    repository.join(".provenance/state/scopes/default/sources/source.jsonl")
}

#[test]
fn a_merge_of_a_scoped_shard_authorizes_the_scope_the_shard_sits_under() {
    let grants = r#"{
      "schema_version": 1,
      "scopes": [{"id": "default", "path_prefix": "."}],
      "disposition_actor_ids": [],
      "rbac": {"assignments": [
        {"actor_id": "merger", "identity_type": "human", "capabilities": ["edit"], "scopes": ["default"]}
      ]}
    }"#;
    let directory = scoped_sources_fixture(Some("merger"), grants);
    let repository = directory.path();

    let output = run_merge(repository);
    assert!(
        output.status.success(),
        "a merger granted edit on `default` must merge a default-scope shard: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let merged = std::fs::read_to_string(sources_shard(repository)).unwrap();
    assert!(merged.contains("source_theirs"), "{merged}");
}

#[test]
fn a_merge_of_a_scoped_shard_refuses_a_principal_granted_only_another_scope() {
    let grants = r#"{
      "schema_version": 1,
      "scopes": [{"id": "default", "path_prefix": "."}, {"id": "docs", "path_prefix": "docs"}],
      "disposition_actor_ids": [],
      "rbac": {"assignments": [
        {"actor_id": "merger", "identity_type": "human", "capabilities": ["edit"], "scopes": ["docs"]}
      ]}
    }"#;
    let directory = scoped_sources_fixture(Some("merger"), grants);
    let repository = directory.path();

    let output = run_merge(repository);
    assert!(
        !output.status.success(),
        "a principal holding edit only on `docs` must not merge a default-scope shard"
    );
    let merged = std::fs::read_to_string(sources_shard(repository)).unwrap();
    assert!(!merged.contains("source_theirs"), "{merged}");
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
