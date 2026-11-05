#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
    #[allow(dead_code)]
    #[path = "../../../provenance-transport/tests/support/resource_http.rs"]
    pub mod resource_http;
    pub mod schema_check;
}

use provenance_core::{ScopeId, StableId};
use provenance_macros::verifies;
use rmcp::ServiceExt as _;
use serde_json::json;
use support::records::Repository;
use support::schema_check::{call, schemas};

#[tokio::test]
#[verifies("rule_porcelain_mcp_target_argument", examples)]
async fn mcp_target_first_authoring_publishes_target_and_type_schemas() {
    let repository = Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, true);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    for action in provenance_porcelain::action::Action::RECORD {
        assert!(!tools.iter().any(|tool| tool.name == action.as_str()));
    }
    let (input, _) = schemas(&client, "change").await;
    let actions = input["properties"]["action"]["enum"].as_array().unwrap();
    for action in provenance_porcelain::action::Action::RECORD {
        assert!(actions.contains(&json!(action.as_str())), "{actions:?}");
    }
    assert_eq!(input["properties"]["target"]["type"], "string");
    assert!(input["properties"]["type"]["enum"]
        .as_array()
        .unwrap()
        .contains(&json!("source")));
    let create = call(
        &client,
        "change",
        json!({"action":"create","target":"req_x","type":"requirement"}),
    )
    .await;
    let schema = create.structured_content.unwrap()["error"]["schema"].to_string();
    assert!(schema.contains("refines"), "{schema}");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
