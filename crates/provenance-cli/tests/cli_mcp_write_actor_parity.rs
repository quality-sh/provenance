//! The CLI and a spawned `provenance mcp` give the same answer for the same write, with or
//! without an actor. The writes that need an actor or that refuse one keep that behaviour
//! on the two interfaces.

use crate::cli_mcp_record_actions::Server;
use crate::cli_mcp_write_actor::{cli_json, cli_review, mcp_review, repository, run, STATEMENT};
use provenance_macros::verifies;
use serde_json::{json, Map, Value};

/// The value with each proposal ID replaced by the field name, because each write makes a
/// new random proposal ID.
fn without_proposal_ids(value: &Value) -> Value {
    match value {
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, field)| {
                    let field = if key == "proposal_id" {
                        json!("<proposal_id>")
                    } else {
                        without_proposal_ids(field)
                    };
                    (key.clone(), field)
                })
                .collect::<Map<_, _>>(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(without_proposal_ids).collect()),
        other => other.clone(),
    }
}

/// Compares the data of a CLI answer with the data of an MCP answer field by field. The
/// CLI data has one more field, `review_message`, which tells how to start a review host.
fn assert_same_data(by_cli: &Value, by_mcp: &Value) {
    let by_cli = without_proposal_ids(by_cli);
    let by_mcp = without_proposal_ids(by_mcp);
    let by_cli = by_cli.as_object().unwrap();
    let by_mcp = by_mcp.as_object().unwrap();
    for (field, value) in by_mcp {
        assert_eq!(by_cli.get(field), Some(value), "field {field}");
    }
    let extra = by_cli
        .keys()
        .filter(|field| !by_mcp.contains_key(*field))
        .collect::<Vec<_>>();
    assert_eq!(extra, ["review_message"]);
}

/// Creates `req_a` and updates its description in two new repositories, one through the
/// CLI and one through MCP, with the same fields. `actor` is the actor of the two writes,
/// or no actor. Gives the pending actor that each interface stores.
async fn create_and_update(actor: Option<&str>) -> [Value; 2] {
    let [by_cli, by_mcp] = [repository(), repository()];
    let actor_flags = actor.map_or_else(Vec::new, |actor| vec!["--actor", actor]);
    let mut create = vec!["req_a", "create", "--type", "requirement", "--statement", STATEMENT];
    create.extend(["--status", "active", "--depends-on-json", "[]", "--supersedes-json", "[]"]);
    create.extend(&actor_flags);
    let cli_created = cli_json(by_cli.path(), &create);
    let etag = cli_review(by_cli.path())["edit"]["etag"].as_str().unwrap().to_owned();
    let mut update = vec!["req_a", "update", "--if-match", &etag, "--description", "Edited."];
    update.extend(&actor_flags);
    let cli_updated = cli_json(by_cli.path(), &update);

    let server = Server::start(by_mcp.path().to_str().unwrap()).await;
    let mut arguments = json!({"action":"create","target":"req_a","type":"requirement",
        "statement":STATEMENT,"status":"active","depends_on":[],"supersedes":[]});
    if let Some(actor) = actor {
        arguments["actor"] = json!(actor);
    }
    let mcp_created = server.client.call("change", arguments).await;
    let mut arguments = json!({"action":"update","target":"req_a","if_match":etag,
        "description":"Edited."});
    if let Some(actor) = actor {
        arguments["actor"] = json!(actor);
    }
    let mcp_updated = server.client.call("change", arguments).await;
    let mcp_stored = mcp_review(&server).await["decision"]["pending"]["actor"].clone();
    server.close().await;

    assert_same_data(&cli_created["data"], &mcp_created["structuredContent"]["data"]);
    assert_same_data(&cli_updated["data"], &mcp_updated["structuredContent"]["data"]);
    let cli_stored = cli_review(by_cli.path())["decision"]["pending"]["actor"].clone();
    [cli_stored, mcp_stored]
}

#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn the_cli_and_mcp_give_the_same_answer_to_a_write_with_no_actor() {
    let stored = create_and_update(None).await;
    assert_eq!(stored, [json!("authoring"), json!("authoring")]);
}

/// A write that gives its actor records that actor on the two interfaces.
#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn a_write_that_gives_an_actor_records_that_actor_on_the_cli_and_mcp() {
    let stored = create_and_update(Some("agent")).await;
    assert_eq!(stored, [json!("agent"), json!("agent")]);
}

