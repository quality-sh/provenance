use super::journal;
use crate::{
    cache::review_families,
    layout::ProvenanceLayout,
    shards,
    state_store::{
        CreateBoundaryInput, CreateQuestionInput, CreateResolutionInput, CreateTopicInput,
        StateStore, TypedSpecInput,
    },
};
use camino::Utf8Path;
use provenance_core::{
    protocol::{TypedRequirementInput, TypedRuleInput, TypedSourceInput},
    review::{ReviewEntry, SaveOutcome, REVIEW_SCHEMA_VERSION},
    ArtifactLink, ArtifactLinkTargetType, NodeType, QuestionStatus, ResolutionMethod,
    ResolutionStatus, ScopeId, SourceReference, StableId, TopicStatus, SUPPORTED_SCHEMA_VERSION,
};

const OWNER: &str = "spec://review/typed";

fn fixture() -> (tempfile::TempDir, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    (
        temp,
        StateStore::new(layout),
        ScopeId::new("default").unwrap(),
    )
}

fn document(source_name: &str, requirement: &str, rule: &str) -> TypedSpecInput {
    TypedSpecInput {
        schema_version: SUPPORTED_SCHEMA_VERSION.0,
        spec: "review".into(),
        declared_by: OWNER.into(),
        adopt_unowned: Vec::new(),
        sources: vec![TypedSourceInput {
            key: "policy".into(),
            id: None,
            name: source_name.into(),
            kind: "document".into(),
            url: None,
            reference: None,
            supersedes: None,
        }],
        requirements: vec![TypedRequirementInput {
            key: "storage".into(),
            id: None,
            statement: requirement.into(),
            description: None,
            sources: vec!["policy".into()],
            refines: None,
            depends_on: None,
            supersedes: None,
            spawned_by: None,
        }],
        rules: vec![TypedRuleInput {
            key: "retain".into(),
            id: None,
            address: None,
            requirement: None,
            requirements: vec!["storage".into()],
            statement: rule.into(),
            name: None,
            description: None,
            resolution_ids: None,
            implementation: None,
        }],
    }
}

