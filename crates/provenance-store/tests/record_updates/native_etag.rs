//! Examples of the etag that the native Requirement update gets.

use super::support::Fixture;
use provenance_store::state_store::UpdateRequirementInput;
use provenance_store::write_error::WriteError;
use serde_json::{json, Value};

fn current_etag(fixture: &Fixture) -> String {
    let id = provenance_core::StableId::new("req_one").unwrap();
    fixture
        .store
        .requirement_edit_state(&fixture.scope, &id)
        .unwrap()
        .etag
}

fn stored(fixture: &Fixture) -> provenance_core::Requirement {
    fixture.store.list_requirements(&fixture.scope).unwrap()[0].clone()
}

/// A native update of the description, on `etag` when it is given.
fn update(etag: Option<&str>, description: &str) -> Value {
    let mut request = json!({"scope_id":"default","id":"req_one","description":description});
    if let Some(etag) = etag {
        request["expected_etag"] = json!(etag);
    }
    request
}

/// The refusal of a native update through the Rust API: the text, the status
/// and the safe body.
fn refusal(fixture: &Fixture, request: Value) -> (String, u16, Value) {
    let input: UpdateRequirementInput = serde_json::from_value(request).unwrap();
    let error = WriteError(fixture.store.update_requirement(input).unwrap_err());
    let body = serde_json::to_value(error.safe()).unwrap();
    (error.0.to_string(), error.status(), body)
}

// No Rule: a characterization test of the refusal that wins for an input with
// an etag, which is green on the production code of the base commit.
#[tokio::test]
async fn a_native_update_with_an_etag_in_an_unknown_scope_is_refused_as_before() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let etag = current_etag(&fixture);
    let before = stored(&fixture);
    let mut request = update(Some(&etag), "Other scope");
    request["scope_id"] = json!("scope_unknown");
    assert_eq!(
        refusal(&fixture, request),
        (
            "requirement req_one does not exist uniquely in this scope".to_owned(),
            404,
            json!({"kind":"resource_not_found"}),
        )
    );
    assert_eq!(stored(&fixture), before);
}

// No Rule: a characterization test of the refusal that wins for an input with
// an etag, which is green on the production code of the base commit.
#[tokio::test]
async fn a_native_update_with_an_etag_of_a_missing_requirement_is_refused_as_before() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let etag = current_etag(&fixture);
    let before = stored(&fixture);
    let mut request = update(Some(&etag), "Missing record");
    request["id"] = json!("req_missing");
    assert_eq!(
        refusal(&fixture, request),
        (
            "requirement req_missing does not exist uniquely in this scope".to_owned(),
            404,
            json!({"kind":"resource_not_found"}),
        )
    );
    assert_eq!(stored(&fixture), before);
}

// No Rule: no active Rule states that the native Requirement update checks
// the etag that it gets.
#[tokio::test]
async fn a_native_requirement_update_on_the_current_etag_succeeds() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let etag = current_etag(&fixture);
    let updated = fixture
        .call("update-requirement-native", update(Some(&etag), "Current"))
        .await
        .unwrap();
    assert_eq!(updated["description"], "Current");
    assert_eq!(stored(&fixture).description.as_deref(), Some("Current"));
    assert_ne!(current_etag(&fixture), etag);
}

// No Rule: no active Rule states how the native Requirement update saves an
// input with no etag.
#[tokio::test]
async fn native_requirement_updates_with_no_etag_save_on_the_current_etag() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    for description in ["First writer", "Second writer"] {
        let updated = fixture
            .call("update-requirement-native", update(None, description))
            .await
            .unwrap();
        assert_eq!(updated["description"], description);
    }
    assert_eq!(
        stored(&fixture).description.as_deref(),
        Some("Second writer")
    );
}

// No Rule: no active Rule states that the native Requirement update checks
// the etag that it gets.
#[tokio::test]
async fn a_stale_native_update_gets_the_refusal_of_the_public_update() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let etag = current_etag(&fixture);
    fixture
        .call(
            "update-requirement-native",
            update(Some(&etag), "First writer"),
        )
        .await
        .unwrap();
    let after_first = current_etag(&fixture);
    let body = json!({"kind":"requirement_edit_conflict","current_etag":after_first});
    let native = fixture
        .call(
            "update-requirement-native",
            update(Some(&etag), "Second writer"),
        )
        .await;
    assert_eq!(native, Err(body.clone()));
    assert_eq!(
        refusal(&fixture, update(Some(&etag), "Second writer")),
        ("stale Requirement edit etag".to_owned(), 409, body.clone())
    );
    let public = fixture
        .call(
            "update-requirement",
            json!({"actor":"reviewer","id":"req_one","expected_etag":etag,"description":"Second writer"}),
        )
        .await;
    assert_eq!(public, Err(body));
    assert_eq!(
        stored(&fixture).description.as_deref(),
        Some("First writer")
    );
    assert_eq!(current_etag(&fixture), after_first);
}

// No Rule: no active Rule states that the native Requirement update checks
// the etag that it gets.
#[tokio::test]
async fn a_stale_native_update_with_no_content_change_succeeds_with_no_write() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let etag = current_etag(&fixture);
    fixture
        .call(
            "update-requirement-native",
            update(Some(&etag), "First writer"),
        )
        .await
        .unwrap();
    let before = stored(&fixture);
    let after_first = current_etag(&fixture);
    let updated = fixture
        .call(
            "update-requirement-native",
            update(Some(&etag), "First writer"),
        )
        .await
        .unwrap();
    assert_eq!(updated["description"], "First writer");
    assert_eq!(stored(&fixture), before);
    assert_eq!(current_etag(&fixture), after_first);
}