/// A discuss with no actor gives the same receipt on the two interfaces, apart from the
/// IDs that the write makes.
#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn the_cli_and_mcp_give_the_same_receipt_to_a_discuss_with_no_actor() {
    let [by_cli, by_mcp] = [repository(), repository()];
    let mut receipts = Vec::new();
    for repo in [by_cli.path(), by_mcp.path()] {
        crate::cli_mcp_write_actor::requirement(repo);
    }
    let started = cli_json(
        by_cli.path(),
        &["req_a", "discuss", "--role", "user", "--body", "A concern."],
    );
    receipts.push(started);
    let server = Server::start(by_mcp.path().to_str().unwrap()).await;
    let arguments = json!({"action":"discuss","target":"req_a","role":"user","body":"A concern."});
    let mut started = server.client.call("change", arguments).await["structuredContent"].clone();
    server.close().await;
    assert_eq!(started["action"], "discuss", "{started}");
    started.as_object_mut().unwrap().remove("action");
    receipts.push(started);
    for receipt in &mut receipts {
        let receipt = &mut receipt["receipt"];
        for generated in ["discussion_id", "root_message_id", "message_ids"] {
            receipt[generated] = json!(format!("<{generated}>"));
        }
    }
    assert_eq!(receipts[0], receipts[1]);
    assert_eq!(receipts[0]["receipt"]["actor"], "authoring");
}

/// A submit that gives no actor is refused on the two interfaces with the same error.
#[tokio::test]
#[verifies("rule_write_actor_required", examples)]
async fn a_submit_with_no_actor_is_refused_on_the_cli_and_mcp() {
    let directory = repository();
    let repo = directory.path();
    crate::cli_mcp_write_actor::requirement(repo);
    let refusal = json!({"error":{"field":"request.actor","kind":"invalid_input",
        "reason":"required"},"meta":{}});
    let lists = ["--source-ids-json", "[]", "--evidence-references-json", "[]"];
    let submit = [&["req_a", "submit"][..], &lists[..], &["--builds-on-json", "[]"]].concat();
    let (code, stdout, stderr) = run(repo, &[&submit[..], &["--format", "json"]].concat());
    assert_eq!((code, stdout.as_str()), (1, ""));
    assert_eq!(serde_json::from_str::<Value>(&stderr).unwrap(), refusal);
    let server = Server::start(repo.to_str().unwrap()).await;
    let refused = server
        .client
        .call(
            "change",
            json!({"action":"submit","target":"req_a","source_ids":[],"evidence_references":[],
                "builds_on":[]}),
        )
        .await;
    server.close().await;
    assert_eq!(refused["isError"], json!(true), "{refused}");
    assert_eq!(refused["structuredContent"], refusal);
}

/// A Rule or a Resolution write takes no actor on the two interfaces, and records the actor
/// `authoring`.
#[tokio::test]
#[verifies("rule_write_without_actor_records_authoring", examples)]
async fn rule_and_resolution_writes_record_authoring_and_refuse_an_actor() {
    let directory = repository();
    let repo = directory.path();
    crate::cli_mcp_write_actor::requirement(repo);
    let rule = ["rule_a", "create", "--type", "rule", "--statement", "The system stores rules."];
    let resolution = ["res_a", "create", "--type", "resolution", "--title", "One store"];
    let resolution_text = ["--position", "Keep one store.", "--rationale", "It is simple."];
    let writes = [rule.to_vec(), [&resolution[..], &resolution_text[..]].concat()];
    for create in writes {
        let id = create[0];
        let create = [&create[..], &["--requirement-ids", "req_a"]].concat();
        let (code, stdout, stderr) = run(repo, &[&create[..], &["--actor", "agent"]].concat());
        assert_eq!(
            (code, stdout.as_str(), stderr.as_str()),
            (2, "", "error: unsupported option --actor\n")
        );
        cli_json(repo, &create);
        let read = cli_json(repo, &[id, "get", "--view", "review"]);
        assert_eq!(read["review"]["decision"]["pending"]["actor"], "authoring", "{id}");
    }
    let server = Server::start(repo.to_str().unwrap()).await;
    for (id, field) in [("rule_a", "description"), ("res_a", "rationale")] {
        let mut arguments = json!({"action":"update","target":id,"actor":"agent"});
        arguments[field] = json!("Edited.");
        let refused = server.client.call("change", arguments).await;
        assert_eq!(refused["isError"], json!(true), "{refused}");
        let error = &refused["structuredContent"]["error"];
        assert_eq!(
            (&error["kind"], &error["field"], &error["reason"]),
            (&json!("invalid_input"), &json!("actor"), &json!("unknown_field")),
            "{id}"
        );
    }
    server.close().await;
}
