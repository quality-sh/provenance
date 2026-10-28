//! A shard that does not parse reaches the review page without the host path.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_core::{NodeType, ScopeId};
use support::records::Repository;
use support::resource_http::{call, host};

// Implementation aid: keeps the absolute state path off the page; no Rule
// states what a catch-up failure message contains.
#[tokio::test]
async fn a_shard_parse_error_on_a_review_read_names_the_shard_inside_the_state_directory() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let path = "/requirements/req_shared/discussions";
    let (status, healthy) = call(&host, "GET", path, None).await;
    assert_eq!(status, 200, "{healthy}");

    let scope = ScopeId::new("default").unwrap();
    let shard = provenance_store::shards::path_for(&repo.layout, &scope, NodeType::Rule);
    let stored = std::fs::read_to_string(&shard).unwrap();
    std::fs::write(&shard, format!("{stored}\n")).unwrap();

    let (_, answer) = call(&host, "GET", path, None).await;
    let text = answer.to_string().replace("\\\\", "/");
    let root = repo.dir.path().to_str().unwrap().replace('\\', "/");
    assert!(
        text.contains("scopes/default/rules/rule.jsonl line 2: EOF while parsing a value"),
        "{text}"
    );
    assert!(!text.contains(&root), "{text}");
}
