//! The plan route shows each deletion that the apply route refuses, with the
//! cause that the apply route gives.
#![cfg(feature = "test-fixture")]
use crate::support;
use provenance_macros::verifies;
use serde_json::{json, Value};
use support::records::Repository;
use support::resource_http::{call, host};

fn spec(requirements: &[(&str, &str)]) -> Value {
    let requirements = requirements
        .iter()
        .map(|(key, statement)| json!({"key": key, "statement": statement}))
        .collect::<Vec<_>>();
    json!({"data":{
        "spec":"fx", "declared_by":"spec://fx", "sources":[], "requirements":requirements
    }})
}

fn resource<'a>(answer: &'a Value, key: &str) -> &'a Value {
    answer["data"]["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["key"] == key)
        .unwrap_or_else(|| panic!("no resource {key} in {answer}"))
}

/// The state files of the repository. The cache is a derived copy of the
/// state, so it is left out.
fn state_files(repo: &Repository) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    let mut files = repo.bytes();
    files.retain(|path, _| !path.starts_with(".provenance/cache"));
    files
}

#[tokio::test]
#[verifies("rule_typed_plan_shows_refused_deletions", examples)]
async fn the_plan_route_shows_the_deletion_that_the_apply_route_refuses() {
    let repo = Repository::new("The shared graph is readable.");
    let host = host(&repo, true);
    let kept = ("kept", "The system retains records.");
    let plain = ("plain", "The system stores records.");
    let mut applied = Value::Null;
    // The second statement is a typed change, so the apply opens a submission.
    for statement in ["The host stops.", "The host starts."] {
        let (status, answer) = call(
            &host,
            "POST",
            "/authoring-changes",
            Some(spec(&[("restated", statement), plain, kept])),
        )
        .await;
        assert_eq!(status, 200, "{answer}");
        applied = answer;
    }
    let restated = resource(&applied, "restated")["id"].clone();
    let message = format!(
        "cannot delete requirement {} because it has a review submission; \
         keep the requirement in the typed document",
        restated.as_str().unwrap()
    );
    let before = state_files(&repo);

    let (status, plan) = call(&host, "POST", "/authoring-plans", Some(spec(&[kept]))).await;
    let (refused_status, refused) =
        call(&host, "POST", "/authoring-changes", Some(spec(&[kept]))).await;

    assert_eq!(status, 200, "{plan}");
    let entry = resource(&plan, "restated");
    assert_eq!(entry["state"], "refused", "{plan}");
    assert_eq!(
        entry["refusal"],
        json!({"cause": "review_submission", "message": message}),
        "{plan}"
    );
    assert_eq!(resource(&plan, "plain")["state"], "deleted", "{plan}");
    assert_eq!(
        [&plan["data"]["deleted"], &plan["data"]["refused"]],
        [&json!(1), &json!(1)],
        "{plan}"
    );
    assert_eq!(refused_status, 409, "{refused}");
    assert_eq!(
        refused["error"],
        json!({"kind": "enrolled_record_deletion_conflict", "record_kind": "requirement",
               "record_id": restated, "message": message})
    );
    assert_eq!(state_files(&repo), before);
}
