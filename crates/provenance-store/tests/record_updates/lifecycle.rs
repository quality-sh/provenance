use super::support::Fixture;
use serde_json::{json, Value};

fn git(fixture: &Fixture, args: &[&str]) -> String {
    let output = provenance_store::git_process::git()
        .current_dir(fixture.dir.path())
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

fn commit(fixture: &Fixture) -> String {
    git(
        fixture,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.test",
            "commit",
            "--allow-empty",
            "-m",
            "Record stamp fixture",
        ],
    );
    git(fixture, &["rev-parse", "HEAD"])
}

#[tokio::test]
async fn all_four_records_keep_created_and_update_only_for_changed_content() {
    let fixture = Fixture::new();
    git(&fixture, &["init", "--quiet"]);
    let first = commit(&fixture);
    fixture.requirement().await;
    let source = fixture.source().await;
    let rule = fixture.call("create-rule", json!({"scope_id":"default","id":"rule_one","statement":"The system saves the record.","status":"draft","severity":"medium","requirement_ids":["req_one"],"resolution_ids":[]})).await.unwrap();
    let resolution = fixture.call("create-resolution", json!({"scope_id":"default","id":"res_one","title":"Storage","position":"Save records","rationale":"Keep the content","status":"draft","requirement_ids":["req_one"],"supersedes":[],"inputs":[]})).await.unwrap();
    let requirement =
        serde_json::to_value(&fixture.store.list_requirements(&fixture.scope).unwrap()[0]).unwrap();
    let second = commit(&fixture);
    assert_ne!(first, second);
    for (kind, before, change) in [
        ("source", source, json!({"name":"Updated policy"})),
        (
            "requirement",
            requirement,
            json!({"description":"Updated details"}),
        ),
        ("rule", rule, json!({"description":"Updated details"})),
        (
            "resolution",
            resolution,
            json!({"rationale":"Updated reason"}),
        ),
    ] {
        assert_eq!(before["created"]["commit"], first, "{kind}");
        assert_eq!(before["updated"], before["created"], "{kind}");
        let operation = if kind == "requirement" {
            "update-requirement-native".to_owned()
        } else {
            format!("update-{kind}")
        };
        let mut request = json!({"scope_id":"default","id":before["id"]});
        let unchanged = fixture.call(&operation, request.clone()).await.unwrap();
        assert_eq!(unchanged, before, "no-op {kind}");
        request
            .as_object_mut()
            .unwrap()
            .extend(change.as_object().unwrap().clone());
        let after = fixture.call(&operation, request.clone()).await.unwrap();
        assert_eq!(after["created"], before["created"], "{kind}");
        assert_eq!(after["updated"]["commit"], second, "{kind}");
        assert!(after["updated"]["at"].as_str().is_some(), "{kind}");
        assert_eq!(
            fixture.call(&operation, request).await.unwrap(),
            after,
            "repeated edit {kind}"
        );
    }
}

#[tokio::test]
async fn repeated_valid_relationship_removal_keeps_the_record_stamp() {
    let fixture = Fixture::new();
    git(&fixture, &["init", "--quiet"]);
    commit(&fixture);
    create_source_for_stamp(&fixture, "source_old", &[]).await;
    create_source_for_stamp(&fixture, "source_one", &["source_old"]).await;
    let removal_commit = commit(&fixture);
    let removed = fixture
        .call(
            "update-source",
            json!({
                "scope_id":"default", "id":"source_one",
                "supersedes":{"remove":["source_old"]}
            }),
        )
        .await
        .unwrap();
    assert_eq!(removed["updated"]["commit"], removal_commit);
    commit(&fixture);
    let repeated = fixture
        .call(
            "update-source",
            json!({
                "scope_id":"default", "id":"source_one",
                "supersedes":{"remove":["source_old"]}
            }),
        )
        .await
        .unwrap();
    assert_eq!(repeated, removed);
}

async fn create_source_for_stamp(fixture: &Fixture, id: &str, supersedes: &[&str]) {
    fixture
        .call(
            "create-source",
            json!({
                "scope_id":"default", "id":id, "name":id,
                "source_type":"policy", "supersedes":supersedes
            }),
        )
        .await
        .unwrap();
}

async fn rule(fixture: &Fixture, status: &str, archive: Option<Value>) -> Result<Value, Value> {
    let mut input = json!({"scope_id":"default","id":"rule_one","statement":"The system saves the record.","status":status,"severity":"medium","requirement_ids":["req_one"],"resolution_ids":[]});
    if let Some(archive) = archive {
        input["archived_in_commit"] = archive;
    }
    fixture.call("create-rule", input).await
}

