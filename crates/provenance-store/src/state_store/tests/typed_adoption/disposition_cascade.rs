use super::{document, initialized_store};
use provenance_core::{ScopeId, SUPPORTED_SCHEMA_VERSION};

#[test]
fn typed_spec_accepts_dispositions_for_each_surviving_canonical_kind() {
    for (artifact_type, record) in canonical_records() {
        let (_dir, store, scope) = initialized_store();
        write_jsonl(&canonical_path(&store.layout, &scope, artifact_type), record);
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
fn typed_spec_refuses_dispositions_for_each_missing_canonical_kind() {
    for artifact_type in canonical_artifact_types() {
        let (_dir, store, scope) = initialized_store();
        write_disposition(&store, &scope, artifact_type);

        let error = store
            .apply_typed_spec(
                &scope,
                document("spec://rust/no-op", Vec::new(), Vec::new()),
            )
            .unwrap_err()
            .to_string();

        assert!(
            error.contains(&format!("cannot delete canonical {artifact_type}")),
            "{artifact_type}: {error}"
        );
    }
}

fn write_disposition(
    store: &crate::state_store::StateStore,
    scope: &ScopeId,
    artifact_type: &str,
) {
    let record = serde_json::json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
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
        "domain" => crate::shards::domains_path(layout, scope),
        "boundary" => crate::shards::boundaries_path(layout, scope),
        "topic" => crate::shards::topics_path(layout, scope),
        "question" => crate::shards::questions_path(layout, scope),
        _ => unreachable!("all canonical artifact kinds are covered"),
    }
}

fn canonical_artifact_types() -> [&'static str; 8] {
    [
        "source",
        "requirement",
        "resolution",
        "rule",
        "domain",
        "boundary",
        "topic",
        "question",
    ]
}

fn canonical_records() -> [(&'static str, String); 8] {
    let version = SUPPORTED_SCHEMA_VERSION.0;
    [
        ("source", serde_json::json!({"schema_version":version,"scope_id":"default","id":"artifact_test","name":"Test","source_type":"document","url":null}).to_string()),
        ("requirement", serde_json::json!({"schema_version":version,"scope_id":"default","id":"artifact_test","statement":"Test","status":"active"}).to_string()),
        ("resolution", serde_json::json!({"schema_version":version,"scope_id":"default","id":"artifact_test","title":"Test","position":"Test","rationale":"Test","status":"approved","inputs":[],"review_on":null}).to_string()),
        ("rule", serde_json::json!({"schema_version":version,"scope_id":"default","id":"artifact_test","statement":"Test","status":"draft","severity":"high"}).to_string()),
        ("domain", serde_json::json!({"schema_version":version,"scope_id":"default","id":"artifact_test","name":"Test"}).to_string()),
        ("boundary", serde_json::json!({"schema_version":version,"scope_id":"default","id":"artifact_test","requirement_id":"requirement_test","statement":"Test"}).to_string()),
        ("topic", serde_json::json!({"schema_version":version,"scope_id":"default","id":"artifact_test","requirement_id":"requirement_test","title":"Test","status":"open","links":[]}).to_string()),
        ("question", serde_json::json!({"schema_version":version,"scope_id":"default","id":"artifact_test","topic_id":"topic_test","requirement_id":"requirement_test","question":"Test?","resolution_method":"research","status":"open","links":[]}).to_string()),
    ]
}

fn write_jsonl(path: &camino::Utf8Path, record: impl AsRef<str>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{}\n", record.as_ref())).unwrap();
}
