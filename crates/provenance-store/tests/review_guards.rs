#[allow(dead_code)]
mod review_support;
use camino::Utf8Path;
use provenance_core::NodeType;
use provenance_store::jsonl::write_jsonl_atomic;
use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
use review_support::{fixture, scope};
use serde_json::json;

/// Implementation aid: a public raw writer cannot change submitted content.
#[test]
fn unintegrated_atomic_writer_cannot_replace_a_submitted_requirement() {
    let (temp, store) = fixture();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    let path = provenance_store::shards::requirements_path(&layout, &scope());
    let before = std::fs::read(&path).unwrap();
    let mut record = serde_json::to_value(&store.list_requirements(&scope()).unwrap()[0]).unwrap();
    record["statement"] = json!("Changed content.");
    assert!(write_jsonl_atomic(&path, &[record]).is_err());
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn review_schema_is_readable_only_for_requirements_and_manifest() {
    use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":3,"scopes":[],"disposition_actor_ids":[]}"#,
    )
    .unwrap();
    assert!(StateStore::new(layout).manifest().is_ok());
    assert!(provenance_core::ensure_supported_schema_version(
        "proposal",
        provenance_core::SchemaVersion(3)
    )
    .is_err());
}

/// Implementation aid: repeated ids cannot bypass the public writer guard.
#[test]
fn duplicate_submitted_identity_cannot_hide_an_extra_mutation() {
    let (temp, store) = fixture();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    let path = provenance_store::shards::requirements_path(&layout, &scope());
    let before = serde_json::to_value(&store.list_requirements(&scope()).unwrap()[0]).unwrap();
    let mut duplicate = before.clone();
    duplicate["statement"] = json!("Changed content.");
    assert!(write_jsonl_atomic(&path, &[before, duplicate]).is_err());
}

fn input<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).unwrap()
}

fn create(store: &StateStore, kind: NodeType) -> serde_json::Value {
    let value = match kind {
        NodeType::Requirement => serde_json::to_value(&store.list_requirements(&scope()).unwrap()[0]),
        NodeType::Source => serde_json::to_value(store.create_source(input(json!({
            "scope_id":"default","id":"source_a","name":"Policy","source_type":"policy","supersedes":[]
        }))).unwrap()),
        NodeType::Resolution => serde_json::to_value(store.create_resolution(input(json!({
            "scope_id":"default","id":"resolution_a","title":"Store records",
            "position":"Store records.","rationale":"Records are required.","status":"proposed",
            "requirement_ids":["req_a"],"supersedes":[],"inputs":[]
        }))).unwrap()),
        NodeType::Rule => serde_json::to_value(store.create_rule(input(json!({
            "scope_id":"default","id":"rule_a","statement":"The system stores records.",
            "status":"active","severity":"medium","requirement_ids":["req_a"],"resolution_ids":[]
        }))).unwrap()),
        NodeType::Domain => serde_json::to_value(store.create_domain(input(json!({
            "scope_id":"default","id":"domain_a","name":"Storage"
        }))).unwrap()),
        NodeType::Boundary => serde_json::to_value(store.create_boundary(input(json!({
            "scope_id":"default","id":"boundary_a","requirement_id":"req_a","statement":"Storage only."
        }))).unwrap()),
        NodeType::Topic | NodeType::Question => {
            let topic = store.create_topic(input(json!({
                "scope_id":"default","id":"topic_a","requirement_id":"req_a",
                "title":"Storage","status":"open","links":[]
            }))).unwrap();
            if kind == NodeType::Topic {
                serde_json::to_value(topic)
            } else {
                serde_json::to_value(store.create_question(input(json!({
                    "scope_id":"default","id":"question_a","topic_id":"topic_a",
                    "question":"Which storage?","resolution_method":"research","status":"open","links":[]
                }))).unwrap())
            }
        }
    }.unwrap();
    if kind != NodeType::Requirement {
        store.submit_record_review(input(json!({
            "scope_id":"default","actor":"author","record_kind":kind,"record_id":value["id"],
            "title":"Review the record","summary":"Review the stored text.","declared_by":null,
            "source_ids":[],"evidence_references":[],"builds_on":[],"expected_revision":null,"revises":null
        }))).unwrap();
    }
    value
}

