#[path = "review_host_support/mod.rs"]
mod review_host_support;
use provenance_macros::verifies;
use review_host_support::{events::{source, Socket}, repository, request, start};
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
    assert_eq!(changed["sequence"].as_u64(), Some(ready["sequence"].as_u64().unwrap() + 1));
    assert_eq!(changed["scopes"], json!([{"scope":"default", "records":[{"kind":"source", "id":"source_external"}], "families":["source"]}]));
    let read: Value = serde_json::from_str(&request(&host, "GET", "/sources/source_external", true).call().unwrap().into_string().unwrap()).unwrap();
    assert!(read.to_string().contains("source_external"));
    first.quiet(); second.quiet();
}

#[tokio::test]
// Implementation aid: the observation Rule is a plan proposal until version fields are removed.
async fn atomic_external_deletion_is_observed_after_guard_release() {
    let repo = repository(); let host = start(repo.path());
    source(&host, repo.path(), "source_deleted");
    let mut socket = Socket::connect(&host); socket.frame();
    let layout = provenance_store::layout::ProvenanceLayout::new(camino::Utf8PathBuf::from_path_buf(repo.path().to_path_buf()).unwrap());
    let guard = provenance_store::publication::publication_guard(&layout).await.unwrap();
    let shard = layout.scopes_dir().join("default/sources/source.jsonl");
    let replacement = shard.with_extension("replacement");
    std::fs::write(&replacement, "").unwrap(); std::fs::rename(replacement, shard).unwrap();
    socket.quiet(); drop(guard);
    let changed = socket.frame();
    assert_eq!(changed["type"], "changed");
    assert_eq!(changed["scopes"][0]["records"], json!([{"kind":"source","id":"source_deleted"}]));
    assert_eq!(review_host_support::response(request(&host, "GET", "/sources/source_deleted", true).call()).status(), 404);
    socket.quiet();
}
