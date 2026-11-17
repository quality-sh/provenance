//! The api tool refuses a completion whose verification run row passes the
//! byte budget of the reads of the runs, and the run stays running.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_core::{VerificationRun, VerificationRunStatus};
use provenance_macros::verifies;
use rmcp::model::CallToolResult;
use serde_json::{json, Value};
use support::api_session::ApiSession;
use support::records::Repository;
use support::resource_http::host;

/// The bytes of one run row that the reads of the runs give. The store owns
/// the number in `read_budget::resource_record_bytes`.
const BUDGET: usize = 1_048_576 - 16_384;

/// The data of one tool result that is not an error.
fn data(result: &CallToolResult) -> Value {
    assert_ne!(result.is_error, Some(true), "{result:?}");
    result.structured_content.as_ref().unwrap()["data"].clone()
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn the_api_tool_refuses_a_completion_with_a_row_over_the_budget() {
    let repo = Repository::new("The evidence is readable.");
    std::fs::write(repo.dir.path().join("check.rs"), "fn check() {}\n").unwrap();
    let session = ApiSession::start(host(&repo, true)).await;
    let start = json!({
        "rule": "rule_shared", "key": "check", "method": "examples",
        "declared_by": "fixture", "file": "check.rs"
    });
    let started = session
        .call(json!({
            "path": "verification-runs/begin-verification",
            "method": "post",
            "input": start
        }))
        .await;
    let run = data(&started);
    let id = run["id"].as_str().unwrap();
    let mut failed: VerificationRun = serde_json::from_value(run.clone()).unwrap();
    failed.status = VerificationRunStatus::Failed;
    failed.completed_at = Some(failed.started_at);
    failed.error = Some(String::new());
    let error = "x".repeat(BUDGET + 1 - serde_json::to_vec(&failed).unwrap().len());

    let refused = session
        .call(json!({
            "path": format!("verification-runs/{id}/complete-verification"),
            "method": "post",
            "input": {"status": "failed", "error": error}
        }))
        .await;
    let member = session
        .call(json!({"path": format!("verification-runs/{id}")}))
        .await;

    assert_eq!(refused.is_error, Some(true));
    let refusal = refused.structured_content.as_ref().unwrap();
    assert_eq!(refusal["error"]["kind"], "record_too_large");
    assert_eq!(data(&member), run);
    session.shutdown().await;
}
