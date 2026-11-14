//! A line of the verification runs cache that a write of the cache refuses
//! gets one answer from each read and each write of the runs: the first line
//! at fault, as `invalid_state_file`.
#![cfg(feature = "test-fixture")]
mod support {
    pub mod records;
    pub mod resource_http;
}
use provenance_core::ScopeId;
use provenance_store::operations::catalog::Direct;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{access, call, host};

/// The routes of the runs and the body of each request.
fn routes() -> [(&'static str, &'static str, Option<Value>); 5] {
    let start = json!({"data": {
        "rule": "rule_shared", "key": "check", "method": "examples",
        "declared_by": "fixture", "file": "check.rs"
    }});
    [
        ("GET", "/verification-runs", None),
        ("GET", "/verification-runs/verification_1", None),
        ("GET", "/rules/rule_shared/evidence", None),
        ("POST", "/verification-runs/begin-verification", Some(start)),
        (
            "POST",
            "/verification-runs/verification_1/complete-verification",
            Some(json!({"data": {"status": "passed"}})),
        ),
    ]
}

/// The status and the error of each route and each SDK operation of the
/// runs, for a runs cache that holds `cache`.
async fn answers(cache: &[u8]) -> Vec<(String, u16, Value)> {
    let repo = Repository::new("The shared graph is readable.");
    std::fs::write(repo.dir.path().join("check.rs"), "fn check() {}\n").unwrap();
    let path = repo
        .layout
        .verification_runs_path(&ScopeId::new("default").unwrap());
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, cache).unwrap();
    let host = host(&repo, true);
    let mut answers = Vec::new();
    for (method, route, body) in routes() {
        let (status, answer) = call(&host, method, route, body).await;
        answers.push((format!("{method} {route}"), status, answer["error"].clone()));
    }
    for (operation, request) in [
        ("verification-runs", json!({})),
        ("list-verification-runs", Value::Null),
    ] {
        let call = json!({
            "context": {"repository": "selected", "scope": "default"},
            "request": request
        });
        let failure = provenance_contract::invoke_with(
            operation,
            provenance_core::SDK_PROTOCOL_VERSION,
            call,
            Direct(std::sync::Arc::new(access(&repo))),
        )
        .await
        .unwrap_err();
        answers.push((operation.to_owned(), failure.status_code(), failure.error));
    }
    answers
}

// No Rule binding: `rule_state_parse_refusal_names_the_line` is for record
// files below the state directory, and the runs cache is a cache file.
#[tokio::test]
async fn each_read_and_write_of_the_runs_names_the_first_bad_line_of_the_runs_cache() {
    let cases: [(&[u8], &str); 3] = [
        (b"{not json\n\xff\n", "key must be a string"),
        (b"\xff\n", "invalid utf-8 sequence of 1 bytes from index 0"),
        (b"{\"id\":\"verification_1\"}\n", "missing field `scope_id`"),
    ];
    for (cache, message) in cases {
        let answers = answers(cache).await;

        let error = json!({
            "kind": "invalid_state_file", "file": "verification-runs.jsonl", "line": 1,
            "message": message
        });
        let expected = answers
            .iter()
            .map(|(surface, _, _)| (surface.clone(), 500, error.clone()))
            .collect::<Vec<_>>();
        assert_eq!(answers, expected, "{message}");
    }
}
