#![cfg(feature = "test-fixture")]
#[path = "support/records.rs"]
#[allow(dead_code)]
mod records;
#[path = "support/writes.rs"]
mod writes;
use records::{call, host, Repository};
use serde_json::json;
use writes::{scoped, writable_host};

#[tokio::test]
async fn authoring_schema_failure_is_typed_and_preserves_graph() {
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let (status, failure) = call(
        &host,
        "apply",
        scoped(&json!({"schema_version":999,"spec":"test","declared_by":"test"})),
    )
    .await;
    assert_eq!(status, 400, "{failure}");
    assert_eq!(failure["error"]["kind"], "schema_version");
    host.shutdown().await;
}

#[test]
fn catalog_inventory_matches_the_existing_operation_contract() {
    let expected = [
        "check-statement",
        "create-source",
        "create-requirement",
        "create-resolution",
        "create-rule",
        "add-source-reference",
        "post-thread-message",
        "list-threads",
        "list-messages",
        "list-proposals",
        "list-dispositions",
        "list-assertions",
        "create-proposal",
        "create-assertion",
        "create-disposition",
        "info",
        "get",
        "search",
        "neighbors",
        "trace",
        "impact",
        "resolve-symbol",
        "evidence",
        "stale",
        "verification-runs",
        "verification-bindings",
        "plan",
        "apply",
        "begin-verification",
        "complete-verification",
    ]
    .into_iter()
    .collect::<std::collections::BTreeSet<_>>();
    let actual = provenance_store::operations::catalog::definitions()
        .into_iter()
        .map(|definition| definition.name)
        .collect();
    assert_eq!(expected, actual);
}

#[tokio::test]
async fn fixture_writes_require_explicit_opt_in() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&[("selected", &repo)], &["selected"]);
    let (status, failure) = call(&host, "apply", scoped(&json!({"schema_version":provenance_core::SUPPORTED_SCHEMA_VERSION.0,"spec":"test","declared_by":"test"}))).await;
    assert_eq!(status, 403, "{failure}");
    assert_eq!(failure["error"]["kind"], "access_denied");
    host.shutdown().await;
}

fn document() -> serde_json::Value {
    json!({"schema_version":provenance_core::SUPPORTED_SCHEMA_VERSION.0,"spec":"contract","declared_by":"fixture",
        "sources":[{"key":"policy","name":"Policy","kind":"github"}],
        "requirements":[{"key":"ready","statement":"The system is ready.","sources":["policy"]}],
        "rules":[{"key":"ready","requirement":"ready","statement":"The system is ready."}]})
}

