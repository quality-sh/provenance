mod review_support;
use provenance_core::{review::SaveOutcome, StableId};
use review_support::*;
use serde_json::json;

#[test]
fn failed_relationship_replacement_does_not_publish_the_text_edit() {
    let (_temp, store) = fixture();
    store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    let before = store.requirement_edit_state(&scope(), &id()).unwrap();
    let mut input = save(&store, "invalid", json!({"description":"must not persist"}));
    input.relationships = Some(serde_json::from_value(json!({"refines":"req_a", "depends_on":[], "supersedes":[], "spawned_by":null, "source_refs":[]})).unwrap());
    assert!(store.save_requirement(input).is_err());
    assert_eq!(
        store.requirement_edit_state(&scope(), &id()).unwrap(),
        before
    );
    assert!(store
        .requirement_save_receipt(
            &scope(),
            &id(),
            &StableId::new("invalid").unwrap(),
            "ben",
            None
        )
        .unwrap()
        .is_none());
}

#[test]
fn relationship_sets_are_normalized_and_classified_without_lifecycle_changes() {
    let (_temp, store) = fixture();
    let mut other =
        serde_json::to_value(store.list_requirements(&scope()).unwrap().remove(0)).unwrap();
    other["id"] = json!("req_b");
    // Use the existing create input, whose arrays are explicit.
    other["depends_on"] = json!([]);
    other["supersedes"] = json!([]);
    other.as_object_mut().unwrap().remove("schema_version");
    store
        .create_requirement(serde_json::from_value(other).unwrap())
        .unwrap();
    let first = store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    let mut input = save(&store, "relations", json!({}));
    input.relationships = Some(serde_json::from_value(json!({"refines":null, "depends_on":["req_b","req_b"], "supersedes":[], "spawned_by":null, "source_refs":[]})).unwrap());
    let result = store.save_requirement(input).unwrap();
    assert_eq!(result.outcome, SaveOutcome::Changed);
    assert_ne!(first.revision, result.revision);
    assert_eq!(result.changed_fields, ["depends_on"]);
    assert_eq!(
        store.list_requirements(&scope()).unwrap()[0].depends_on,
        [StableId::new("req_b").unwrap()]
    );
}

#[test]
fn missing_snapshot_refuses_further_saves() {
    let (temp, store) = fixture();
    let first = store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    let input = save(&store, "later", json!({"description":"B"}));
    let path = temp.path().join(format!(
        ".provenance/state/scopes/default/review/snapshots/{}.json",
        first.after.id.as_str()
    ));
    std::fs::remove_file(path).unwrap();
    assert!(
        store.save_requirement(input).is_err(),
        "a save must refuse damaged evidence"
    );
}

#[test]
fn enrolled_scope_refuses_lossy_portability_and_external_content_gap() {
    let (temp, store) = fixture();
    assert!(store.ensure_review_portable(&scope()).is_ok());
    store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    assert!(store.ensure_review_portable(&scope()).is_err());
    let path = temp
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let mut record: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    record["description"] = json!("external");
    std::fs::write(path, format!("{record}\n")).unwrap();
    assert!(store
        .requirement_edit_state(&scope(), &id())
        .unwrap_err()
        .to_string()
        .contains("history gap"));
}

#[test]
fn statement_edits_keep_existing_verification_review_behavior() {
    let (temp, store) = fixture();
    store.create_rule(serde_json::from_value(json!({"scope_id":"default", "id":"rule_a", "requirement_ids":["req_a"],"resolution_ids":[], "statement":"The system stores records.","status":"active","severity":"high"})).unwrap()).unwrap();
    std::fs::write(temp.path().join("check.rs"), "fn check() {}\n").unwrap();
    store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    store
        .save_requirement(save(
            &store,
            "statement",
            json!({"statement":"The system reads records."}),
        ))
        .unwrap();
    let reviews = store.open_requirement_reviews(&scope()).unwrap();
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0].before, "The system stores records.");
    assert_eq!(reviews[0].after, "The system reads records.");
    let run = store.begin_verification(scope(), serde_json::from_value(json!({"rule":"rule_a","key":"check","method":"examples","declared_by":"test","file":"check.rs"})).unwrap()).unwrap();
    store
        .complete_verification(
            &scope(),
            serde_json::from_value(json!({"run":run.id,"status":"passed"})).unwrap(),
        )
        .unwrap();
    assert!(store.open_requirement_reviews(&scope()).unwrap().is_empty());
    assert_eq!(store.list_requirement_reviews(&scope()).unwrap().len(), 1);
}

#[test]
fn typed_apply_refuses_before_publishing_any_other_shard() {
    let (_temp, store) = fixture();
    let spec = json!({"schema_version":2,"spec":"fixture","declared_by":"spec://fixture",
        "requirements":[{"key":"a","statement":"The system stores records."}],"sources":[{"key":"source","name":"Original","kind":"policy"}]});
    store
        .apply_typed_spec(&scope(), serde_json::from_value(spec.clone()).unwrap())
        .unwrap();
    let record = store
        .list_requirements(&scope())
        .unwrap()
        .into_iter()
        .find(|r| r.declared_by.is_some())
        .unwrap();
    let etag = store
        .requirement_edit_state(&scope(), &record.id)
        .unwrap()
        .etag;
    store.save_requirement(serde_json::from_value(json!({"request_id":"enroll_owned","actor":"ben","expected_etag":etag,
        "update":{"scope_id":"default","id":record.id,"declared_by":"spec://fixture"},"relationships":null})).unwrap()).unwrap();
    let sources = store.list_sources(&scope()).unwrap();
    let mut changed = spec;
    changed["sources"][0]["name"] = json!("Must not publish");
    changed["requirements"][0]["statement"] = json!("The system reads records.");
    assert!(store
        .apply_typed_spec(&scope(), serde_json::from_value(changed).unwrap())
        .is_err());
    assert_eq!(store.list_sources(&scope()).unwrap(), sources);
}

#[test]
fn an_unknown_enrolled_field_is_refused_before_a_writer_can_drop_it() {
    let (temp, store) = fixture();
    store
        .save_requirement(save(&store, "enroll", json!({})))
        .unwrap();
    let path = temp
        .path()
        .join(".provenance/state/scopes/default/requirements/req.jsonl");
    let mut row: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    row["unsupported_field"] = json!("preserve this value");
    std::fs::write(path, format!("{row}\n")).unwrap();
    assert!(store.requirement_edit_state(&scope(), &id()).is_err());
}
