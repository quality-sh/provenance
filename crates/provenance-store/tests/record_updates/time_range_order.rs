//! The `approved_at` refusal of a Resolution create or update comes before
//! each other check of the operation body. Each case puts an `approved_at`
//! past the range together with one other fault.
//!
//! No Rule: implementation aids. A request that does not decode is refused
//! before the body, so the cases hold the decode refusal there too.

use super::support::{shard_path, Fixture};
use super::time_range::{create, refusal, stored, LARGEST};
use serde_json::{json, Value};

/// The one Resolution file of the fixture, with a line that is not a
/// record added at its end.
fn break_the_file(fixture: &Fixture) {
    let (name, text) = stored(fixture).remove(0);
    std::fs::write(
        shard_path(fixture, "resolutions", &name),
        text + "{\"id\":\n",
    )
    .unwrap();
}

/// A fixture with `req_one` and the Resolution `resolution_one`.
async fn with_resolution() -> Fixture {
    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture
        .call("create-resolution", create(1234, "req_one"))
        .await
        .unwrap();
    fixture
}

/// `request` with each field of `patch` set.
fn patched(mut request: Value, patch: &Value) -> Value {
    request
        .as_object_mut()
        .unwrap()
        .extend(patch.as_object().unwrap().clone());
    request
}

/// Calls `operation` with each case and gives each case whose refusal is
/// not `expected`, with the answer that it got.
async fn mismatches(operation: &str, cases: Vec<(&str, Fixture, Value, Value)>) -> Vec<String> {
    let mut found = Vec::new();
    for (name, fixture, request, expected) in cases {
        let before = stored(&fixture);
        let answer = fixture.call(operation, request).await;
        if answer != Err(expected) {
            found.push(format!("{name}: {answer:?}"));
        }
        if stored(&fixture) != before {
            found.push(format!("{name}: the operation changed the file"));
        }
    }
    found
}

#[tokio::test]
async fn the_approval_time_refusal_comes_before_each_check_of_a_create() {
    let past = create(LARGEST + 1, "req_one");
    let existing = with_resolution().await;
    let reserved = with_resolution().await;
    let origin = with_resolution().await;
    let no_requirement = with_resolution().await;
    let missing_requirement = with_resolution().await;
    let missing_older = with_resolution().await;
    let broken = with_resolution().await;
    break_the_file(&broken);
    let second = |patch: Value| {
        patched(
            patched(past.clone(), &json!({"id":"resolution_two"})),
            &patch,
        )
    };
    let cases = vec![
        ("an id that exists", existing, past.clone(), refusal()),
        (
            "a reserved id",
            reserved,
            patched(past.clone(), &json!({"id":"api"})),
            refusal(),
        ),
        (
            "an origin thread that does not exist",
            origin,
            second(json!({"origin_thread":"thread_missing"})),
            refusal(),
        ),
        (
            "no Requirement",
            no_requirement,
            second(json!({"requirement_ids":[]})),
            refusal(),
        ),
        (
            "a Requirement that does not exist",
            missing_requirement,
            second(json!({"requirement_ids":["req_missing"]})),
            refusal(),
        ),
        (
            "a superseded Resolution that does not exist",
            missing_older,
            second(json!({"supersedes":["resolution_missing"]})),
            refusal(),
        ),
        (
            "a Resolution file with a bad line",
            broken,
            second(json!({})),
            refusal(),
        ),
    ];
    assert_eq!(
        mismatches("create-resolution", cases).await,
        Vec::<String>::new()
    );
}

#[tokio::test]
async fn the_approval_time_refusal_comes_before_each_check_of_an_update() {
    let past = json!({"scope_id":"default","id":"resolution_one","approved_at":LARGEST + 1});
    let blank_input = json!({"input_type":"technical","reference":" ","summary":" "});
    let broken = with_resolution().await;
    break_the_file(&broken);
    let mut cases = vec![(
        "a blank input, refused when the request is decoded",
        with_resolution().await,
        patched(past.clone(), &json!({"inputs":[blank_input]})),
        json!({"kind":"invalid_input"}),
    )];
    for (name, patch) in [
        (
            "a record that does not exist",
            json!({"id":"resolution_missing"}),
        ),
        (
            "an edit tag that does not match",
            json!({"expected_etag":"etag_wrong"}),
        ),
        (
            "a Requirement that does not exist",
            json!({"requirement_ids":["req_missing"]}),
        ),
        (
            "a superseded Resolution that does not exist",
            json!({"supersedes":["resolution_missing"]}),
        ),
        ("a blank title", json!({"title":" "})),
        (
            "a field that is set and cleared",
            json!({"clear_fields":["approved_at"]}),
        ),
    ] {
        cases.push((
            name,
            with_resolution().await,
            patched(past.clone(), &patch),
            refusal(),
        ));
    }
    cases.push(("a Resolution file with a bad line", broken, past, refusal()));
    assert_eq!(
        mismatches("update-resolution", cases).await,
        Vec::<String>::new()
    );
}
