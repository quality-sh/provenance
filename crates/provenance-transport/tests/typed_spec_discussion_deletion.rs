//! Typed-spec apply deletes the Discussions of the records that it deletes.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_core::ScopeId;
use provenance_macros::verifies;
use provenance_store::state_store::StateStore;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, call_with_headers, host};

const PARENT: &str = "/requirements/req_shared/discussions";

async fn start(
    host: &provenance_transport::StatementHost,
    path: &str,
    declared_by: Value,
) -> Value {
    let body = json!({"data":{
        "actor":"reviewer", "declared_by":declared_by, "role":"user", "body":"A concern."
    }});
    let (status, started) = call(host, "POST", path, Some(body)).await;
    assert_eq!(status, 200, "{started}");
    started["data"].clone()
}

fn spec(sources: &Value, requirements: &Value) -> Value {
    json!({"data":{
        "schema_version":2, "spec":"fx", "declared_by":"spec://fx",
        "sources":sources, "requirements":requirements
    }})
}

/// Only `other` and its Thread and Message stay in the Discussion shards.
fn assert_only_the_other_discussion_stays(repo: &Repository, other: &Value, removed: &[&Value]) {
    let store = StateStore::new(repo.layout.clone());
    let scope = ScopeId::new("default").unwrap();
    let discussions = store.list_discussions(&scope).unwrap();
    assert_eq!(discussions.len(), 1, "{discussions:?}");
    assert_eq!(
        discussions[0].discussion_id.as_str(),
        other["discussion_id"].as_str().unwrap()
    );
    let removed_threads = removed
        .iter()
        .map(|discussion| discussion["thread_id"].as_str().unwrap())
        .collect::<Vec<_>>();
    let threads = store.list_threads(&scope).unwrap();
    assert_eq!(threads.len(), 1, "{threads:?}");
    assert_eq!(threads[0].parent.node_id.as_str(), "req_shared");
    let messages = store.list_messages(&scope).unwrap();
    assert!(messages
        .iter()
        .all(|message| !removed_threads.contains(&message.thread_id.as_str())));
    assert_eq!(messages.len(), 1, "{messages:?}");
}

#[tokio::test]
#[verifies("rule_typed_deletion_deletes_discussions", examples)]
async fn typed_spec_apply_deletes_the_discussions_of_the_records_it_deletes() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let (status, applied) = call(
        &host,
        "POST",
        "/authoring-changes",
        Some(spec(
            &json!([{"key":"src", "name":"Original", "kind":"policy"}]),
            &json!([{"key":"drop", "statement":"The host stops."}]),
        )),
    )
    .await;
    assert_eq!(status, 200, "{applied}");
    let id = |key: &str| {
        applied["data"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|resource| resource["key"] == key)
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let (source, requirement) = (id("src"), id("drop"));
    // The Resolution is deleted by cascade with its Requirement.
    let (status, created) = call(
        &host,
        "POST",
        "/resolutions",
        Some(json!({"data":{
            "id":"res_drop", "title":"Stop", "requirement_ids":[requirement],
            "position":"The host stops.", "rationale":"The host stops.",
            "status":"proposed", "supersedes":[], "inputs":[]
        }})),
    )
    .await;
    assert_eq!(status, 200, "{created}");
    let on_source = start(
        &host,
        &format!("/sources/{source}/discussions"),
        json!("spec://fx"),
    )
    .await;
    let on_resolution = start(&host, "/resolutions/res_drop/discussions", Value::Null).await;
    let resolve = json!({"data":{"actor":"reviewer", "declared_by":null, "status":"resolved"}});
    let (status, resolved) = call_with_headers(
        &host,
        "PATCH",
        &format!(
            "/resolutions/res_drop/discussions/{}",
            on_resolution["discussion_id"].as_str().unwrap()
        ),
        Some(resolve),
        &[("if-match", "\"1\"")],
    )
    .await;
    assert_eq!(status, 200, "{resolved}");
    let other = start(&host, PARENT, Value::Null).await;

    let (status, applied) = call(
        &host,
        "POST",
        "/authoring-changes",
        Some(spec(&json!([]), &json!([]))),
    )
    .await;
    assert_eq!(status, 200, "{applied}");
    assert_eq!(applied["data"]["deleted"], 3, "{applied}");
    assert_eq!(
        applied["data"]["cascade"],
        json!([{"id":"res_drop", "kind":"resolution", "state":"deleted"}])
    );
    for path in [format!("/sources/{source}"), "/resolutions/res_drop".into()] {
        let (status, read) = call(&host, "GET", &path, None).await;
        assert_eq!(status, 404, "{path}: {read}");
    }

    assert_only_the_other_discussion_stays(&repo, &other, &[&on_source, &on_resolution]);

    // Discussion reads and writes on the other records keep working.
    start(&host, PARENT, Value::Null).await;
    let (status, listed) = call(&host, "GET", PARENT, None).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(
        listed["data"]["items"].as_array().unwrap().len(),
        2,
        "{listed}"
    );
}
