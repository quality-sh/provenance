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
    let shard = repo.path().join(".provenance/state/scopes/default/sources/source.jsonl");
    let content = std::fs::read_to_string(&shard).unwrap();
    std::fs::write(&shard, content.replace("\"name\":\"source_fresh\"", "\"name\":\"Changed source\"")).unwrap();
    assert_eq!(read(&host)["data"]["name"], "Changed source");
}

// Characterization: a failed catch-up must stay explicit at the public route.
#[test]
fn member_read_reports_a_failed_catch_up() {
    let repo = repository();
    source_before_host(repo.path(), "source_fresh");
    let host = start(repo.path());
    assert_eq!(read(&host)["data"]["id"], "source_fresh");
    let shard = repo.path().join(".provenance/state/scopes/default/sources/source.jsonl");
    std::fs::write(shard, "invalid json\n").unwrap();
    let answer = read(&host);
    assert_eq!(answer["data"]["id"], "source_fresh");
    assert_eq!(answer["meta"]["stamp"]["policy"], "catch_up_failed");
    assert!(answer["meta"]["freshness_error"].as_str().is_some_and(|text| !text.is_empty()));
}
