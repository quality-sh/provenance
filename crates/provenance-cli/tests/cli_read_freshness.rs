#[path = "review_host_support/mod.rs"]
mod review_host_support;

use review_host_support::{events::source_before_host, repository, request, start};
use serde_json::Value;

fn read(host: &review_host_support::Host) -> Value {
    let response = request(host, "GET", "/sources/source_fresh", true)
        .call()
        .unwrap();
    assert_eq!(response.status(), 200);
    serde_json::from_str(&response.into_string().unwrap()).unwrap()
}

// Characterization: an external file edit must reach the public member route.
#[test]
fn member_read_sees_an_external_file_change() {
    let repo = repository();
    source_before_host(repo.path(), "source_fresh");
    let host = start(repo.path());
    assert_eq!(read(&host)["data"]["name"], "source_fresh");
    let shard = repo
        .path()
        .join(".provenance/state/scopes/default/sources/source.jsonl");
    let content = std::fs::read_to_string(&shard).unwrap();
    let edited = content.replace("\"name\":\"source_fresh\"", "\"name\":\"Changed source\"");
    assert_ne!(edited, content);
    std::fs::write(&shard, edited).unwrap();
    assert_eq!(read(&host)["data"]["name"], "Changed source");
}
