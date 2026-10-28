#![cfg(feature = "test-fixture")]
#[allow(dead_code)]
mod support {
    pub mod records;
    pub mod resource_http;
}
use axum::{body::Body, http::Request};
use provenance_core::protocol::failure::OperationFailure;
use provenance_store::operations::catalog::{
    ContextResolver, ExecutionNeeds, PreparedContext, RequestedContext,
};
use provenance_transport::{HostAccess, ReviewPageWrite};
use serde_json::json;
use std::sync::{Arc, Mutex};
use support::records::Repository;
use tower::ServiceExt as _;

// Implementation aid: a host policy changes the read settings after capture
// starts. Canonical writes do not need these settings, but notification reads do.
struct UnreadableSettings {
    access: provenance_transport::fixture::FixtureAccess,
    settings: std::path::PathBuf,
}
impl ContextResolver for UnreadableSettings {
    fn authorize(
        &self,
        operation: &'static str,
        context: &RequestedContext,
    ) -> Result<(), OperationFailure> {
        self.access.authorize(operation, context)
    }

    fn prepare(
        &self,
        operation: &'static str,
        context: RequestedContext,
        needs: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        let prepared = self.access.prepare(operation, context, needs)?;
        std::fs::write(&self.settings, "invalid read settings").unwrap();
        Ok(prepared)
    }
}
impl HostAccess for UnreadableSettings {
    fn advertises(&self, operation: &str) -> bool {
        self.access.advertises(operation)
    }
    fn bound_identity(&self) -> Option<(String, String)> {
        self.access.bound_identity()
    }
}

#[test]
fn a_saved_page_write_succeeds_when_the_snapshot_fails() {
    bounded(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            use provenance_transport::{
                fixture::{FixtureAccess, Target},
                StatementHost,
            };
            let repo = Repository::new("The graph is readable.");
            let access = FixtureAccess::new(
                vec![Target {
                    id: "selected".into(),
                    root: repo.dir.path().to_path_buf(),
                }],
                vec![("selected".into(), "default".into())],
                "fixture-secret",
                "fixture.test",
            )
            .unwrap()
            .allow_writes();
            let notifications = Arc::new(Mutex::new(Vec::new()));
            let seen = notifications.clone();
            let host = StatementHost::with_access(Arc::new(UnreadableSettings {
                access,
                settings: repo.dir.path().join(".provenance/settings.json"),
            }))
            .with_credential(Arc::new(support::resource_http::access(&repo)))
            .with_page_writes(
                repo.open(),
                Arc::new(move |changes| seen.lock().unwrap().push(changes)),
            );
            let mut request = Request::builder()
                .method("POST")
                .uri("/rules")
                .header("host", "fixture.test")
                .header("authorization", "Bearer fixture-secret")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"data":{
                        "id":"rule_saved", "statement":"The saved write succeeds.", "status":"draft",
                        "severity":"high", "requirement_ids":["req_shared"], "resolution_ids":[]
                    }})
                    .to_string(),
                ))
                .unwrap();
            request.extensions_mut().insert(ReviewPageWrite);
            let response = host.router().oneshot(request).await.unwrap();
            let status = response.status();
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            assert!(std::fs::read_to_string(
                repo.dir
                    .path()
                    .join(".provenance/state/scopes/default/rules/rule.jsonl")
            )
            .unwrap()
            .contains("rule_saved"));
            assert_eq!(status.as_u16(), 200, "{}", String::from_utf8_lossy(&body));
            let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(value["data"]["id"], "rule_saved");
            let notifications = notifications.lock().unwrap();
            assert_eq!(notifications.len(), 1);
            assert!(notifications[0].is_err());
            drop(notifications);
        });
    });
}

// Implementation aid: pins the observer payload through public HTTP writes.
// No Rule specifies the internal record-family notification payload.
#[test]
fn saved_page_writes_report_added_changed_and_unchanged_records() {
    bounded(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let repo = Repository::new("The graph is readable.");
            let notifications = Arc::new(Mutex::new(Vec::new()));
            let seen = notifications.clone();
            let host = support::resource_http::host(&repo, true).with_page_writes(
                repo.open(),
                Arc::new(move |changes| seen.lock().unwrap().push(changes)),
            );
            let created = page_call(
                &host,
                "POST",
                "/rules",
                json!({
                    "id":"rule_saved", "statement":"The saved write succeeds.", "status":"draft",
                    "severity":"high", "requirement_ids":["req_shared"], "resolution_ids":[]
                }),
            )
            .await;
            assert_eq!(created["data"]["id"], "rule_saved");
            assert_notification(&notifications, &[("rule", "rule_saved")]);

            let changed = page_call(
                &host,
                "PATCH",
                "/rules/rule_saved",
                json!({
                    "statement":"The changed write succeeds."
                }),
            )
            .await;
            assert_eq!(changed["data"]["statement"], "The changed write succeeds.");
            assert_notification(&notifications, &[("rule", "rule_saved")]);

            page_call(
                &host,
                "PATCH",
                "/rules/rule_saved",
                json!({
                    "statement":"The changed write succeeds."
                }),
            )
            .await;
            assert_notification(&notifications, &[]);
        });
    });
}

