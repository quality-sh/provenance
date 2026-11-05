//! The read tools refuse each request that writes, and no call of a read tool changes
//! the repository.
#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/check_port.rs"]
    pub mod check_port;
    pub mod fixture_host;
    pub mod mcp;
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
    pub mod schema_check;
}

use provenance_macros::verifies;
use serde_json::{json, Value};
use support::{mcp::Session, records::Repository};

/// The error object of a refused call.
async fn refused(session: &Session, name: &str, arguments: Value) -> Value {
    let result = support::schema_check::call(&session.client, name, arguments.clone()).await;
    assert_eq!(result.is_error, Some(true), "{name}: {arguments}");
    result.structured_content.unwrap()["error"].clone()
}

/// The routes of the api catalog: method, path with each path parameter filled, and
/// whether the registered operation of the route writes.
async fn routes(session: &Session) -> Vec<(String, String, bool)> {
    let catalog = support::schema_check::call(&session.client, "api", json!({})).await;
    let catalog = catalog.structured_content.unwrap();
    let routes = catalog["routes"]
        .as_array()
        .expect("the catalog lists routes");
    assert_ne!(routes.len(), 0);
    routes
        .iter()
        .map(|route| {
            let method = route["method"].as_str().unwrap().to_owned();
            let template = route["path"].as_str().unwrap();
            let writes = provenance_contract::definitions()
                .iter()
                .find(|definition| {
                    definition.method.as_str() == method
                        && definition.path.trim_start_matches('/')
                            == template.trim_start_matches('/')
                })
                .unwrap_or_else(|| panic!("{method} {template} has no definition"))
                .mutates();
            let path = template
                .split('/')
                .map(|part| {
                    if part.starts_with('{') {
                        "req_shared"
                    } else {
                        part
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
            (method, path, writes)
        })
        .collect()
}

/// The paths below the repository whose bytes differ. The projection cache is left out:
/// a read can bring that derived copy of the store up to date.
fn changed(
    before: &std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    repository: &Repository,
) -> Vec<std::path::PathBuf> {
    let after = repository.bytes();
    after
        .keys()
        .chain(before.keys())
        .filter(|path| !path.starts_with(".provenance/cache"))
        .filter(|path| after.get(*path) != before.get(*path))
        .cloned()
        .collect()
}

#[tokio::test]
#[verifies("rule_mcp_tools_read_or_write", examples)]
async fn get_refuses_notify_and_names_the_change_tool() {
    let repository = Repository::new("The shared graph is readable.");
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let before = repository.bytes();
    let error = refused(
        &session,
        "get",
        json!({"target": "req_shared", "view": "review",
               "notify": {"client": "codex", "thread": "thread-1"}}),
    )
    .await;
    assert_eq!(
        (&error["field"], &error["reason"]),
        (&json!("notify"), &json!("unknown_field")),
        "{error}"
    );
    assert!(
        error["message"].as_str().unwrap().contains("action notify"),
        "{error}"
    );
    assert_eq!(
        changed(&before, &repository),
        Vec::<std::path::PathBuf>::new()
    );
    session.close().await;
}

#[tokio::test]
#[verifies("rule_mcp_tools_read_or_write", examples)]
async fn api_refuses_each_route_that_writes_and_names_the_change_tool() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let before = repository.bytes();
    let routes = routes(&session).await;
    let writes = routes.iter().filter(|(_, _, writes)| *writes).count();
    assert!(writes > 10, "{routes:?}");
    for (method, path, writes) in &routes {
        let result = support::schema_check::call(
            &session.client,
            "api",
            json!({"method": method, "path": path, "body": {}}),
        )
        .await;
        let error = &result.structured_content.unwrap()["error"];
        let names_change = error["message"]
            .as_str()
            .is_some_and(|message| message.contains("call it with the change tool, action api"));
        if *writes {
            assert_eq!(
                (&error["field"], &error["reason"], names_change),
                (&json!("path"), &json!("invalid_value"), true),
                "{method} {path}: {error}"
            );
        } else {
            assert!(!names_change, "{method} {path}: {error}");
        }
    }
    assert_eq!(
        changed(&before, &repository),
        Vec::<std::path::PathBuf>::new()
    );
    session.close().await;
}

/// Each read tool with each of its fields set.
fn read_calls() -> Vec<(&'static str, Value)> {
    let parent = json!({"node_type": "requirement", "node_id": "req_shared"});
    let mut calls = Vec::new();
    for view in ["record", "children", "trace", "impact", "review"] {
        calls.push((
            "get",
            json!({"target": "req_shared", "view": view, "limit": 5, "max_depth": 2,
                   "returned_kinds": ["requirement", "source"]}),
        ));
    }
    calls.extend([
        (
            "trace",
            json!({"target": "req_shared", "limit": 5, "max_depth": 2,
                   "returned_kinds": ["source"]}),
        ),
        (
            "search",
            json!({"text": "shared", "node_types": ["requirement"], "limit": 5,
                   "exclude_terminal": true}),
        ),
        (
            "search",
            json!({"file": "src/lib.rs", "symbol": "run", "line": 1, "limit": 5,
                   "exclude_terminal": false}),
        ),
        (
            "check",
            json!({"categories": ["graph", "statements", "bindings", "gaps"],
                   "statement": "The pump stops."}),
        ),
        (
            "discussions",
            json!({"action": "discussions", "parent": parent, "status": "all", "limit": 5}),
        ),
        (
            "discussions",
            json!({"action": "discussion", "discussion_id": "discussion_x", "limit": 5}),
        ),
        (
            "api",
            json!({"method": "get", "path": "requirements/req_shared", "query": {},
                   "headers": {}}),
        ),
        ("api", json!({})),
    ]);
    calls
}

#[tokio::test]
#[verifies("rule_mcp_tools_read_or_write", examples)]
async fn no_read_tool_changes_the_repository() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let tools = session.client.list_all_tools().await.unwrap();
    let mut called = std::collections::BTreeSet::new();
    let before = repository.bytes();
    for (name, arguments) in read_calls() {
        support::schema_check::call(&session.client, name, arguments.clone()).await;
        let changed = changed(&before, &repository);
        assert_eq!(
            changed,
            Vec::<std::path::PathBuf>::new(),
            "{name} {arguments}"
        );
        called.insert(name);
    }
    for (method, path, writes) in routes(&session).await {
        if !writes {
            let arguments = json!({"method": method, "path": path, "body": {}});
            support::schema_check::call(&session.client, "api", arguments).await;
        }
    }
    assert_eq!(
        changed(&before, &repository),
        Vec::<std::path::PathBuf>::new()
    );
    let read_tools = tools
        .iter()
        .filter(|tool| tool.name != "change")
        .map(|tool| tool.name.as_ref())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(read_tools, called);
    session.close().await;
}
