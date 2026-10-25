//! Typed-spec apply deletes the Discussions of the records that it deletes.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_macros::verifies;
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

/// The id that an apply result gives the declaration with `key`.
fn resource_id(applied: &Value, key: &str) -> String {
    applied["data"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["key"] == key)
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Only `other` and its Thread and Message stay, read through the public
/// list routes.
async fn assert_only_the_other_discussion_stays(
    host: &provenance_transport::StatementHost,
    other: &Value,
    removed: &[&Value],
) {
    let items = |listed: &Value| listed["data"]["items"].as_array().unwrap().clone();
    let (status, listed) = call(host, "GET", PARENT, None).await;
    assert_eq!(status, 200, "{listed}");
    let discussions = items(&listed);
    assert_eq!(discussions.len(), 1, "{listed}");
    assert_eq!(
        discussions[0]["discussion"]["discussion_id"],
        other["discussion_id"]
    );
    let (status, listed) = call(host, "GET", "/discussion-containers", None).await;
    assert_eq!(status, 200, "{listed}");
    let threads = items(&listed);
    assert_eq!(threads.len(), 1, "{listed}");
    assert_eq!(threads[0]["id"], other["thread_id"]);
    let (status, listed) = call(host, "GET", "/messages", None).await;
    assert_eq!(status, 200, "{listed}");
    let messages = items(&listed);
    assert_eq!(messages.len(), 1, "{listed}");
    assert_eq!(messages[0]["thread_id"], other["thread_id"]);
    for discussion in removed {
        assert_ne!(discussion["thread_id"], other["thread_id"]);
    }
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
    let (source, requirement) = (resource_id(&applied, "src"), resource_id(&applied, "drop"));
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

    assert_only_the_other_discussion_stays(&host, &other, &[&on_source, &on_resolution]).await;

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

#[tokio::test]
#[verifies("rule_typed_deletion_deletes_discussions", examples)]
async fn a_record_with_review_submissions_is_refused_and_keeps_its_discussions() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let mut applied = Value::Null;
    // The second statement is a typed change, so apply opens a submission.
    for statement in ["The host stops.", "The host starts."] {
        let (status, result) = call(
            &host,
            "POST",
            "/authoring-changes",
            Some(spec(
                &json!([]),
                &json!([{"key":"kept", "statement":statement}]),
            )),
        )
        .await;
        assert_eq!(status, 200, "{result}");
        applied = result;
    }
    let requirement = resource_id(&applied, "kept");
    let discussions = format!("/requirements/{requirement}/discussions");
    let started = start(&host, &discussions, json!("spec://fx")).await;

    let (status, refused) = call(
        &host,
        "POST",
        "/authoring-changes",
        Some(spec(&json!([]), &json!([]))),
    )
    .await;
    assert_eq!(status, 409, "{refused}");
    assert_eq!(
        refused["error"]["kind"], "enrolled_record_deletion_conflict",
        "{refused}"
    );
    let (status, read) = call(&host, "GET", &format!("/requirements/{requirement}"), None).await;
    assert_eq!(status, 200, "{read}");
    let (status, listed) = call(&host, "GET", &discussions, None).await;
    assert_eq!(status, 200, "{listed}");
    let items = listed["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "{listed}");
    assert_eq!(
        items[0]["discussion"]["discussion_id"],
        started["discussion_id"]
    );
    let (status, listed) = call(&host, "GET", "/messages", None).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(
        listed["data"]["items"].as_array().unwrap().len(),
        1,
        "{listed}"
    );
}
