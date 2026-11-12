//! No Rule: no active Rule states that the native Requirement update checks
//! the etag that it gets, so these examples have no Rule binding.

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

#[tokio::test]
async fn a_native_requirement_update_on_a_stale_etag_is_refused() {
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
    let second = fixture
        .call(
            "update-requirement-native",
            update(Some(&etag), "Second writer"),
        )
        .await;
    assert_eq!(
        second,
        Err(json!({"kind":"requirement_edit_conflict","current_etag":after_first}))
    );
    assert_eq!(
        stored(&fixture).description.as_deref(),
        Some("First writer")
    );
    assert_eq!(current_etag(&fixture), after_first);
}

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
    let input: UpdateRequirementInput =
        serde_json::from_value(update(Some(&etag), "Second writer")).unwrap();
    let native = WriteError(fixture.store.update_requirement(input).unwrap_err());
    assert_eq!(native.0.to_string(), "stale Requirement edit etag");
    let public = fixture
        .call(
            "update-requirement",
            json!({"actor":"reviewer","id":"req_one","expected_etag":etag,"description":"Second writer"}),
        )
        .await
        .unwrap_err();
    assert_eq!(native.status(), 409);
    assert_eq!(serde_json::to_value(native.safe()).unwrap(), public);
}

#[tokio::test]
async fn a_stale_native_update_after_a_status_change_keeps_the_status() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let etag = current_etag(&fixture);
    fixture
        .call(
            "update-requirement",
            json!({"actor":"other-writer","id":"req_one","expected_etag":etag,"status":"refinement"}),
        )
        .await
        .unwrap();
    let after_status = current_etag(&fixture);
    let stale = fixture
        .call(
            "update-requirement-native",
            update(Some(&etag), "Stale writer"),
        )
        .await;
    assert_eq!(
        stale,
        Err(json!({"kind":"requirement_edit_conflict","current_etag":after_status}))
    );
    let requirement = stored(&fixture);
    assert_eq!(
        requirement.status,
        provenance_core::RequirementStatus::Refinement
    );
    assert_eq!(requirement.description, None);
}
