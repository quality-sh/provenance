//! The MCP import action refuses a scope that holds a Requirement review record.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_macros::verifies;
use provenance_store::layout::ProvenanceLayout;
use provenance_transport::repository::RepoContext;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;
use support::mcp::{call, Session};
use support::records::Repository;

const REFUSAL: &str = "scope `default` holds a Requirement review record in \
     scopes/default/requirements/review.jsonl, which a scope export file cannot hold, so an \
     import into this scope and an export of it are not possible; read and change the \
     Requirements, Rules and other records of this scope with the routes that `provenance api \
     --scope default` lists";

/// A scope export with the Requirement `req_record` of `statement` and the Rule `rule_a`.
fn document(statement: &str) -> Value {
    json!({
        "scope": "default",
        "sources": [],
        "requirements": [
            {"scope_id": "default", "id": "req_record", "statement": statement,
             "status": "discovery"}
        ],
        "resolutions": [],
        "rules": [
            {"scope_id": "default", "id": "rule_a", "statement": "The system keeps the record.",
             "status": "draft", "severity": "medium", "requirement_ids": ["req_record"]}
        ],
        "threads": [],
        "messages": []
    })
}

/// Imports `document` through the MCP change tool.
async fn import(session: &Session, repository: &Repository, document: &Value) -> Value {
    let input = repository.dir.path().join("import.json");
    std::fs::write(&input, document.to_string()).unwrap();
    let arguments = json!({"action": "import", "input": input.to_str().unwrap()});
    let result = call(&session.client, "change", arguments).await;
    let content = result.structured_content.unwrap();
    match result.is_error {
        Some(true) => content["error"].clone(),
        _ => content,
    }
}

/// The bytes of each file and folder below the state folder of `repository`.
fn state_bytes(repository: &Repository) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut bytes = repository.bytes();
    bytes.retain(|path, _| path.starts_with(".provenance/state"));
    bytes
}

#[tokio::test]
#[verifies("rule_import_export_refuse_requirement_reviews", examples)]
async fn mcp_import_on_a_scope_with_a_requirement_review_record_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(dir.path().to_str().unwrap());
    let repository = Repository { dir, layout };
    support::records::allow_reviewer(&repository);
    let root = repository.dir.path().to_str().unwrap().to_owned();
    let session = Session::start(RepoContext::new(root, "default").host().unwrap()).await;
    let stored = import(&session, &repository, &document("The system keeps the record.")).await;
    assert_eq!(stored["records"], 2, "{stored}");
    let restated = import(
        &session,
        &repository,
        &document("The system keeps the record for one year."),
    )
    .await;
    assert_eq!(restated["records"], 2, "{restated}");
    let before = state_bytes(&repository);

    let refused = import(&session, &repository, &document("The system keeps the record.")).await;

    assert_eq!(
        (&refused["kind"], &refused["message"]),
        (&json!("invalid_update"), &json!(REFUSAL))
    );
    assert_eq!(state_bytes(&repository), before);
    session.close().await;
}
