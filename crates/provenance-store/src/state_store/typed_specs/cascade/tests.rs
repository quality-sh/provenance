use super::*;
use crate::{
    layout::ProvenanceLayout,
    state_store::{
        CreateBoundaryInput, CreateQuestionInput, CreateRequirementInput, CreateTopicInput,
    },
};
use camino::Utf8Path;
use provenance_core::{
    Manifest, QuestionStatus, RepoPathPrefix, RequirementStatus, ResolutionMethod, TopicStatus,
};

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn fixture() -> (tempfile::TempDir, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let layout = ProvenanceLayout::new(Utf8Path::from_path(temp.path()).unwrap());
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&Manifest::default_with_scope(
            scope.clone(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    let store = StateStore::new(layout);
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: id("requirement_cascade_writer"),
            statement: "The system stores records.".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: None,
            refines: None,
            depends_on: Vec::new(),
            supersedes: Vec::new(),
            spawned_by: None,
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: id("topic_cascade_writer"),
            requirement_id: id("requirement_cascade_writer"),
            title: "Storage topic".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            scope_id: scope.clone(),
            id: id("question_cascade_writer"),
            topic_id: id("topic_cascade_writer"),
            question: "Does storage work?".into(),
            resolution_method: ResolutionMethod::Research,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
            contradicts: None,
        })
        .unwrap();
    store
        .create_boundary(CreateBoundaryInput {
            scope_id: scope.clone(),
            id: id("boundary_cascade_writer"),
            requirement_id: id("requirement_cascade_writer"),
            statement: "Storage boundary".into(),
            source_ref: None,
        })
        .unwrap();
    (temp, store, scope)
}

fn publish_changed_cascade(kind: NodeType) {
    let (_temp, store, scope) = fixture();
    let record = match kind {
        NodeType::Topic => id("topic_cascade_writer"),
        NodeType::Question => id("question_cascade_writer"),
        NodeType::Boundary => id("boundary_cascade_writer"),
        _ => unreachable!(),
    };
    let created = store.record_edit_state(&scope, kind, &record).unwrap();
    let mut topics = store.list_topics(&scope).unwrap();
    let mut questions = store.list_questions(&scope).unwrap();
    let mut boundaries = store.list_boundaries(&scope).unwrap();
    match kind {
        NodeType::Topic => topics[0].title = "Changed storage topic".into(),
        NodeType::Question => questions[0].question = "Does changed storage work?".into(),
        NodeType::Boundary => boundaries[0].statement = "Changed storage boundary".into(),
        _ => unreachable!(),
    }
    Cascade {
        rules: BTreeSet::new(),
        resolutions: store.list_resolutions(&scope).unwrap(),
        topics,
        questions,
        boundaries,
        changes: Vec::new(),
    }
    .publish(&store, &scope)
    .unwrap();

    let changed = store.record_edit_state(&scope, kind, &record).unwrap();
    assert!(created.revision.is_some());
    assert_ne!(changed.revision, created.revision);
}

#[test]
fn topic_cascade_writer_records_an_enrolled_change() {
    publish_changed_cascade(NodeType::Topic);
}

#[test]
fn question_cascade_writer_records_an_enrolled_change() {
    publish_changed_cascade(NodeType::Question);
}

#[test]
fn boundary_cascade_writer_records_an_enrolled_change() {
    publish_changed_cascade(NodeType::Boundary);
}
