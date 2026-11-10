use crate::porcelain_authoring_support::{
    initialized_repo, json_output, json_stdin_output, provenance,
};
use serde_json::{json, Value};

const OWNER: &str = "spec://typescript/share-links";

fn document(requirements: Value) -> Value {
    json!({
        "spec": "share-links",
        "declared_by": OWNER,
        "sources": [{"key": "policy", "name": "Policy", "kind": "policy"}],
        "requirements": requirements,
        "rules": []
    })
}

fn authoring(repo: &str, route: &str, input: &Value) -> Value {
    let arguments = [
        "api", route, "--method", "post", "--input", "-", "--repo", repo, "--scope", "default",
        "--format", "json",
    ];
    json_stdin_output(&arguments, input)["data"].clone()
}

fn requirement_id(applied: &Value, key: &str) -> String {
    applied["resources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|resource| resource["kind"] == "requirement" && resource["key"] == key)
        .unwrap_or_else(|| panic!("no requirement {key} in {applied}"))["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn create(repo: &str, id: &str, kind: &str, fields: &[&str]) {
    let mut arguments = vec![
        id, "create", "--type", kind, "--repo", repo, "--format", "json",
    ];
    arguments.extend_from_slice(fields);
    json_output(&arguments);
}

fn record(repo: &str, id: &str) -> Value {
    json_output(&[id, "get", "--repo", repo, "--format", "json"])["record"]["value"].clone()
}

#[test]
fn deleting_a_requirement_removes_its_ids_from_surviving_records() {
    let (_directory, repo) = initialized_repo();
    let other = json!({"key": "other", "statement": "The host starts."});
    let both = json!([
        {"key": "sharing", "statement": "The host stops.", "sources": ["policy"]},
        other
    ]);
    let initial = authoring(&repo, "authoring-changes", &document(both));
    let removed = requirement_id(&initial, "sharing");
    let retained = requirement_id(&initial, "other");
    let both_ids = json!([removed, retained]).to_string();
    let removed_link = json!([{"target_type": "requirement", "target_id": removed}]).to_string();
    create(
        &repo,
        "res_shared",
        "resolution",
        &[
            "--title",
            "Shared decision",
            "--position",
            "The host stops.",
            "--rationale",
            "The host stops.",
            "--requirement-ids-json",
            &both_ids,
        ],
    );
    create(
        &repo,
        "rule_shared",
        "rule",
        &[
            "--statement",
            "The host stops.",
            "--requirement-ids-json",
            &both_ids,
            "--resolution-ids-json",
            r#"["res_shared"]"#,
        ],
    );
    create(
        &repo,
        "topic_retained",
        "topic",
        &[
            "--title",
            "Retained topic",
            "--requirement-id",
            &retained,
            "--links-json",
            &removed_link,
        ],
    );
    create(
        &repo,
        "question_retained",
        "question",
        &[
            "--topic-id",
            "topic_retained",
            "--question",
            "Retained?",
            "--resolution-method",
            "research",
            "--contradicts",
            &removed,
            "--links-json",
            &removed_link,
        ],
    );
    let after = document(json!([other]));

    let planned = authoring(&repo, "authoring-plans", &after);
    let applied = authoring(&repo, "authoring-changes", &after);

    let shared_before = json!([retained, removed]);
    let links_before = json!([{"target_id": removed, "target_type": "requirement"}]);
    let expected = json!([
        {"id": "res_shared", "kind": "resolution", "state": "updated", "changes": [
            {"field": "requirement_ids", "before": shared_before, "after": [retained]}
        ]},
        {"id": "rule_shared", "kind": "rule", "state": "updated", "changes": [
            {"field": "requirement_ids", "before": shared_before, "after": [retained]}
        ]},
        {"id": "topic_retained", "kind": "topic", "state": "updated", "changes": [
            {"field": "links", "before": links_before, "after": []}
        ]},
        {"id": "question_retained", "kind": "question", "state": "updated", "changes": [
            {"field": "contradicts", "before": removed, "after": null},
            {"field": "links", "before": links_before, "after": []}
        ]}
    ]);
    assert_eq!(planned["cascade"], expected, "plan cascade: {planned}");
    assert_eq!(
        (&planned["updated"], &planned["deleted"]),
        (&json!(4), &json!(1)),
        "plan counts: {planned}"
    );
    assert_eq!(
        applied["cascade"], planned["cascade"],
        "apply cascade: {applied}"
    );
    assert_eq!(
        (&applied["updated"], &applied["deleted"]),
        (&planned["updated"], &planned["deleted"]),
        "apply counts: {applied}"
    );

    let retained_ids = json!([retained]);
    let resolution = record(&repo, "res_shared");
    assert_eq!(resolution["requirement_ids"], retained_ids, "{resolution}");
    let rule = record(&repo, "rule_shared");
    assert_eq!(rule["requirement_ids"], retained_ids, "{rule}");
    assert_eq!(rule["resolution_ids"], json!(["res_shared"]), "{rule}");
    let topic = record(&repo, "topic_retained");
    assert_eq!(topic["links"], json!([]), "{topic}");
    let question = record(&repo, "question_retained");
    assert_eq!(question["contradicts"], Value::Null, "{question}");
    assert_eq!(question["links"], json!([]), "{question}");
    let missing = provenance()
        .args([removed.as_str(), "get", "--repo", &repo, "--format", "json"])
        .output()
        .unwrap();
    let error = String::from_utf8_lossy(&missing.stderr);
    assert_eq!(
        (missing.status.code(), error.trim()),
        (
            Some(1),
            r#"{"error":{"kind":"resource_not_found","message":"record does not exist"},"meta":{}}"#
        ),
        "read of the deleted Requirement"
    );
}
