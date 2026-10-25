//! Discussion writes answer client faults with typed refusals, not 500s.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
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

async fn set_status(
    host: &provenance_transport::StatementHost,
    discussion: &str,
    status: &str,
    version: u64,
) -> (u16, Value) {
    let body = json!({"data":{"actor":"reviewer", "declared_by":null, "status":status}});
    let guard = format!("\"{version}\"");
    call_with_headers(
        host,
        "PATCH",
        discussion,
        Some(body),
        &[("if-match", &guard)],
    )
    .await
}

#[tokio::test]
async fn a_blank_actor_is_an_invalid_input_on_the_actor_field() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let (status, read) = call(&host, "GET", "/requirements/req_shared", None).await;
    assert_eq!(status, 200, "{read}");
    let etag = read["data"]["edit"]["etag"].as_str().unwrap().to_owned();
    let start_body =
        json!({"data":{"actor":"  ", "declared_by":null, "role":"user", "body":"A concern."}});
    let save_body = json!({"data":{"actor":"  ", "description":"Edited."}});
    for (method, path, body, headers) in [
        ("POST", PARENT, start_body, Vec::new()),
        (
            "PATCH",
            "/requirements/req_shared",
            save_body,
            vec![("if-match", etag.as_str())],
        ),
    ] {
        let (status, refused) = call_with_headers(&host, method, path, Some(body), &headers).await;
        assert_eq!(status, 400, "{method} {path}: {refused}");
        assert_eq!(
            refused["error"],
            json!({"kind":"invalid_input", "field":"request.actor", "reason":"invalid_value"}),
            "{method} {path}"
        );
    }
    let (status, listed) = call(&host, "GET", PARENT, None).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(listed["data"]["items"], json!([]));
}

#[tokio::test]
async fn setting_the_status_a_discussion_already_has_is_a_no_op() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let started = start(&host, PARENT, Value::Null).await;
    let discussion = format!("{PARENT}/{}", started["discussion_id"].as_str().unwrap());

    let (status, unchanged) = set_status(&host, &discussion, "active", 1).await;
    assert_eq!(status, 200, "{unchanged}");
    assert_eq!(unchanged["data"]["status"], "active");
    assert_eq!(unchanged["data"]["version"], 1);

    let (status, resolved) = set_status(&host, &discussion, "resolved", 1).await;
    assert_eq!(status, 200, "{resolved}");
    assert_eq!(resolved["data"]["version"], 2);

    let (status, repeated) = set_status(&host, &discussion, "resolved", 2).await;
    assert_eq!(status, 200, "{repeated}");
    assert_eq!(repeated["data"]["status"], "resolved");
    assert_eq!(repeated["data"]["version"], 2);

    let (status, read) = call(&host, "GET", &discussion, None).await;
    assert_eq!(status, 200, "{read}");
    assert_eq!(read["data"]["discussion"]["version"], 2);

    // A stale guard still wins over the no-op.
    let (status, stale) = set_status(&host, &discussion, "resolved", 1).await;
    assert_eq!(status, 409, "{stale}");
    assert_eq!(stale["error"]["kind"], "discussion_version_conflict");
}

#[tokio::test]
async fn typed_spec_apply_refuses_to_delete_a_record_that_has_discussions() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let spec = |sources: Value| {
        json!({"data":{
            "schema_version":2, "spec":"fx", "declared_by":"spec://fx", "sources":sources
        }})
    };
    let (status, applied) = call(
        &host,
        "POST",
        "/authoring-changes",
        Some(spec(
            json!([{"key":"src", "name":"Original", "kind":"policy"}]),
        )),
    )
    .await;
    assert_eq!(status, 200, "{applied}");
    assert_eq!(applied["data"]["created"], 1);
    let source = applied["data"]["resources"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    start(
        &host,
        &format!("/sources/{source}/discussions"),
        json!("spec://fx"),
    )
    .await;

    let (status, refused) = call(&host, "POST", "/authoring-changes", Some(spec(json!([])))).await;
    assert_eq!(status, 409, "{refused}");
    assert_eq!(
        refused["error"],
        json!({
            "kind":"enrolled_record_deletion_conflict",
            "record_kind":"source", "record_id":source
        })
    );

    let (status, kept) = call(&host, "GET", &format!("/sources/{source}"), None).await;
    assert_eq!(status, 200, "{kept}");
    assert_eq!(kept["data"]["name"], "Original");
    // Discussion writes elsewhere in the scope keep working.
    start(&host, PARENT, Value::Null).await;
    let (status, listed) = call(&host, "GET", PARENT, None).await;
    assert_eq!(status, 200, "{listed}");
    assert_eq!(
        listed["data"]["items"].as_array().unwrap().len(),
        1,
        "{listed}"
    );
}
