#[path = "review_host_support/mod.rs"]
mod review_host_support;
use provenance_macros::verifies;
use review_host_support::{
    events::{source, Socket},
    repository, request, start,
};
use serde_json::{json, Value};

#[test]
#[verifies("rule_review_host_pushes_graph_changes", examples)]
fn external_cli_write_reaches_two_clients_and_committed_read() {
    let repo = repository();
    let host = start(repo.path());
    let mut first = Socket::connect(&host);
    let ready = first.frame();
    assert_eq!(ready["type"], "ready");
    let mut second = Socket::connect(&host);
    assert_eq!(second.frame(), ready);
    source(&host, repo.path(), "source_external");
    let changed = first.frame();
    assert_eq!(second.frame(), changed);
    assert_eq!(changed["type"], "changed");
    assert_eq!(changed["epoch"], ready["epoch"]);
    assert_eq!(
        changed["sequence"].as_u64(),
        Some(ready["sequence"].as_u64().unwrap() + 1)
    );
    assert_eq!(
        changed["scopes"],
        json!([{"scope":"default", "records":[{"kind":"source", "id":"source_external"}], "families":["source"]}])
    );
    let read: Value = serde_json::from_str(
        &request(&host, "GET", "/sources/source_external", true)
            .call()
            .unwrap()
            .into_string()
            .unwrap(),
    )
    .unwrap();
    assert!(read.to_string().contains("source_external"));
    first.quiet();
    second.quiet();
}

#[tokio::test]
// Implementation aid: the observation Rule is a plan proposal until version fields are removed.
async fn atomic_external_deletion_is_observed_after_guard_release() {
    let repo = repository();
    let host = start(repo.path());
    source(&host, repo.path(), "source_deleted");
    let mut socket = Socket::connect(&host);
    socket.frame();
    let layout = provenance_store::layout::ProvenanceLayout::new(
        camino::Utf8PathBuf::from_path_buf(repo.path().to_path_buf()).unwrap(),
    );
    let guard = provenance_store::publication::publication_guard(&layout)
        .await
        .unwrap();
    let shard = layout.scopes_dir().join("default/sources/source.jsonl");
    let replacement = shard.with_extension("replacement");
    std::fs::write(&replacement, "").unwrap();
    std::fs::rename(replacement, shard).unwrap();
    socket.quiet();
    drop(guard);
    let changed = socket.frame();
    assert_eq!(changed["type"], "changed");
    assert_eq!(
        changed["scopes"][0]["records"],
        json!([{"kind":"source","id":"source_deleted"}])
    );
    assert_eq!(
        review_host_support::response(
            request(&host, "GET", "/sources/source_deleted", true).call()
        )
        .status(),
        404
    );
    socket.quiet();
}

#[test]
// Implementation aid: the selected scope and canonical content define invalidations.
fn unrelated_scope_cache_and_equal_content_do_not_invalidate() {
    let repo = repository();
    let status = std::process::Command::new(assert_cmd::cargo::cargo_bin("provenance"))
        .args([
            "init",
            "--path",
            repo.path().to_str().unwrap(),
            "--scope",
            "other",
            "--path-prefix",
            "other/",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let host = start(repo.path());
    let mut socket = Socket::connect(&host);
    socket.frame();
    let output = host
        .cli()
        .args([
            "sources",
            "create",
            "--repo",
            repo.path().to_str().unwrap(),
            "--scope",
            "other",
            "--id",
            "source_other",
            "--name",
            "Other",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(repo.path().join(".provenance/cache/noise"), "noise").unwrap();
    socket.quiet();
    source(&host, repo.path(), "source_local");
    assert_eq!(
        socket.frame()["scopes"][0]["records"][0]["id"],
        "source_local"
    );
    let shard = repo
        .path()
        .join(".provenance/state/scopes/default/sources/source.jsonl");
    let bytes = std::fs::read(&shard).unwrap();
    let replacement = shard.with_extension("replacement");
    std::fs::write(&replacement, bytes).unwrap();
    std::fs::rename(replacement, shard).unwrap();
    socket.quiet();
    std::fs::write(repo.path().join(".provenance/settings.json"), "{}\n").unwrap();
    assert_eq!(socket.frame()["type"], "reset");
}

#[tokio::test]
// Implementation aid: a guarded publication must expose one coherent record delta.
async fn rename_burst_is_coherent_and_failed_cli_preparation_is_quiet() {
    let repo = repository();
    let host = start(repo.path());
    source(&host, repo.path(), "source_one");
    source(&host, repo.path(), "source_two");
    let mut socket = Socket::connect(&host);
    socket.frame();
    let layout = provenance_store::layout::ProvenanceLayout::new(
        camino::Utf8PathBuf::from_path_buf(repo.path().to_path_buf()).unwrap(),
    );
    let guard = provenance_store::publication::publication_guard(&layout)
        .await
        .unwrap();
    let shard = layout.scopes_dir().join("default/sources/source.jsonl");
    let records: Vec<Value> = std::fs::read_to_string(&shard)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    for name in ["Intermediate", "Final"] {
        let bytes = records
            .iter()
            .map(|record| {
                let mut record = record.clone();
                record["name"] = json!(name);
                format!("{record}\n")
            })
            .collect::<String>();
        let replacement = shard.with_extension("replacement");
        std::fs::write(&replacement, bytes).unwrap();
        std::fs::rename(replacement, &shard).unwrap();
    }
    socket.quiet();
    drop(guard);
    let changed = socket.frame();
    assert_eq!(changed["type"], "changed");
    assert_eq!(
        changed["scopes"][0]["records"],
        json!([{"kind":"source","id":"source_one"},{"kind":"source","id":"source_two"}])
    );
    let read = request(&host, "GET", "/sources/source_one", true)
        .call()
        .unwrap()
        .into_string()
        .unwrap();
    assert!(read.contains("Final"));
    assert!(!read.contains("Intermediate"));
    let output = host
        .cli()
        .args([
            "sources",
            "create",
            "--repo",
            repo.path().to_str().unwrap(),
            "--id",
            "source_one",
            "--name",
            "Duplicate",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    socket.quiet();
}