#[tokio::test]
#[provenance_macros::verifies("rule_archived_rule_requires_commit", examples)]
#[provenance_macros::verifies("rule_archived_rule_is_terminal", examples)]
async fn archive_requires_a_permalink_and_refuses_every_exit() {
    let fixture = Fixture::new();
    fixture.requirement().await;
    assert!(rule(&fixture, "archived", None).await.is_err());
    let rule_path = fixture
        .dir
        .path()
        .join(".provenance/state/scopes/default/rules/rule.jsonl");
    assert_eq!(std::fs::read_to_string(&rule_path).unwrap_or_default(), "");
    let permalink = json!({"commit":"a".repeat(40),"at":"2026-09-12T00:00:00Z"});
    assert!(rule(&fixture, "draft", Some(permalink.clone()))
        .await
        .is_err());
    assert_eq!(std::fs::read_to_string(&rule_path).unwrap_or_default(), "");
    rule(&fixture, "draft", None).await.unwrap();
    fixture.accept_pending("rule", "rule_one");
    assert!(fixture
        .call(
            "update-rule",
            json!({"scope_id":"default","id":"rule_one","status":"archived"})
        )
        .await
        .is_err());
    fixture.call("update-rule", json!({"scope_id":"default","id":"rule_one","status":"archived","archived_in_commit":permalink})).await.unwrap();
    fixture.accept_status_request("rule", "rule_one");
    let archived =
        serde_json::to_value(&fixture.store.list_rules(&fixture.scope).unwrap()[0]).unwrap();
    assert_eq!(archived["status"], "archived");
    for status in ["draft", "review", "active", "deprecated"] {
        assert!(fixture
            .call(
                "update-rule",
                json!({"scope_id":"default","id":"rule_one","status":status})
            )
            .await
            .is_err());
    }
    assert_eq!(
        serde_json::to_value(&fixture.store.list_rules(&fixture.scope).unwrap()[0]).unwrap(),
        archived
    );
}

#[tokio::test]
async fn typed_reapply_at_a_new_head_preserves_stamps_and_detects_real_changes() {
    let fixture = Fixture::new();
    git(&fixture, &["init", "--quiet"]);
    let first = commit(&fixture);
    let mut spec = json!({"spec":"fixture","declared_by":"spec://fixture",
        "sources":[{"key":"policy","name":"Policy","kind":"policy"}],
        "requirements":[{"key":"ready","statement":"The system is ready."}],
        "rules":[{"key":"ready","requirement":"ready","statement":"The system is ready."}]});
    fixture.call("apply", spec.clone()).await.unwrap();
    let before = fixture.store.list_rules(&fixture.scope).unwrap()[0].clone();
    assert_eq!(before.created.as_ref().unwrap().commit, first);
    let second = commit(&fixture);
    let result = fixture.call("apply", spec.clone()).await.unwrap();
    assert_eq!(result["updated"], 0);
    assert_eq!(result["conflicts"], 0);
    let after = fixture.store.list_rules(&fixture.scope).unwrap()[0].clone();
    assert_eq!(after.updated, before.updated);
    spec["rules"][0]["statement"] = json!("The system saves the record.");
    fixture.call("apply", spec).await.unwrap();
    let changed = fixture.store.list_rules(&fixture.scope).unwrap()[0].clone();
    assert_eq!(changed.created, before.created);
    assert_eq!(changed.updated.unwrap().commit, second);
    assert_ne!(changed.statement, before.statement);
}

/// The public store writes that `Repository::store` gives open a status
/// request as the catalog operations do, and keep the stored status.
#[tokio::test]
#[provenance_macros::verifies("rule_status_change_waits_for_review", examples)]
#[provenance_macros::verifies("rule_new_active_rule_waits_for_review", examples)]
async fn a_store_write_of_an_active_or_archived_status_opens_a_status_request() {
    use provenance_core::{RequirementStatus, RuleStatus, StableId};
    use provenance_store::state_store::{CreateRuleInput, UpdateRequirementInput};

    let fixture = Fixture::new();
    fixture.requirement().await;
    fixture.accept_pending("requirement", "req_one");
    rule(&fixture, "draft", None).await.unwrap();
    fixture.accept_pending("rule", "rule_one");
    let update = json!({"scope_id":"default","id":"rule_one","status":"active"});

    let kept = fixture
        .store
        .update_rule(serde_json::from_value(update).unwrap())
        .unwrap();
    let created = fixture
        .store
        .create_rule(CreateRuleInput {
            scope_id: fixture.scope.clone(),
            id: StableId::new("rule_two").unwrap(),
            name: None,
            description: None,
            requirement_ids: vec![StableId::new("req_one").unwrap()],
            resolution_ids: Vec::new(),
            statement: "The system keeps the record.".into(),
            status: RuleStatus::Active,
            archived_in_commit: None,
            severity: provenance_core::RuleSeverity::Medium,
            source_document: None,
            source_section: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    let requirement = fixture
        .store
        .update_requirement(UpdateRequirementInput {
            scope_id: fixture.scope.clone(),
            id: StableId::new("req_one").unwrap(),
            expected_etag: None,
            declared_by: None,
            statement: None,
            description: None,
            status: Some(RequirementStatus::Archived),
            archived_in_commit: Some(provenance_core::ArchivedStamp {
                commit: "a".repeat(40),
                at: None,
            }),
            clear_fields: Vec::new(),
        })
        .unwrap();

    assert_eq!(kept.status, RuleStatus::Draft);
    assert_eq!(created.status, RuleStatus::Review);
    assert_eq!(requirement.status, RequirementStatus::Active);
    fixture.accept_status_request("rule", "rule_one");
    fixture.accept_status_request("rule", "rule_two");
    let rules = fixture.store.list_rules(&fixture.scope).unwrap();
    assert!(rules.iter().all(|rule| rule.status == RuleStatus::Active));
}
