use super::{document, initialized_store};
use crate::state_store::readers::StateLineInvalid;
use provenance_core::ScopeId;
use provenance_macros::verifies;

#[test]
fn typed_spec_accepts_dispositions_for_each_surviving_canonical_kind() {
    for (artifact_type, record) in canonical_records("default") {
        let (_dir, store, scope) = initialized_store();
        write_jsonl(
            &canonical_path(&store.layout, &scope, artifact_type),
            record,
        );
        write_disposition(&store, &scope, artifact_type);

        store
            .apply_typed_spec(
                &scope,
                document("spec://rust/no-op", Vec::new(), Vec::new()),
            )
            .unwrap_or_else(|error| panic!("{artifact_type}: {error:#}"));
    }
}

#[test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn typed_spec_refuses_dispositions_for_each_wrong_scope_canonical_kind() {
    for (artifact_type, record) in canonical_records("other") {
        let (_dir, store, scope) = initialized_store();
        write_jsonl(
            &canonical_path(&store.layout, &scope, artifact_type),
            record,
        );
        write_disposition(&store, &scope, artifact_type);

        assert_names_no_record(&store, &scope, artifact_type);
    }
}

#[test]
#[verifies("rule_inconsistent_records_refusal_names_records", examples)]
fn typed_spec_refuses_dispositions_for_each_missing_canonical_kind() {
    for artifact_type in canonical_artifact_types() {
        let (_dir, store, scope) = initialized_store();
        write_disposition(&store, &scope, artifact_type);

        assert_names_no_record(&store, &scope, artifact_type);
    }
}

/// The plan and the apply refuse the scope at the line of the Disposition,
/// which names a record that the scope does not have.
fn assert_names_no_record(
    store: &crate::state_store::StateStore,
    scope: &ScopeId,
    artifact_type: &str,
) {
    let no_op = || document("spec://rust/no-op", Vec::new(), Vec::new());
    let path = crate::shards::dispositions_path(&store.layout, scope);
    let planned = store.plan_typed_spec(scope, no_op()).unwrap_err();
    let applied = store.apply_typed_spec(scope, no_op()).unwrap_err();

    for error in [planned, applied] {
        let fault = StateLineInvalid::find(&error)
            .unwrap_or_else(|| panic!("{artifact_type}: {error:#}"));
        assert_eq!(
            (&fault.path, fault.line, fault.message.as_str()),
            (
                &path,
                1,
                format!(
                    "disposition disposition_artifact names {artifact_type} artifact_test, \
                     which the scope does not have"
                )
                .as_str()
            ),
            "{artifact_type}"
        );
    }
}

fn write_disposition(store: &crate::state_store::StateStore, scope: &ScopeId, artifact_type: &str) {
    let record = serde_json::json!({
        "scope_id": scope,
        "id": "disposition_artifact",
        "proposal_id": "proposal_artifact",
        "decision": "rejected",
        "rationale": "Reviewed",
        "actor": {"identity_type": "human", "id": "reviewer"},
        "canonical_artifact": {
            "artifact_type": artifact_type,
            "artifact_id": "artifact_test"
        }
    });
    write_jsonl(
        &crate::shards::dispositions_path(&store.layout, scope),
        record.to_string(),
    );
}

fn canonical_path(
    layout: &crate::layout::ProvenanceLayout,
    scope: &ScopeId,
    artifact_type: &str,
) -> camino::Utf8PathBuf {
    match artifact_type {
        "source" => crate::shards::sources_path(layout, scope),
        "requirement" => crate::shards::requirements_path(layout, scope),
        "resolution" => crate::shards::resolutions_path(layout, scope),
        "rule" => crate::shards::rules_path(layout, scope),
        "topic" => crate::shards::topics_path(layout, scope),
        "question" => crate::shards::questions_path(layout, scope),
        _ => unreachable!("all canonical artifact kinds are covered"),
    }
}

fn canonical_artifact_types() -> [&'static str; 6] {
    [
        "source",
        "requirement",
        "resolution",
        "rule",
        "topic",
        "question",
    ]
}

fn canonical_records(scope: &str) -> [(&'static str, String); 6] {
    [
        ("source", serde_json::json!({"scope_id":scope,"id":"artifact_test","name":"Test","source_type":"document","url":null}).to_string()),
        ("requirement", serde_json::json!({"scope_id":scope,"id":"artifact_test","statement":"Test","status":"active"}).to_string()),
        ("resolution", serde_json::json!({"scope_id":scope,"id":"artifact_test","title":"Test","position":"Test","rationale":"Test","status":"approved","inputs":[]}).to_string()),
        ("rule", serde_json::json!({"scope_id":scope,"id":"artifact_test","statement":"Test","status":"draft","severity":"high"}).to_string()),
        ("topic", serde_json::json!({"scope_id":scope,"id":"artifact_test","requirement_id":"requirement_test","title":"Test","status":"open","links":[]}).to_string()),
        ("question", serde_json::json!({"scope_id":scope,"id":"artifact_test","topic_id":"topic_test","requirement_id":"requirement_test","question":"Test?","resolution_method":"research","status":"open","links":[]}).to_string()),
    ]
}

fn write_jsonl(path: &camino::Utf8Path, record: impl AsRef<str>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{}\n", record.as_ref())).unwrap();
}