#[tokio::test]
async fn http_plan_apply_and_verification_match_native_state() {
    use provenance_store::{operations, state_store::StateStore};
    let native = Repository::new("The shared graph is readable.");
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let scope = provenance_core::ScopeId::new("default").unwrap();
    let expected = operations::plan(
        Some(native.dir.path().to_str().unwrap().into()),
        &scope,
        serde_json::from_value(document()).unwrap(),
    )
    .unwrap();
    let (status, actual) = call(&host, "plan", scoped(&document())).await;
    assert_eq!(status, 200, "{actual}");
    assert_eq!(actual, serde_json::to_value(expected).unwrap());
    let expected = operations::apply(
        Some(native.dir.path().to_str().unwrap().into()),
        &scope,
        serde_json::from_value(document()).unwrap(),
    )
    .unwrap();
    let (status, actual) = call(&host, "apply", scoped(&document())).await;
    assert_eq!(status, 200, "{actual}");
    assert_eq!(actual, serde_json::to_value(expected).unwrap());
    let rule = actual["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["kind"] == "rule")
        .unwrap()["id"]
        .clone();
    std::fs::write(repo.dir.path().join("check.rs"), "fn check() {}\n").unwrap();
    let (status, run) = call(&host, "begin-verification", scoped(&json!({"rule":rule,"key":"ready","method":"examples","declared_by":"fixture","file":"check.rs"}))).await;
    assert_eq!(status, 200, "{run}");
    assert_eq!(run["status"], "running");
    let (status, done) = call(
        &host,
        "complete-verification",
        scoped(&json!({"run":run["id"],"status":"passed"})),
    )
    .await;
    assert_eq!(status, 200, "{done}");
    assert_eq!(done["status"], "passed");
    let store = StateStore::new(repo.layout.clone());
    assert_eq!(
        serde_json::to_value(store.list_verification_runs(&scope).unwrap()).unwrap(),
        json!([done])
    );
    assert_eq!(
        serde_json::to_value(store.list_rules(&scope).unwrap()).unwrap(),
        serde_json::to_value(StateStore::new(native.layout).list_rules(&scope).unwrap()).unwrap()
    );
    let (status, failure) = call(
        &host,
        "complete-verification",
        scoped(&json!({"run":run["id"],"status":"passed"})),
    )
    .await;
    assert_eq!(status, 409, "{failure}");
    assert_eq!(failure["error"]["kind"], "already_complete");
    host.shutdown().await;
}

#[tokio::test]
async fn native_context_kind_refuses_before_any_handler_reads() {
    use provenance_store::operations::catalog::{
        invoke_typed, CompleteVerification, PreparedContext, PreparedRepository,
    };
    let error = invoke_typed::<CompleteVerification>(
        PreparedContext::for_repository(PreparedRepository {
            root: "/missing-context-root".into(),
            requested_target: String::new(),
        }),
        serde_json::from_value(json!({"run":"verification_missing","status":"passed"})).unwrap(),
    )
    .await
    .unwrap_err();
    assert!(matches!(
        error,
        provenance_core::protocol::failure::OperationError::Common(
            provenance_core::protocol::failure::OperationFailure::UnavailableNeeds
        )
    ));
}

#[tokio::test]
async fn mcp_advertises_only_permitted_writes_and_persists_the_same_contract() {
    use rmcp::{model::CallToolRequestParams, ServiceExt};
    let repo = Repository::new("The shared graph is readable.");
    for writable in [false, true] {
        let host = if writable {
            writable_host(&repo)
        } else {
            host(&[("selected", &repo)], &["selected"])
        };
        let (client_io, server_io) = tokio::io::duplex(256 * 1024);
        let server_host = host.clone();
        let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
        let client = ().serve(client_io).await.unwrap();
        let tools = client.list_all_tools().await.unwrap();
        assert_eq!(tools.iter().any(|tool| tool.name == "apply"), writable);
        let result = client
            .call_tool(
                CallToolRequestParams::new("apply").with_arguments(
                    json!({"protocol_version":provenance_core::SDK_PROTOCOL_VERSION,"call":scoped(&document())}).as_object().unwrap().clone(),
                ),
            )
            .await
            .unwrap();
        let value = result.structured_content.unwrap();
        if writable {
            assert_ne!(result.is_error, Some(true));
            assert_eq!(value["created"], 3);
            let (status, plan) = call(&host, "plan", scoped(&document())).await;
            assert_eq!(status, 200, "{plan}");
            assert_eq!(plan["unchanged"], 3);
            std::fs::write(repo.dir.path().join("mcp-check.rs"), "fn check() {}\n").unwrap();
            let run = client
                .call_tool(CallToolRequestParams::new("begin-verification").with_arguments(
                    json!({"protocol_version":provenance_core::SDK_PROTOCOL_VERSION,"call":scoped(&json!({"rule":"rule_shared","key":"mcp","method":"examples","declared_by":"fixture","file":"mcp-check.rs"}))}).as_object().unwrap().clone(),
                ))
                .await
                .unwrap();
            assert_ne!(run.is_error, Some(true));
            let run = run.structured_content.unwrap();
            let complete = client
                .call_tool(CallToolRequestParams::new("complete-verification").with_arguments(
                    json!({"protocol_version":provenance_core::SDK_PROTOCOL_VERSION,"call":scoped(&json!({"run":run["id"],"status":"failed","error":"callback failure"}))}).as_object().unwrap().clone(),
                ))
                .await
                .unwrap();
            assert_ne!(complete.is_error, Some(true));
            let complete = complete.structured_content.unwrap();
            assert_eq!(complete["status"], "failed");
            let (_, runs) = call(&host, "verification-runs", scoped(&json!({}))).await;
            assert_eq!(runs, json!([complete]));
        } else {
            assert_eq!(result.is_error, Some(true));
            assert_eq!(value["error"]["kind"], "access_denied");
        }
        client.cancel().await.unwrap();
        server.await.unwrap().cancel().await.unwrap();
        host.shutdown().await;
    }
}

#[tokio::test]
async fn protocol_and_selected_file_refusals_precede_graph_changes() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let before = state_bytes(repo.dir.path());
    let response = host
        .router()
        .oneshot(
            Request::post(format!(
                "/v{}/operations/apply",
                provenance_core::SDK_PROTOCOL_VERSION + 1
            ))
            .header("host", "fixture.test")
            .header("authorization", "Bearer fixture-secret")
            .body(Body::from(scoped(&document()).to_string()))
            .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 400);
    let failure: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(failure["error"]["kind"], "protocol_mismatch");
    for path in ["../outside.rs", "/outside.rs", "missing.rs"] {
        let (status, _) = call(&host, "begin-verification", scoped(&json!({"rule":"rule_shared","key":"test","method":"examples","declared_by":"fixture","file":path}))).await;
        assert_ne!(status, 200);
        assert_eq!(state_bytes(repo.dir.path()), before);
    }
    assert_eq!(state_bytes(repo.dir.path()), before);
    host.shutdown().await;
}

fn state_bytes(root: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut result = std::collections::BTreeMap::new();
    for entry in std::fs::read_dir(root).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            result.extend(state_bytes(&entry.path()));
        } else {
            result.insert(entry.path(), std::fs::read(entry.path()).unwrap());
        }
    }
    result
}

#[tokio::test]
async fn statement_refusal_preserves_plan_diagnostics_and_canonical_bytes() {
    let repo = Repository::new("The shared graph is readable.");
    let host = writable_host(&repo);
    let mut input = document();
    input["requirements"][0]["statement"] = json!("Café; requirement");
    let before = state_bytes(repo.layout.state_dir().as_std_path());
    let (status, plan) = call(&host, "plan", scoped(&input.clone())).await;
    assert_eq!(status, 200, "{plan}");
    assert!(!plan["diagnostics"].as_array().unwrap().is_empty());
    let (status, refused) = call(&host, "apply", scoped(&input)).await;
    assert_eq!(status, 400, "{refused}");
    assert_eq!(refused["error"]["kind"], "statement_rejected");
    assert_eq!(refused["error"]["diagnostics"], plan["diagnostics"]);
    assert_eq!(state_bytes(repo.layout.state_dir().as_std_path()), before);
    host.shutdown().await;
}
