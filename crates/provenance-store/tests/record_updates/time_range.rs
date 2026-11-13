//! The create and the update of a Resolution refuse an `approved_at` past
//! the range of the read database, ±(2^53 − 1), and keep nothing.
//!
//! No Rule: the plan proposes the Rule to the owner. Until it is in the
//! graph, these tests hold the refusal and its order.

use super::support::{shard_path, Fixture};
use serde_json::{json, Value};

const LARGEST: i64 = (1 << 53) - 1;

/// The text of each stored Resolution file, by file name.
fn stored(fixture: &Fixture) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> =
        std::fs::read_dir(shard_path(fixture, "resolutions", ""))
            .unwrap()
            .map(|entry| {
                let path = entry.unwrap().path();
                let name = path.file_name().unwrap().to_string_lossy().into_owned();
                (name, std::fs::read_to_string(path).unwrap())
            })
            .collect();
    files.sort();
    files
}

/// The refusal of an `approved_at` past the range.
fn refusal() -> Value {
    json!({"kind":"invalid_input","field":"request.approved_at","reason":"invalid_value"})
}

fn create(approved_at: i64, requirement: &str) -> Value {
    json!({"scope_id":"default","id":"resolution_one","title":"Decision",
        "position":"Use the saved record.","rationale":"The record is available.",
        "status":"approved","requirement_ids":[requirement],"supersedes":[],"inputs":[],
        "approved_by":"reviewer","approved_at":approved_at})
}

#[tokio::test]
async fn a_create_refuses_an_approval_time_past_the_range_and_keeps_nothing() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    for past in [LARGEST + 1, -LARGEST - 1, i64::MAX, i64::MIN] {
        let error = fixture
            .call("create-resolution", create(past, "req_one"))
            .await
            .unwrap_err();
        assert_eq!(error, refusal(), "{past}");
    }
    assert_eq!(
        fixture.store.list_resolutions(&fixture.scope).unwrap(),
        Vec::<provenance_core::Resolution>::new()
    );
    for end in [LARGEST, -LARGEST] {
        let fixture = Fixture::new();
        fixture.requirement().await;
        let created = fixture
            .call("create-resolution", create(end, "req_one"))
            .await
            .unwrap();
        assert_eq!(created["approved_at"], json!(end));
    }
}

/// A create with an `approved_at` past the range that also names a
/// Requirement that does not exist gives the `approved_at` refusal.
#[tokio::test]
async fn the_approval_time_refusal_comes_before_the_reference_checks_of_a_create() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    let error = fixture
        .call("create-resolution", create(LARGEST + 1, "req_missing"))
        .await
        .unwrap_err();
    assert_eq!(error, refusal());
}

#[tokio::test]
async fn an_update_refuses_an_approval_time_past_the_range_and_keeps_the_record() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture
        .call("create-resolution", create(1234, "req_one"))
        .await
        .unwrap();
    let before = stored(&fixture);
    for past in [LARGEST + 1, -LARGEST - 1] {
        let error = fixture
            .call(
                "update-resolution",
                json!({"scope_id":"default","id":"resolution_one","approved_at":past}),
            )
            .await
            .unwrap_err();
        assert_eq!(error, refusal(), "{past}");
    }
    assert_eq!(stored(&fixture), before);
    let updated = fixture
        .call(
            "update-resolution",
            json!({"scope_id":"default","id":"resolution_one","approved_at":LARGEST}),
        )
        .await
        .unwrap();
    assert_eq!(updated["approved_at"], json!(LARGEST));
}

/// An update with an `approved_at` past the range gives the `approved_at`
/// refusal also when it clears the same field, names a record that does
/// not exist, or holds a blank title.
#[tokio::test]
async fn the_approval_time_refusal_comes_before_the_other_checks_of_an_update() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture
        .call("create-resolution", create(1234, "req_one"))
        .await
        .unwrap();
    for patch in [
        json!({"clear_fields":["approved_at"]}),
        json!({"id":"resolution_missing"}),
        json!({"title":" "}),
    ] {
        let mut request =
            json!({"scope_id":"default","id":"resolution_one","approved_at":LARGEST + 1});
        request
            .as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let error = fixture
            .call("update-resolution", request)
            .await
            .unwrap_err();
        assert_eq!(error, refusal(), "{patch}");
    }
}