fn enroll(store: &StateStore, scope: &ScopeId, kind: NodeType, id: &StableId) {
    let path = shards::path_for(&store.layout, scope, kind);
    let text = std::fs::read_to_string(&path).unwrap();
    let mut values = text
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    let value = values
        .iter_mut()
        .find(|value| value["id"] == id.as_str())
        .unwrap();
    value["schema_version"] = REVIEW_SCHEMA_VERSION.0.into();
    let record = review_families::deserialize_record(kind, value).unwrap();
    let lines = values
        .iter()
        .map(|value| serde_json::to_string(value).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{lines}\n")).unwrap();
    let after = journal::snapshot(&store.layout, &record).unwrap();
    let entry = ReviewEntry {
        schema_version: REVIEW_SCHEMA_VERSION,
        scope_id: scope.clone(),
        record_kind: kind,
        record_id: id.clone(),
        id: journal::new_id(),
        sequence: 1,
        predecessor: None,
        revision: journal::new_id(),
        prior_revision: None,
        before: None,
        after,
        changed_fields: Vec::new(),
        actor: "reviewer".into(),
        request_id: journal::new_id(),
        intent_digest: "sha256:enrollment".into(),
        etag: journal::etag(&record, None).unwrap(),
        outcome: SaveOutcome::Enrolled,
        origin: None,
    };
    journal::write_new(
        &journal::entry_path(&store.layout, scope, &entry.request_id),
        &entry,
    )
    .unwrap();
}

fn enrolled_typed_records(store: &StateStore, scope: &ScopeId) -> Vec<(NodeType, StableId)> {
    let records = vec![
        (
            NodeType::Source,
            store.list_sources(scope).unwrap()[0].id.clone(),
        ),
        (
            NodeType::Requirement,
            store.list_requirements(scope).unwrap()[0].id.clone(),
        ),
        (
            NodeType::Rule,
            store.list_rules(scope).unwrap()[0].id.clone(),
        ),
    ];
    for (kind, id) in &records {
        enroll(store, scope, *kind, id);
    }
    records
}

#[test]
fn typed_updates_capture_owned_enrolled_records() {
    let (_temp, store, scope) = fixture();
    store
        .apply_typed_spec(
            &scope,
            document(
                "Policy one",
                "The system stores records.",
                "The system retains records.",
            ),
        )
        .unwrap();
    let records = enrolled_typed_records(&store, &scope);

    store
        .apply_typed_spec(
            &scope,
            document(
                "Policy two",
                "The system stores durable records.",
                "The system retains durable records.",
            ),
        )
        .unwrap();

    for (kind, id) in records {
        let record = match kind {
            NodeType::Source => store.list_sources(&scope).unwrap()[0].clone().into(),
            NodeType::Requirement => store.list_requirements(&scope).unwrap()[0].clone().into(),
            NodeType::Rule => store.list_rules(&scope).unwrap()[0].clone().into(),
            _ => unreachable!(),
        };
        let head = store.head(&record).unwrap().unwrap();
        assert_eq!(head.record_kind, kind);
        assert_eq!(head.record_id, id);
        assert_eq!(head.sequence, 2);
        assert_eq!(head.actor, OWNER);
    }
}

#[test]
fn typed_omission_refuses_enrolled_deletion() {
    let (_temp, store, scope) = fixture();
    let input = document(
        "Policy one",
        "The system stores records.",
        "The system retains records.",
    );
    store.apply_typed_spec(&scope, input).unwrap();
    let records = enrolled_typed_records(&store, &scope);
    let before = std::fs::read(shards::requirements_path(&store.layout, &scope)).unwrap();
    let empty = TypedSpecInput {
        schema_version: SUPPORTED_SCHEMA_VERSION.0,
        spec: "review".into(),
        declared_by: OWNER.into(),
        adopt_unowned: Vec::new(),
        sources: Vec::new(),
        requirements: Vec::new(),
        rules: Vec::new(),
    };

    let error = store.apply_typed_spec(&scope, empty).unwrap_err();

    assert!(
        error.to_string().contains("cannot delete enrolled"),
        "{error:#}"
    );
    assert_eq!(
        std::fs::read(shards::requirements_path(&store.layout, &scope)).unwrap(),
        before
    );
    assert_eq!(records.len(), 3);
}

fn cascade_document(include_removable: bool) -> TypedSpecInput {
    let mut input = document(
        "Policy one",
        "The system stores records.",
        "The system retains records.",
    );
    input.rules.clear();
    input.requirements.push(TypedRequirementInput {
        key: "removable".into(),
        id: None,
        statement: "The system removes temporary records.".into(),
        description: None,
        sources: Vec::new(),
        refines: None,
        depends_on: None,
        supersedes: None,
        spawned_by: None,
    });
    if !include_removable {
        input.sources.clear();
        input.requirements.pop();
    }
    input
}

#[test]
fn typed_cascade_captures_each_changed_enrolled_kind() {
    let (_temp, store, scope) = fixture();
    store
        .apply_typed_spec(&scope, cascade_document(true))
        .unwrap();
    let source = store.list_sources(&scope).unwrap()[0].id.clone();
    let requirements = store.list_requirements(&scope).unwrap();
    let kept = requirements
        .iter()
        .find(|record| record.statement.contains("stores"))
        .unwrap()
        .id
        .clone();
    let removed = requirements
        .iter()
        .find(|record| record.statement.contains("removes"))
        .unwrap()
        .id
        .clone();
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope.clone(),
            id: StableId::new("resolution_cascade").unwrap(),
            title: "Keep the durable record".into(),
            requirement_ids: vec![kept.clone(), removed.clone()],
            supersedes: Vec::new(),
            position: "Keep the durable record.".into(),
            rationale: "The durable record is required.".into(),
            status: ResolutionStatus::Proposed,
            context: None,
            enforcement: None,
            confidence: None,
            inputs: Vec::new(),
            made_by: None,
            approved_by: None,
            approved_at: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_boundary(CreateBoundaryInput {
            scope_id: scope.clone(),
            id: StableId::new("boundary_cascade").unwrap(),
            requirement_id: kept.clone(),
            statement: "The policy is the boundary.".into(),
            source_ref: Some(SourceReference {
                source_id: source.clone(),
                clause: None,
            }),
        })
        .unwrap();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: StableId::new("topic_cascade").unwrap(),
            requirement_id: kept,
            title: "Durable storage".into(),
            status: TopicStatus::Open,
            links: vec![ArtifactLink {
                target_type: ArtifactLinkTargetType::Source,
                target_id: source,
            }],
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            scope_id: scope.clone(),
            id: StableId::new("question_cascade").unwrap(),
            topic_id: StableId::new("topic_cascade").unwrap(),
            question: "Does the temporary record remain?".into(),
            resolution_method: ResolutionMethod::Research,
            status: QuestionStatus::Open,
            answer: None,
            links: vec![ArtifactLink {
                target_type: ArtifactLinkTargetType::Requirement,
                target_id: removed.clone(),
            }],
            contradicts: Some(removed),
            resolution_id: None,
        })
        .unwrap();
    let records = [
        (NodeType::Resolution, "resolution_cascade"),
        (NodeType::Boundary, "boundary_cascade"),
        (NodeType::Topic, "topic_cascade"),
        (NodeType::Question, "question_cascade"),
    ];
    for (kind, id) in records {
        enroll(&store, &scope, kind, &StableId::new(id).unwrap());
    }

    store
        .apply_typed_spec(&scope, cascade_document(false))
        .unwrap();

    for (kind, id) in records {
        let record = match kind {
            NodeType::Resolution => store.list_resolutions(&scope).unwrap()[0].clone().into(),
            NodeType::Boundary => store.list_boundaries(&scope).unwrap()[0].clone().into(),
            NodeType::Topic => store.list_topics(&scope).unwrap()[0].clone().into(),
            NodeType::Question => store.list_questions(&scope).unwrap()[0].clone().into(),
            _ => unreachable!(),
        };
        let head = store.head(&record).unwrap().unwrap();
        assert_eq!(head.record_id.as_str(), id);
        assert_eq!(head.sequence, 2, "{kind:?}");
        assert_eq!(head.actor, OWNER);
    }
}