#[verifies("rule_porcelain_mcp_target_argument", examples)]
#[verifies("rule_porcelain_mcp_readable_structured", examples)]
async fn mcp_target_first_source_create_and_update_keep_structured_targets() {
    let repository = Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, true);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let created = call(
        &client,
        "change",
        json!({"action":"create",
            "target":"source_mcp_target",
            "type":"source",
            "data":{
                "name":"MCP target",
                "source_type":"policy",
                "url":"https://example.test/policy",
                "supersedes":[]
            }
        }),
    )
    .await;
    assert_ne!(created.is_error, Some(true), "{created:?}");
    assert_eq!(
        created.structured_content.as_ref().unwrap()["data"]["id"],
        "source_mcp_target"
    );
    assert!(created.content[0]
        .as_text()
        .unwrap()
        .text
        .contains("source_mcp_target"));

    let edited = call(
        &client,
        "change",
        json!({"action":"update","target":"source_mcp_target","data":{"reference":"section 1","url":null}}),
    )
    .await;
    assert_ne!(edited.is_error, Some(true), "{edited:?}");
    assert_eq!(
        edited.structured_content.as_ref().unwrap()["data"]["name"],
        "MCP target"
    );
    assert_eq!(
        edited.structured_content.as_ref().unwrap()["data"]["reference"],
        "section 1"
    );
    assert!(edited.structured_content.as_ref().unwrap()["data"]["url"].is_null());

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_target_first_requirement_update_passes_relationship_deltas() {
    let repository = Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, true);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let requirement = call(
        &client,
        "change",
        json!({"action":"create",
            "target":"req_mcp_target",
            "type":"requirement",
            "data":{
                "actor":"agent",
                "statement":"The MCP action submits the target Requirement.",
                "status":"active",
                "refines":["req_shared"],
                "depends_on":[],
                "supersedes":[]
            }
        }),
    )
    .await;
    assert_ne!(requirement.is_error, Some(true), "{requirement:?}");
    assert_eq!(
        requirement.structured_content.as_ref().unwrap()["data"]["id"],
        "req_mcp_target"
    );
    assert_eq!(
        requirement.structured_content.as_ref().unwrap()["data"]["refines"],
        json!(["req_shared"])
    );
    let created_submission = requirement.structured_content.as_ref().unwrap()["data"]["decision"]
        ["pending"]["proposal_id"]
        .as_str()
        .expect("the MCP create response returns its submission identity")
        .to_owned();

    let dependency = call(
        &client,
        "change",
        json!({"action":"create",
            "target":"req_mcp_dependency",
            "type":"requirement",
            "data":{
                "actor":"agent",
                "statement":"The MCP dependency exists.",
                "status":"active",
                "depends_on":[],
                "supersedes":[]
            }
        }),
    )
    .await;
    assert_ne!(dependency.is_error, Some(true), "{dependency:?}");

    let edited_requirement = call(
        &client,
        "change",
        json!({"action":"update",
            "target":"req_mcp_target",
            "if_match":requirement.structured_content.as_ref().unwrap()["data"]["edit"]["etag"],
            "data":{
                "actor":"agent",
                "relationships":{"depends_on":{"add":["req_mcp_dependency"]}}
            }
        }),
    )
    .await;
    assert_ne!(
        edited_requirement.is_error,
        Some(true),
        "{edited_requirement:?}"
    );
    assert_eq!(
        edited_requirement.structured_content.as_ref().unwrap()["data"]["depends_on"],
        json!(["req_mcp_dependency"])
    );
    let edited = edited_requirement.structured_content.as_ref().unwrap();
    assert!(edited["data"]["decision"]["pending"]["proposal_id"].is_string());
    assert_ne!(
        edited["data"]["decision"]["pending"]["proposal_id"],
        created_submission
    );
    assert_eq!(
        edited["data"]["decision"]["pending"]["revision"],
        edited["data"]["edit"]["revision"]
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_target_first_requirement_submit_uses_the_target() {
    let repository = Repository::new("The shared graph is readable.");
    let store = repository.open().store();
    let scope = ScopeId::new("default").unwrap();
    let requirement = StableId::new("req_shared").unwrap();
    let proposal = store
        .requirement_decision_state(&scope, &requirement)
        .unwrap()
        .pending
        .unwrap()
        .proposal_id;
    store
        .withdraw_record_review(
            serde_json::from_value(
                json!({"scope_id":"default","actor":"agent","proposal_id":proposal}),
            )
            .unwrap(),
        )
        .unwrap();
    let host = support::resource_http::host(&repository, true);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let submitted = call(
        &client,
        "change",
        json!({"action":"submit",
            "target":"req_shared",
            "data":{
                "actor":"agent",
                "title":"MCP target",
                "summary":"The target-first action submits this Requirement.",
                "source_ids":[],
                "evidence_references":[],
                "builds_on":[]
            }
        }),
    )
    .await;
    assert_ne!(submitted.is_error, Some(true), "{submitted:?}");
    assert_eq!(
        submitted.structured_content.as_ref().unwrap()["data"]["requirement_id"],
        "req_shared"
    );
    assert_eq!(
        submitted.structured_content.as_ref().unwrap()["data"]["fact"],
        "submitted"
    );
    assert!(
        submitted.structured_content.as_ref().unwrap()["data"]["proposal_key"]
            .as_str()
            .unwrap()
            .starts_with("review:requirement:req_shared:")
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
#[verifies("rule_porcelain_existing_action_infers_kind", examples)]
#[verifies("rule_porcelain_named_domain_actions", examples)]
async fn mcp_named_actions_infer_kinds_and_refuse_invalid_targets() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = support::resource_http::host(&repository, true);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let claimed = call(
        &client,
        "change",
        json!({"action":"claim","target":"topic_shared","data":{"actor":"worker"}}),
    )
    .await;
    assert_ne!(claimed.is_error, Some(true), "{claimed:?}");
    assert_eq!(
        claimed.structured_content.as_ref().unwrap()["data"]["claimed_by"],
        "worker"
    );

    let released = call(
        &client,
        "change",
        json!({"action":"release","target":"topic_shared","data":{"actor":"worker"}}),
    )
    .await;
    assert_ne!(released.is_error, Some(true), "{released:?}");
    assert!(released.structured_content.as_ref().unwrap()["data"]["claimed_by"].is_null());

    let answered = call(
        &client,
        "change",
        json!({"action":"answer","target":"question_shared","data":{"answer":"The shared answer."}}),
    )
    .await;
    assert_ne!(answered.is_error, Some(true), "{answered:?}");
    assert_eq!(
        answered.structured_content.as_ref().unwrap()["data"]["status"],
        "answered"
    );

    let invalid = call(
        &client,
        "change",
        json!({"action":"claim","target":"question_shared","data":{"actor":"worker"}}),
    )
    .await;
    assert_eq!(invalid.is_error, Some(true), "{invalid:?}");

    let missing = call(
        &client,
        "change",
        json!({"action":"update","target":"record_missing","data":{"name":"Missing"}}),
    )
    .await;
    assert_eq!(missing.is_error, Some(true), "{missing:?}");
    assert_eq!(
        missing.structured_content.unwrap()["error"]["kind"],
        "not_found"
    );

    let kind_selector = call(
        &client,
        "change",
        json!({"action":"update","target":"source_shared","type":"invented","data":{"name":"Changed"}}),
    )
    .await;
    assert_eq!(kind_selector.is_error, Some(true), "{kind_selector:?}");

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

#[tokio::test]
async fn mcp_authoring_does_not_expose_or_run_hidden_operations() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = support::resource_http::host(&repository, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { provenance_mcp::serve(host, server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();

    let tools = client.list_all_tools().await.unwrap();
    assert!(!tools.iter().any(|tool| tool.name == "change"));
    assert!(!tools.iter().any(|tool| tool.name == "update"));
    let hidden = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("change").with_arguments(
                json!({"action":"update","target":"source_shared","data":{"name":"Hidden"}})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap_err();
    assert!(
        hidden.to_string().contains("Unknown tool: change"),
        "{hidden}"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
