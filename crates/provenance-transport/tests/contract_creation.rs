#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
#[allow(dead_code)]
mod records;
use records::{call, Repository};
use serde_json::{json, Value};

fn writable_host(repo: &Repository) -> provenance_transport::StatementHost {
    use provenance_transport::fixture::{FixtureAccess, Target};
    provenance_transport::StatementHost::with_fixture_access(
        FixtureAccess::new(
            vec![Target {
                id: "selected".into(),
                root: repo.dir.path().into(),
            }],
            vec![("selected".into(), "default".into())],
            "fixture-secret",
            "fixture.test",
        )
        .unwrap()
        .allow_writes(),
    )
}
fn scoped(request: Value) -> Value {
    let mut call = json!({"context":{"repository":"selected","scope":"default"}});
    call["request"] = request;
    call
}
fn source() -> Value {
    json!({"scope_id":"default","id":"source_created","name":"Policy","source_type":"policy","supersedes":[],"origin_thread":"thread_origin","origin_message":"message_origin"})
}
#[tokio::test]
async fn creation_preserves_native_source_and_rejects_duplicate_and_scope_escape() {
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let (status, result) = call(&host, "create-source", scoped(source())).await;
    assert_eq!(status, 200, "{result}");
    assert_eq!(result["origin_thread"], "thread_origin");
    assert_eq!(result["origin_message"], "message_origin");
    let (status, failure) = call(&host, "create-source", scoped(source())).await;
    assert_eq!(status, 409, "{failure}");
    assert_eq!(failure["error"]["kind"], "already_exists");
    let mut input = source();
    input["scope_id"] = json!("other");
    let (status, failure) = call(&host, "create-source", scoped(input)).await;
    assert_eq!(status, 400, "{failure}");
    assert_eq!(failure["error"]["kind"], "scope_mismatch");
    host.shutdown().await;
}