/// Implementation aid: each submitted record kind uses the same raw writer guard.
#[test]
fn unintegrated_writers_refuse_submitted_rows_for_every_record_kind() {
    for kind in provenance_core::review::REVIEW_RECORD_KINDS {
        let (temp, store) = fixture();
        let mut record = create(&store, *kind);
        let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
        let path = provenance_store::shards::path_for(&layout, &scope(), *kind);
        let before = std::fs::read(&path).unwrap();
        record["changed"] = json!(true);
        let error = write_jsonl_atomic(&path, &[record]).unwrap_err();
        assert!(
            error.to_string().contains("guarded review save"),
            "{kind:?}: {error:#}"
        );
        assert_eq!(std::fs::read(path).unwrap(), before);
    }
}

#[test]
fn review_schema_is_readable_for_every_record_kind() {
    use provenance_core::ScopeId;
    use provenance_store::{layout::ProvenanceLayout, state_store::StateStore};
    let temp = tempfile::tempdir().unwrap();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":3,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let fixtures = [
        (
            "sources/source.jsonl",
            json!({"schema_version":3,"scope_id":"default","id":"source_a","name":"Policy","source_type":"document","url":null}),
        ),
        (
            "requirements/req.jsonl",
            json!({"schema_version":3,"scope_id":"default","id":"req_a","statement":"The system stores records.","status":"active"}),
        ),
        (
            "resolutions/res.jsonl",
            json!({"schema_version":3,"scope_id":"default","id":"resolution_a","title":"Decision","position":"Use it.","rationale":"It is required.","status":"approved","inputs":[],"requirement_ids":["req_a"],"review_on":null}),
        ),
        (
            "rules/rule.jsonl",
            json!({"schema_version":3,"scope_id":"default","id":"rule_a","statement":"The system stores records.","status":"active","severity":"high","requirement_ids":["req_a"],"resolution_ids":[]}),
        ),
        (
            "domains/domain.jsonl",
            json!({"schema_version":3,"scope_id":"default","id":"domain_a","name":"Storage"}),
        ),
        (
            "boundaries/boundary.jsonl",
            json!({"schema_version":3,"scope_id":"default","id":"boundary_a","requirement_id":"req_a","statement":"Storage only."}),
        ),
        (
            "topics/topic.jsonl",
            json!({"schema_version":3,"scope_id":"default","id":"topic_a","requirement_id":"req_a","title":"Storage","status":"open","links":[]}),
        ),
        (
            "questions/question.jsonl",
            json!({"schema_version":3,"scope_id":"default","id":"question_a","topic_id":"topic_a","requirement_id":"req_a","question":"Where is storage?","resolution_method":"research","status":"open","links":[]}),
        ),
    ];
    for (relative, value) in fixtures {
        let path = layout.scopes_dir().join("default").join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, format!("{value}\n")).unwrap();
        let directory = Utf8Path::new(relative)
            .parent()
            .unwrap()
            .file_name()
            .unwrap();
        let kind = match directory {
            "boundaries" => "boundary",
            other => other.trim_end_matches('s'),
        };
        assert!(provenance_core::ensure_supported_schema_version(
            kind,
            provenance_core::SchemaVersion(3)
        )
        .is_ok());
    }
    let store = StateStore::new(layout);
    let scope = ScopeId::new("default").unwrap();
    assert_eq!(store.list_sources(&scope).unwrap().len(), 1);
    assert_eq!(store.list_requirements(&scope).unwrap().len(), 1);
    assert_eq!(store.list_resolutions(&scope).unwrap().len(), 1);
    assert_eq!(store.list_rules(&scope).unwrap().len(), 1);
    assert_eq!(store.list_domains(&scope).unwrap().len(), 1);
    assert_eq!(store.list_boundaries(&scope).unwrap().len(), 1);
    assert_eq!(store.list_topics(&scope).unwrap().len(), 1);
    assert_eq!(store.list_questions(&scope).unwrap().len(), 1);
}
