use crate::{
    cache::review_families,
    layout::ProvenanceLayout,
    shards,
    state_store::{
        readers::read_jsonl, record_stamps::GraphRecord, CreateBoundaryInput, CreateDomainInput,
        CreateQuestionInput, CreateResolutionInput, CreateTopicInput, StateStore, TypedSpecInput,
    },
    write_error::{WriteError, WriteFailure},
};
use camino::Utf8Path;
use provenance_core::{
    protocol::{
        TypedAdoptionTarget, TypedDeclarationKind, TypedRequirementInput, TypedRuleInput,
        TypedSourceInput,
    },
    review::REVIEW_SCHEMA_VERSION,
    ArtifactLink, ArtifactLinkTargetType, Boundary, Domain, NodeType, Question, QuestionStatus,
    Requirement, Resolution, ResolutionMethod, ResolutionStatus, Rule, ScopeId, Source,
    SourceReference, StableId, Topic, TopicStatus, SUPPORTED_SCHEMA_VERSION,
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
    match kind {
        NodeType::Source => enroll_as::<Source>(store, &path, id),
        NodeType::Requirement => enroll_as::<Requirement>(store, &path, id),
        NodeType::Resolution => enroll_as::<Resolution>(store, &path, id),
        NodeType::Rule => enroll_as::<Rule>(store, &path, id),
        NodeType::Domain => enroll_as::<Domain>(store, &path, id),
        NodeType::Boundary => enroll_as::<Boundary>(store, &path, id),
        NodeType::Topic => enroll_as::<Topic>(store, &path, id),
        NodeType::Question => enroll_as::<Question>(store, &path, id),
    }
}

/// Enrolls one stored record through the native creation writer.
fn enroll_as<T: GraphRecord + serde::de::DeserializeOwned>(
    store: &StateStore,
    path: &camino::Utf8Path,
    id: &StableId,
) {
    let stored = read_jsonl::<T>(store, path)
        .unwrap()
        .into_iter()
        .find(|record| record.id() == id)
        .unwrap();
    store
        .create_native_record::<T>(path, id, move |_| Ok(stored))
        .unwrap();
}

/// The stored record of one kind and id, without its record stamps.
fn content(
    store: &StateStore,
    scope: &ScopeId,
    kind: NodeType,
    id: &StableId,
) -> serde_json::Value {
    let record = review_families::record(store, scope, kind, id).unwrap();
    assert_eq!(record.schema_version(), REVIEW_SCHEMA_VERSION);
    provenance_core::model::record_stamps::content_value(&record).unwrap()
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

fn clear_typed_owner(store: &StateStore, scope: &ScopeId, kind: NodeType, id: &StableId) {
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
    value["declared_by"] = serde_json::Value::Null;
    value["declaration_address"] = serde_json::Value::Null;
    let lines = values
        .iter()
        .map(|value| serde_json::to_string(value).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(path, format!("{lines}\n")).unwrap();
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

    let expected = [
        ("name", "Policy two"),
        ("statement", "The system stores durable records."),
        ("statement", "The system retains durable records."),
    ];
    for ((kind, id), (field, value)) in records.iter().zip(expected) {
        assert_eq!(content(&store, &scope, *kind, id)[field], value);
    }
    let pending = store
        .requirement_decision_state(&scope, &records[1].1)
        .unwrap()
        .pending
        .unwrap();
    assert_eq!(pending.actor, OWNER);
}

#[test]
fn typed_adoption_captures_unowned_enrolled_records() {
    let (_temp, store, scope) = fixture();
    let mut input = document(
        "Policy one",
        "The system stores records.",
        "The system retains records.",
    );
    store.apply_typed_spec(&scope, input.clone()).unwrap();
    let records = vec![
        (
            NodeType::Source,
            TypedDeclarationKind::Source,
            store.list_sources(&scope).unwrap()[0].id.clone(),
        ),
        (
            NodeType::Requirement,
            TypedDeclarationKind::Requirement,
            store.list_requirements(&scope).unwrap()[0].id.clone(),
        ),
        (
            NodeType::Rule,
            TypedDeclarationKind::Rule,
            store.list_rules(&scope).unwrap()[0].id.clone(),
        ),
    ];
    for (kind, _, id) in &records {
        clear_typed_owner(&store, &scope, *kind, id);
        enroll(&store, &scope, *kind, id);
    }
    input.sources[0].id = Some(records[0].2.as_str().to_owned());
    input.requirements[0].id = Some(records[1].2.as_str().to_owned());
    input.rules[0].id = Some(records[2].2.as_str().to_owned());
    input.adopt_unowned = records
        .iter()
        .map(|(_, kind, id)| TypedAdoptionTarget {
            kind: *kind,
            id: id.as_str().to_owned(),
        })
        .collect();

    store.apply_typed_spec(&scope, input).unwrap();

    for (kind, _, id) in records {
        assert_eq!(content(&store, &scope, kind, &id)["declared_by"], OWNER);
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

    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::EnrolledRecordDeletionConflict {
            record_kind: NodeType::Source,
            record_id,
        } if record_id == records[0].1
    ));
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
        input.requirements[0].sources.clear();
    }
    input
}

fn seed_cascade_dependants(
    store: &StateStore,
    scope: &ScopeId,
    source: StableId,
    kept: StableId,
    removed: StableId,
) -> [(NodeType, &'static str); 4] {
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
    store
        .create_domain(CreateDomainInput {
            scope_id: scope.clone(),
            id: StableId::new("domain_unchanged").unwrap(),
            name: "Storage".into(),
            description: None,
            color: None,
        })
        .unwrap();
    let records = [
        (NodeType::Resolution, "resolution_cascade"),
        (NodeType::Boundary, "boundary_cascade"),
        (NodeType::Topic, "topic_cascade"),
        (NodeType::Question, "question_cascade"),
    ];
    for (kind, id) in records {
        enroll(store, scope, kind, &StableId::new(id).unwrap());
    }
    let domain_id = StableId::new("domain_unchanged").unwrap();
    enroll(store, scope, NodeType::Domain, &domain_id);
    records
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
    let records = seed_cascade_dependants(&store, &scope, source, kept, removed);
    let domain_id = StableId::new("domain_unchanged").unwrap();
    let before = records
        .iter()
        .map(|(kind, id)| content(&store, &scope, *kind, &StableId::new(*id).unwrap()))
        .collect::<Vec<_>>();
    let domain = content(&store, &scope, NodeType::Domain, &domain_id);

    store
        .apply_typed_spec(&scope, cascade_document(false))
        .unwrap();

    for ((kind, id), before) in records.iter().zip(before) {
        let after = content(&store, &scope, *kind, &StableId::new(*id).unwrap());
        assert_ne!(after, before, "{kind:?}");
    }
    assert_eq!(
        content(&store, &scope, NodeType::Domain, &domain_id),
        domain
    );
}

mod typed_adoption_deletion_tests;
mod typed_adoption_publication_tests;