#[tokio::test]
async fn creation_and_attachment_match_native_records() {
    use provenance_store::state_store::StateStore;
    let native = Repository::new("The shared graph is readable.");
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let store = StateStore::new(native.layout.clone());
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let requests = [
        ("create-source", source()),
        (
            "create-requirement",
            json!({"scope_id":"default","id":"req_created","statement":"The system is ready.","status":"discovery","refines":"req_shared","depends_on":[],"supersedes":[]}),
        ),
        (
            "create-resolution",
            json!({"scope_id":"default","id":"res_created","title":"Decision","requirement_ids":["req_created"],"supersedes":[],"position":"Use the existing record.","rationale":"The shape is fixed.","status":"draft","inputs":[]}),
        ),
        (
            "create-rule",
            json!({"scope_id":"default","id":"rule_created","requirement_ids":["req_created","req_created"],"resolution_ids":["res_created"],"statement":"The system is ready.","status":"draft","severity":"high"}),
        ),
        (
            "add-source-reference",
            json!({"scope_id":"default","source_id":"source_created","requirement_id":"req_created","clause":"1"}),
        ),
        (
            "add-source-reference",
            json!({"scope_id":"default","source_id":"source_created","requirement_id":"req_created","clause":"1"}),
        ),
    ];
    for (operation, request) in requests {
        let expected = native_creation(&store, operation, request.clone());
        let (status, result) = call(&host, operation, scoped(request)).await;
        assert_eq!(status, 200, "{operation}: {result}");
        assert_eq!(result, expected);
    }
    host.shutdown().await;
    let actual = StateStore::new(repo.layout);
    assert_eq!(
        serde_json::to_value(actual.list_requirements(&scope).unwrap()).unwrap(),
        serde_json::to_value(store.list_requirements(&scope).unwrap()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(actual.list_sources(&scope).unwrap()).unwrap(),
        serde_json::to_value(store.list_sources(&scope).unwrap()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(actual.list_rules(&scope).unwrap()).unwrap(),
        serde_json::to_value(store.list_rules(&scope).unwrap()).unwrap()
    );
    assert_eq!(
        serde_json::to_value(actual.list_resolutions(&scope).unwrap()).unwrap(),
        serde_json::to_value(store.list_resolutions(&scope).unwrap()).unwrap()
    );
}

#[tokio::test]
async fn known_creation_refusals_preserve_graph_records() {
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let cases = [
        (
            "create-source",
            {
                let mut input = source();
                input["commit_pin"] = json!("bad");
                input
            },
            "invalid_commit_pin",
        ),
        (
            "create-source",
            {
                let mut input = source();
                input["supersedes"] = json!(["source_absent"]);
                input
            },
            "missing_reference",
        ),
        (
            "create-requirement",
            json!({"scope_id":"default","id":"req_bad","statement":"Stop; wait.","status":"discovery","depends_on":[],"supersedes":[]}),
            "statement_invalid",
        ),
        (
            "create-rule",
            json!({"scope_id":"default","id":"rule_bad","statement":"The system is ready.","status":"draft","severity":"high","requirement_ids":[],"resolution_ids":[]}),
            "missing_reference",
        ),
        (
            "create-resolution",
            json!({"scope_id":"default","id":"res_bad","title":"Decision","requirement_ids":[],"supersedes":[],"position":"Use the existing record.","rationale":"The shape is fixed.","status":"draft","inputs":[]}),
            "missing_reference",
        ),
        (
            "add-source-reference",
            json!({"scope_id":"default","source_id":"source_absent","requirement_id":"req_shared"}),
            "missing_reference",
        ),
    ];
    let snapshot = || {
        let store = provenance_store::state_store::StateStore::new(repo.layout.clone());
        let scope = provenance_core::ScopeId::new("default").unwrap();
        json!([
            store.list_sources(&scope).unwrap(),
            store.list_requirements(&scope).unwrap(),
            store.list_rules(&scope).unwrap(),
            store.list_resolutions(&scope).unwrap()
        ])
    };
    let before = snapshot();
    for (operation, input, kind) in cases {
        let (status, failure) = call(&host, operation, scoped(input)).await;
        assert_eq!(status, 400, "{operation}: {failure}");
        assert_eq!(failure["error"]["kind"], kind);
        assert_eq!(snapshot(), before);
    }
    host.shutdown().await;
}

#[tokio::test]
async fn mcp_creation_uses_explicit_write_grants_and_native_records() {
    use rmcp::{model::CallToolRequestParam, ServiceExt};
    let repo = Repository::new("The shared graph is readable.");
    for writable in [false, true] {
        let host = if writable {
            writable_host(&repo)
        } else {
            records::host(&[("selected", &repo)], &["selected"])
        };
        let (client_io, server_io) = tokio::io::duplex(256 * 1024);
        let server_host = host.clone();
        let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
        let client = ().serve(client_io).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();
        assert_creation_tools(&tools, writable);
        let result = client
            .call_tool(CallToolRequestParam {
                name: "create-source".into(),
                arguments: Some(
                    json!({"protocol_version":7,"call":scoped(source())})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            })
            .await
            .unwrap();
        let result_value = result.structured_content.unwrap();
        if writable {
            assert_ne!(result.is_error, Some(true));
            let store = provenance_store::state_store::StateStore::new(repo.layout.clone());
            let sources = store
                .list_sources(&provenance_core::ScopeId::new("default").unwrap())
                .unwrap();
            let source = sources
                .iter()
                .find(|source| source.id.as_str() == "source_created")
                .unwrap();
            assert_eq!(result_value, serde_json::to_value(source).unwrap());
            for (name, request, id) in [
                (
                    "create-requirement",
                    json!({"scope_id":"default","id":"req_mcp","statement":"The system is ready.","status":"discovery","depends_on":[],"supersedes":[]}),
                    "req_mcp",
                ),
                (
                    "create-resolution",
                    json!({"scope_id":"default","id":"res_mcp","title":"Decision","requirement_ids":["req_mcp"],"supersedes":[],"position":"Use the existing record.","rationale":"The shape is fixed.","status":"draft","inputs":[]}),
                    "res_mcp",
                ),
                (
                    "create-rule",
                    json!({"scope_id":"default","id":"rule_mcp","statement":"The system is ready.","status":"draft","severity":"high","requirement_ids":["req_mcp"],"resolution_ids":["res_mcp"]}),
                    "rule_mcp",
                ),
                (
                    "add-source-reference",
                    json!({"scope_id":"default","source_id":"source_created","requirement_id":"req_mcp","clause":"1"}),
                    "req_mcp",
                ),
            ] {
                let result = client
                    .call_tool(CallToolRequestParam {
                        name: name.into(),
                        arguments: Some(
                            json!({"protocol_version":7,"call":scoped(request)})
                                .as_object()
                                .unwrap()
                                .clone(),
                        ),
                    })
                    .await
                    .unwrap();
                assert_ne!(result.is_error, Some(true), "{result:?}");
                assert_eq!(result.structured_content.unwrap()["id"], id);
            }
            let requirement = store
                .list_requirements(&provenance_core::ScopeId::new("default").unwrap())
                .unwrap()
                .into_iter()
                .find(|value| value.id.as_str() == "req_mcp")
                .unwrap();
            assert_eq!(requirement.source_refs.len(), 1);
            assert_eq!(
                requirement.source_refs[0].source_id.as_str(),
                "source_created"
            );
        } else {
            assert_eq!(result.is_error, Some(true));
            assert_eq!(result_value["error"]["kind"], "access_denied");
        }
        client.cancel().await.unwrap();
        server.await.unwrap().cancel().await.unwrap();
        host.shutdown().await;
    }
}

fn native_creation(
    store: &provenance_store::state_store::StateStore,
    operation: &str,
    request: Value,
) -> Value {
    match operation {
        "create-source" => serde_json::to_value(
            store
                .create_source(serde_json::from_value(request).unwrap())
                .unwrap(),
        )
        .unwrap(),
        "create-requirement" => serde_json::to_value(
            store
                .create_requirement(serde_json::from_value(request).unwrap())
                .unwrap(),
        )
        .unwrap(),
        "create-resolution" => serde_json::to_value(
            store
                .create_resolution(serde_json::from_value(request).unwrap())
                .unwrap(),
        )
        .unwrap(),
        "create-rule" => serde_json::to_value(
            store
                .create_rule(serde_json::from_value(request).unwrap())
                .unwrap(),
        )
        .unwrap(),
        "add-source-reference" => serde_json::to_value(
            store
                .add_source_reference(serde_json::from_value(request).unwrap())
                .unwrap(),
        )
        .unwrap(),
        _ => panic!("unknown fixture operation"),
    }
}
fn assert_creation_tools(tools: &[rmcp::model::Tool], writable: bool) {
    for name in [
        "create-source",
        "create-requirement",
        "create-resolution",
        "create-rule",
        "add-source-reference",
    ] {
        assert_eq!(tools.iter().any(|tool| tool.name == name), writable);
    }
}
