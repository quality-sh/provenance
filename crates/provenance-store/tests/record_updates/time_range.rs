//! The create and the update of a Resolution refuse an `approved_at` past
//! the range of the read database, ±(2^53 − 1), and keep nothing.
//!
//! No Rule: an implementation aid. The range is a limit of the read
//! database port, and no Rule of the graph states this refusal or its
//! order.

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

/// Appends a line that does not parse to the Resolution file.
fn break_the_file(fixture: &Fixture) {
    let directory = shard_path(fixture, "resolutions", "");
    let path = std::fs::read_dir(directory)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("{not json\n");
    std::fs::write(path, text).unwrap();
}

/// Each other fault of the request or of the stored state, and the kind
/// of refusal that it gives alone. With an `approved_at` past the range,
/// the request gives the `approved_at` refusal, and the state stays the
/// same.
struct Case {
    name: &'static str,
    request: Value,
    broken: bool,
    alone: &'static str,
}

/// Runs each case on a new fixture that holds `req_one` and
/// `resolution_one`, and gives each case whose answer differs from the
/// order.
async fn out_of_order(operation: &str, cases: Vec<Case>) -> Vec<String> {
    let mut wrong = Vec::new();
    for case in cases {
        let fixture = Fixture::new();
        fixture.requirement().await;
        fixture
            .call("create-resolution", create(1234, "req_one"))
            .await
            .unwrap();
        if case.broken {
            break_the_file(&fixture);
        }
        let before = stored(&fixture);
        let mut request = case.request.clone();
        request["approved_at"] = json!(LARGEST + 1);
        let both = fixture.call(operation, request.clone()).await;
        if both != Err(refusal()) {
            wrong.push(format!("{}: with the range fault {both:?}", case.name));
        }
        if stored(&fixture) != before {
            wrong.push(format!("{}: the state changed", case.name));
        }
        request["approved_at"] = json!(1234);
        let alone = fixture.call(operation, request).await;
        if alone.as_ref().err().map(|error| &error["kind"]) != Some(&json!(case.alone)) {
            wrong.push(format!("{}: alone {alone:?}", case.name));
        }
    }
    wrong
}

/// A create with an `approved_at` past the range gives the `approved_at`
/// refusal before each other check of the create.
#[tokio::test]
async fn the_approval_time_refusal_comes_before_each_check_of_a_create() {
    let with = |change: Value| {
        let mut request = create(0, "req_one");
        request["id"] = json!("resolution_two");
        request
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        request
    };
    let case = |name, request, broken, alone| Case {
        name,
        request,
        broken,
        alone,
    };
    let cases = vec![
        case(
            "reserved id",
            with(json!({"id":"api"})),
            false,
            "invalid_input",
        ),
        case(
            "id of a Requirement",
            with(json!({"id":"req_one"})),
            false,
            "already_exists",
        ),
        case(
            "id of a Resolution",
            with(json!({"id":"resolution_one"})),
            false,
            "already_exists",
        ),
        case(
            "origin that does not exist",
            with(json!({"origin_thread":"thread_missing","origin_message":"msg_missing"})),
            false,
            "missing_reference",
        ),
        case(
            "no Requirement",
            with(json!({"requirement_ids":[]})),
            false,
            "missing_reference",
        ),
        case(
            "Requirement that does not exist",
            with(json!({"requirement_ids":["req_missing"]})),
            false,
            "missing_reference",
        ),
        case(
            "superseded Resolution that does not exist",
            with(json!({"supersedes":["resolution_missing"]})),
            false,
            "missing_reference",
        ),
        case(
            "line that does not parse",
            with(json!({})),
            true,
            "invalid_state_file",
        ),
    ];
    let wrong = out_of_order("create-resolution", cases).await;
    assert_eq!(wrong, Vec::<String>::new());
}

/// An update with an `approved_at` past the range gives the `approved_at`
/// refusal before each other check of the update.
#[tokio::test]
async fn the_approval_time_refusal_comes_before_each_check_of_an_update() {
    let with = |change: Value| {
        let mut request = json!({"scope_id":"default","id":"resolution_one"});
        request
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        request
    };
    let case = |name, request, broken, alone| Case {
        name,
        request,
        broken,
        alone,
    };
    let cases = vec![
        case(
            "record that does not exist",
            with(json!({"id":"resolution_missing"})),
            false,
            "not_found",
        ),
        case(
            "edit tag that differs",
            with(json!({"expected_etag":"stale"})),
            false,
            "conflict",
        ),
        case(
            "Requirement that does not exist",
            with(json!({"requirement_ids":{"add":["req_missing"]}})),
            false,
            "missing_reference",
        ),
        case(
            "no Requirement",
            with(json!({"requirement_ids":[]})),
            false,
            "missing_reference",
        ),
        case(
            "blank title",
            with(json!({"title":" "})),
            false,
            "invalid_update",
        ),
        case(
            "field set and cleared",
            with(json!({"approved_by":"reviewer","clear_fields":["approved_by"]})),
            false,
            "invalid_update",
        ),
        case(
            "approved_at set and cleared",
            with(json!({"clear_fields":["approved_at"]})),
            false,
            "invalid_update",
        ),
        case(
            "line that does not parse",
            with(json!({})),
            true,
            "invalid_state_file",
        ),
    ];
    let wrong = out_of_order("update-resolution", cases).await;
    assert_eq!(wrong, Vec::<String>::new());
}

/// A blank input of a Resolution is refused when the request is decoded,
/// before the operation runs, so it comes before the `approved_at`
/// refusal.
#[tokio::test]
async fn a_blank_input_is_refused_before_the_approval_time() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture
        .call("create-resolution", create(1234, "req_one"))
        .await
        .unwrap();
    let error = fixture
        .call(
            "update-resolution",
            json!({"scope_id":"default","id":"resolution_one","approved_at":LARGEST + 1,
                "inputs":[{"input_type":"source_material","reference":"","summary":""}]}),
        )
        .await
        .unwrap_err();
    assert_eq!(error, json!({"kind":"invalid_input"}));
}