// Implementation aid: a Discussion must also invalidate its parent document.
#[test]
fn saved_page_discussion_reports_its_parent_and_message() {
    bounded(|| {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let repo = Repository::new("The graph is readable.");
            let notifications = Arc::new(Mutex::new(Vec::new()));
            let seen = notifications.clone();
            let host = support::resource_http::host(&repo, true).with_page_writes(
                repo.open(),
                Arc::new(move |changes| seen.lock().unwrap().push(changes)),
            );
            let created = page_call(
                &host,
                "POST",
                "/requirements/req_shared/discussions",
                json!({
                    "actor":"reviewer", "declared_by":null, "role":"user", "body":"Check the text."
                }),
            )
            .await;
            let discussion = created["data"]["discussion_id"].as_str().unwrap();
            let message = created["data"]["root_message_id"].as_str().unwrap();
            let thread = created["data"]["thread_id"].as_str().unwrap();
            assert_notification(
                &notifications,
                &[
                    ("threads", thread),
                    ("discussions", discussion),
                    ("messages", message),
                    ("review", "req_shared"),
                ],
            );
        });
    });
}

type Notifications = Mutex<Vec<anyhow::Result<provenance_store::review_changes::ReviewChanges>>>;

fn assert_notification(notifications: &Notifications, expected: &[(&str, &str)]) {
    let mut notifications = notifications.lock().unwrap();
    assert_eq!(notifications.len(), 1);
    let changes = notifications.pop().unwrap().unwrap();
    drop(notifications);
    assert!(!changes.configuration_changed);
    let mut actual = Vec::new();
    for scope in &changes.scopes {
        assert_eq!(scope.scope, "default");
        actual.extend(
            scope
                .records
                .iter()
                .map(|record| (record.kind.as_str(), record.id.as_str())),
        );
    }
    actual.sort_unstable();
    let mut expected = expected.to_vec();
    expected.sort_unstable();
    assert_eq!(actual, expected, "{changes:?}");
}

async fn page_call(
    host: &provenance_transport::StatementHost,
    method: &str,
    path: &str,
    data: serde_json::Value,
) -> serde_json::Value {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "fixture.test")
        .header("authorization", "Bearer fixture-secret")
        .header("content-type", "application/json")
        .body(Body::from(json!({"data":data}).to_string()))
        .unwrap();
    request.extensions_mut().insert(ReviewPageWrite);
    let response = host.router().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(status.as_u16(), 200, "{value}");
    value
}

// Implementation aid: canonical scope replacement can remove records without an
// HTTP delete route. Public store snapshots must retain those notification IDs.
#[test]
fn page_changes_report_removed_records_after_scope_replacement() {
    bounded(|| {
        use provenance_core::ScopeId;
        use provenance_store::{
            publication::{with_publication_guard, with_staged_state},
            review_changes::ReviewStateSnapshot,
            state_store::{ScopeShards, StateStore},
        };
        let repo = Repository::new("The graph is readable.");
        let scope = ScopeId::new("default").unwrap();
        let clear = || {
            with_staged_state(&repo.layout, false, |layout| {
                StateStore::new(layout.clone()).import_scope(&scope, &ScopeShards::default())
            })
            .unwrap();
        };
        clear();
        StateStore::new(repo.layout.clone())
            .create_source(
                serde_json::from_value(json!({
                    "scope_id":"default", "id":"source_removed", "name":"The removed source",
                    "source_type":"document", "supersedes":[]
                }))
                .unwrap(),
            )
            .unwrap();
        let before = with_publication_guard(&repo.open(), ReviewStateSnapshot::read).unwrap();
        clear();
        let after = with_publication_guard(&repo.open(), ReviewStateSnapshot::read).unwrap();
        assert_notification(
            &Mutex::new(vec![Ok(after.page_changes_since(&before))]),
            &[("source", "source_removed")],
        );
        assert_notification(&Mutex::new(vec![Ok(after.page_changes_since(&after))]), &[]);
    });
}

// A separate thread also bounds synchronous filesystem and OS lock waits.
fn bounded(test: impl FnOnce() + Send + 'static) {
    let (sender, receiver) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        test();
        let _ = sender.send(());
    });
    if receiver.recv_timeout(std::time::Duration::from_secs(45))
        == Err(std::sync::mpsc::RecvTimeoutError::Timeout)
    {
        panic!("the page-write test must finish within 45 seconds");
    }
    if let Err(panic) = worker.join() {
        std::panic::resume_unwind(panic);
    }
}

// Implementation aid: preserve assertion diagnostics from the bounded test thread.
#[test]
fn bounded_preserves_the_test_panic() {
    let panic = std::panic::catch_unwind(|| bounded(|| panic!("original test failure")))
        .expect_err("the bounded test must propagate its panic");
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"original test failure"));
}
