use super::support::Fixture;
use serde_json::{json, Value};

async fn create_requirement(fixture: &Fixture, id: &str) {
    fixture
        .call(
            "create-requirement",
            json!({
                "scope_id": "default",
                "id": id,
                "statement": format!("The system stores {id}."),
                "status": "active",
                "depends_on": [],
                "supersedes": []
            }),
        )
        .await
        .unwrap();
}

async fn create_source(fixture: &Fixture, id: &str, supersedes: &[&str]) -> Value {
    fixture
        .call(
            "create-source",
            json!({
                "scope_id": "default",
                "id": id,
                "name": id,
                "source_type": "policy",
                "url": "https://example.test/policy",
                "reference": "section 1",
                "supersedes": supersedes
            }),
        )
        .await
        .unwrap()
}

fn relationship_patch(id: &str, field: &str, edit: Value) -> Value {
    let mut request = json!({"scope_id": "default", "id": id});
    request[field] = edit;
    request
}

#[tokio::test]
#[provenance_macros::verifies("rule_porcelain_relationship_membership_noop", examples)]
#[provenance_macros::verifies("rule_porcelain_relationship_patch_modes", examples)]
#[provenance_macros::verifies("rule_porcelain_update_preserves_omissions", examples)]
async fn public_source_patch_keeps_omissions_and_treats_valid_absent_remove_as_noop() {
    let fixture = Fixture::new();
    create_source(&fixture, "source_old", &[]).await;
    create_source(&fixture, "source_other", &[]).await;
    let original = create_source(&fixture, "source_one", &["source_old", "source_other"]).await;

    let removed = fixture
        .call(
            "update-source",
            json!({
                "scope_id": "default",
                "id": "source_one",
                "supersedes": {"remove": ["source_old"]}
            }),
        )
        .await
        .unwrap();
    assert_eq!(removed["url"], original["url"]);
    assert_eq!(removed["reference"], original["reference"]);
    assert_eq!(removed["supersedes"], json!(["source_other"]));

    for target in ["source_old", "source_unused"] {
        if target == "source_unused" {
            create_source(&fixture, target, &[]).await;
        }
        let repeated = fixture
            .call(
                "update-source",
                json!({
                    "scope_id": "default",
                    "id": "source_one",
                    "supersedes": {"remove": [target]}
                }),
            )
            .await
            .unwrap();
        assert_eq!(repeated, removed);
    }

    let replaced = fixture
        .call(
            "update-source",
            json!({
                "scope_id": "default",
                "id": "source_one",
                "supersedes": ["source_old"]
            }),
        )
        .await
        .unwrap();
    assert_eq!(replaced["supersedes"], json!(["source_old"]));
    assert_eq!(replaced["url"], original["url"]);
}

#[tokio::test]
#[provenance_macros::verifies("rule_porcelain_relationship_noop_validates", examples)]
async fn public_absent_remove_rejects_missing_wrong_kind_and_forbidden_targets() {
    let fixture = Fixture::new();
    create_requirement(&fixture, "req_one").await;
    let before = create_source(&fixture, "source_one", &[]).await;
    create_source(&fixture, "source_back", &["source_one"]).await;

    for target in ["source_missing", "req_one", "source_back"] {
        let error = fixture
            .call(
                "update-source",
                json!({
                    "scope_id": "default",
                    "id": "source_one",
                    "supersedes": {"remove": [target]}
                }),
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error["kind"].as_str(),
            Some("missing_reference" | "invalid_update")
        ));
        assert_eq!(
            serde_json::to_value(&fixture.store.list_sources(&fixture.scope).unwrap()[0]).unwrap(),
            before
        );
    }
}

#[tokio::test]
async fn public_rule_and_resolution_lists_keep_required_parents_on_noops() {
    let fixture = Fixture::new();
    for id in ["req_one", "req_two", "req_three"] {
        create_requirement(&fixture, id).await;
    }
    let resolution = fixture.call("create-resolution", json!({
        "scope_id":"default", "id":"res_one", "title":"Decision",
        "position":"Store records", "rationale":"Records are required", "status":"draft",
        "requirement_ids":["req_one", "req_two"], "supersedes":[], "inputs":[]
    })).await.unwrap();
    let rule = fixture.call("create-rule", json!({
        "scope_id":"default", "id":"rule_one", "statement":"The system stores records.",
        "status":"draft", "severity":"medium",
        "requirement_ids":["req_one", "req_two"], "resolution_ids":["res_one"]
    })).await.unwrap();

    for (operation, id, field, before) in [
        ("update-resolution", "res_one", "requirement_ids", resolution),
        ("update-rule", "rule_one", "requirement_ids", rule),
    ] {
        let removed = fixture
            .call(
                operation,
                relationship_patch(id, field, json!({"remove": ["req_two"]})),
            )
            .await
            .unwrap();
        assert_eq!(removed[field], json!(["req_one"]));
        let repeated = fixture
            .call(
                operation,
                relationship_patch(id, field, json!({"remove": ["req_two"]})),
            )
            .await
            .unwrap();
        assert_eq!(repeated, removed);
        let never_linked = fixture
            .call(
                operation,
                relationship_patch(id, field, json!({"remove": ["req_three"]})),
            )
            .await
            .unwrap();
        assert_eq!(never_linked, removed);
        assert!(fixture
            .call(
                operation,
                relationship_patch(id, field, json!({"remove": ["req_one"]})),
            )
            .await
            .is_err());
        let replaced = fixture
            .call(
                operation,
                relationship_patch(id, field, json!(["req_three"])),
            )
            .await
            .unwrap();
        assert_eq!(replaced[field], json!(["req_three"]));
        assert_eq!(replaced["id"], before["id"]);
    }
}

#[tokio::test]
async fn public_requirement_patch_noops_cover_citations_and_current_etag() {
    let fixture = Fixture::new();
    create_requirement(&fixture, "req_one").await;
    create_source(&fixture, "source_one", &[]).await;
    let etag = fixture
        .store
        .requirement_edit_state(&fixture.scope, &"req_one".try_into().unwrap())
        .unwrap()
        .etag;
    let first = fixture.call("update-requirement-v2", json!({
        "request_id":"remove_once", "actor":"reviewer", "expected_etag":etag,
        "id":"req_one", "relationships":{"cites":{"remove":["source_one"]}}
    })).await.unwrap();
    let repeated = fixture.call("update-requirement-v2", json!({
        "request_id":"remove_again", "actor":"reviewer",
        "expected_etag":first["edit"]["etag"], "id":"req_one",
        "relationships":{"cites":{"remove":["source_one"]}}
    })).await.unwrap();
    assert_eq!(repeated["updated"], first["updated"]);
    assert_eq!(repeated["edit"]["etag"], first["edit"]["etag"]);
    assert_eq!(repeated["statement"], first["statement"]);
    assert!(repeated.get("source_refs").is_none());
}
