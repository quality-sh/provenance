#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use provenance_macros::verifies;
use provenance_transport::StatementHost;
use serde_json::json;
use support::records::Repository;
use support::resource_http::{call, call_with_headers};

fn host(repo: &Repository) -> StatementHost {
    support::resource_http::host(repo, true)
}

#[tokio::test]
async fn addressed_discussion_member_reads_cover_all_six_parent_kinds() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);
    for (plural, id) in [
        ("sources", "source_shared"),
        ("requirements", "req_shared"),
        ("resolutions", "resolution_shared"),
        ("rules", "rule_shared"),
        ("topics", "topic_shared"),
        ("questions", "question_shared"),
    ] {
        let parent = format!("/{plural}/{id}/discussions");
        let (status, started) = call(
            &host,
            "POST",
            &parent,
            Some(json!({"data":{
                "actor":"reviewer", "declared_by":null, "role":"user",
                "body":format!("Discussion for {plural}.")
            }})),
        )
        .await;
        assert_eq!(status, 200, "{plural}: {started}");
        let discussion_id = started["data"]["discussion_id"].as_str().unwrap();
        let (status, read) = call(&host, "GET", &format!("{parent}/{discussion_id}"), None).await;
        assert_eq!(status, 200, "{plural}: {read}");
        assert_eq!(read["data"]["discussion"]["discussion_id"], discussion_id);
    }
}

#[tokio::test]
async fn repeated_discussion_start_after_a_lost_response_creates_another_discussion() {
    let repo = Repository::new("The shared graph is readable.");
    repo.all_kinds();
    let host = host(&repo);
    let body = json!({"data":{
        "actor":"reviewer", "declared_by":null, "role":"user",
        "body":"A response can be lost."
    }});

    let (_, first) = call(
        &host,
        "POST",
        "/requirements/req_shared/discussions",
        Some(body.clone()),
    )
    .await;
    let (status, second) = call(
        &host,
        "POST",
        "/requirements/req_shared/discussions",
        Some(body),
    )
    .await;

    assert_eq!(status, 200, "{second}");
    assert_ne!(
        first["data"]["discussion_id"],
        second["data"]["discussion_id"]
    );
}

#[tokio::test]
async fn repeated_requirement_create_returns_a_typed_conflict() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo);
    let body = json!({"data":{
        "actor":"reviewer", "id":"req_created", "statement":"One statement.",
        "status":"discovery", "depends_on":[], "supersedes":[]
    }});

    let (status, first) = call(&host, "POST", "/requirements", Some(body.clone())).await;
    assert_eq!(status, 200, "{first}");
    let (status, repeated) = call(&host, "POST", "/requirements", Some(body)).await;

    assert_eq!(status, 409, "{repeated}");
    assert_eq!(repeated["error"]["kind"], "already_exists");
}

async fn start_with_reply(host: &StatementHost, parent: &str, name: &str) -> String {
    let (status, started) = call(
        host,
        "POST",
        parent,
        Some(json!({"data":{
            "actor":"reviewer", "declared_by":null, "role":"user",
            "body":format!("Concern {name}.")
        }})),
    )
    .await;
    assert_eq!(status, 200, "{started}");
    let id = started["data"]["discussion_id"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, replied) = call_with_headers(
        host,
        "POST",
        &format!("{parent}/{id}/messages"),
        Some(json!({"data":{
            "actor":"agent", "declared_by":null, "role":"assistant",
            "body":format!("Reply {name}.")
        }})),
        &[("if-match", "\"1\"")],
    )
    .await;
    assert_eq!(status, 200, "{replied}");
    id
}

/// Flow: start Discussions A and B on one Requirement, reply to each, resolve A,
/// then read through a new host on the same repository.
#[tokio::test]
#[verifies("rule_reply_threads_resolve_independently", examples)]
async fn two_discussions_on_one_record_keep_their_replies_and_status_after_restart() {
    let repo = Repository::new("The shared graph is readable.");
    let parent = "/requirements/req_shared/discussions";
    let first = host(&repo);
    let a = start_with_reply(&first, parent, "A").await;
    let b = start_with_reply(&first, parent, "B").await;
    let (status, resolved) = call_with_headers(
        &first,
        "PATCH",
        &format!("{parent}/{a}"),
        Some(json!({"data":{"actor":"reviewer","declared_by":null,"status":"resolved"}})),
        &[("if-match", "\"2\"")],
    )
    .await;
    assert_eq!(status, 200, "{resolved}");
    drop(first);

    let restarted = host(&repo);
    let (status, listed) = call(&restarted, "GET", parent, None).await;
    assert_eq!(status, 200, "{listed}");
    let mut listed = listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|item| {
            let discussion = &item["discussion"];
            (
                discussion["discussion_id"].as_str().unwrap().to_owned(),
                discussion["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<Vec<_>>();
    listed.sort();
    let mut expected = vec![
        (a.clone(), "resolved".to_owned()),
        (b.clone(), "active".to_owned()),
    ];
    expected.sort();
    assert_eq!(listed, expected);

    for (id, name, status) in [(&a, "A", "resolved"), (&b, "B", "active")] {
        let (code, read) = call(&restarted, "GET", &format!("{parent}/{id}"), None).await;
        assert_eq!(code, 200, "{read}");
        assert_eq!(read["data"]["discussion"]["discussion_id"], *id);
        assert_eq!(read["data"]["discussion"]["status"], status);
        let (code, messages) =
            call(&restarted, "GET", &format!("{parent}/{id}/messages"), None).await;
        assert_eq!(code, 200, "{messages}");
        let bodies = messages["data"]["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| message["body"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            bodies,
            [format!("Concern {name}."), format!("Reply {name}.")]
        );
    }
}
