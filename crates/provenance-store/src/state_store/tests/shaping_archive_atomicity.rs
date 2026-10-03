use super::seeded_requirement_store;
use crate::{state_store::StateStore, test_probes, write_error::{WriteError, WriteFailure}};
use camino::Utf8Path;
use provenance_core::{NodeType, QuestionStatus, ScopeId, StableId, TopicStatus};
use serde_json::json;

fn id(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn seed() -> (tempfile::TempDir, StateStore, ScopeId) {
    let (directory, store, scope) = seeded_requirement_store();
    store.create_topic(serde_json::from_value(json!({
        "scope_id":"default", "id":"topic_archive", "requirement_id":"req_overtime",
        "title":"Archive topic", "status":"open", "links":[]
    })).unwrap()).unwrap();
    for question in ["question_one", "question_two"] {
        store.create_question(serde_json::from_value(json!({
            "scope_id":"default", "id":question, "topic_id":"topic_archive",
            "question":"Keep this history?", "resolution_method":"research",
            "status":"open", "answer":null, "links":[], "resolution_id":null,
            "contradicts":null
        })).unwrap()).unwrap();
    }
    (directory, store, scope)
}

fn etag(store: &StateStore, scope: &ScopeId, kind: NodeType, record_id: &str) -> String {
    store.review_entries(scope).unwrap().into_iter()
        .filter(|entry| entry.record_kind == kind && entry.record_id.as_str() == record_id)
        .max_by_key(|entry| entry.sequence).unwrap().etag
}

fn archive_input(store: &StateStore, scope: &ScopeId) -> crate::state_store::UpdateTopicInput {
    serde_json::from_value(json!({
        "scope_id":"default", "id":"topic_archive",
        "expected_etag":etag(store, scope, NodeType::Topic, "topic_archive"),
        "status":"archived", "archived_in_commit":{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
    })).unwrap()
}

fn assert_open(store: &StateStore, scope: &ScopeId) {
    let topic = store.list_topics(scope).unwrap().into_iter()
        .find(|topic| topic.id == id("topic_archive")).unwrap();
    assert_eq!(topic.status, TopicStatus::Open);
    assert!(topic.archived_in_commit.is_none());
    for question in store.list_questions(scope).unwrap().into_iter()
        .filter(|question| question.topic_id == id("topic_archive"))
    {
        assert_eq!(question.status, QuestionStatus::Open);
        assert!(question.archived_in_commit.is_none());
    }
}

fn assert_archived(store: &StateStore, scope: &ScopeId) {
    let topic = store.list_topics(scope).unwrap().into_iter()
        .find(|topic| topic.id == id("topic_archive")).unwrap();
    assert_eq!(topic.status, TopicStatus::Archived);
    assert!(topic.archived_in_commit.is_some());
    for question in store.list_questions(scope).unwrap().into_iter()
        .filter(|question| question.topic_id == id("topic_archive"))
    {
        assert_eq!(question.status, QuestionStatus::Archived);
        assert!(question.archived_in_commit.is_some());
    }
}

#[test]
fn stale_topic_archive_returns_a_typed_conflict_and_changes_nothing() {
    let (_directory, store, scope) = seed();
    let stale = etag(&store, &scope, NodeType::Topic, "topic_archive");
    store.edit_topic(serde_json::from_value(json!({
        "scope_id":"default", "id":"topic_archive", "expected_etag":stale,
        "title":"Changed title"
    })).unwrap()).unwrap();
    let error = store.edit_topic(serde_json::from_value(json!({
        "scope_id":"default", "id":"topic_archive", "expected_etag":stale,
        "status":"archived", "archived_in_commit":{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
    })).unwrap()).unwrap_err();
    assert!(matches!(WriteError(error).safe(), WriteFailure::RecordEditConflict {
        record_kind: NodeType::Topic, ..
    }));
    let topic = store.list_topics(&scope).unwrap().into_iter()
        .find(|topic| topic.id == id("topic_archive")).unwrap();
    assert_eq!(topic.title, "Changed title");
    assert_eq!(topic.status, TopicStatus::Open);
    for question in store.list_questions(&scope).unwrap() {
        assert_eq!(question.status, QuestionStatus::Open);
    }
}

#[test]
fn failure_after_topic_mutation_publishes_no_archive_changes() {
    let (_directory, store, scope) = seed();
    test_probes::crash_at("topic_archive_after_topic");
    assert!(store.edit_topic(archive_input(&store, &scope)).is_err());
    test_probes::disarm("topic_archive_after_topic");
    assert_open(&store, &scope);
}

#[test]
fn failure_during_question_work_publishes_no_archive_changes() {
    let (_directory, store, scope) = seed();
    test_probes::crash_at("topic_archive_question_changed");
    assert!(store.edit_topic(archive_input(&store, &scope)).is_err());
    test_probes::disarm("topic_archive_question_changed");
    assert_open(&store, &scope);
}

#[test]
fn crash_child() {
    let Ok(root) = std::env::var("PROVENANCE_TOPIC_ARCHIVE_CRASH_ROOT") else { return; };
    let phase = std::env::var("PROVENANCE_TOPIC_ARCHIVE_CRASH_PHASE").unwrap();
    let phase = match phase.as_str() {
        "state_marker_prepared" => "state_marker_prepared",
        "state_installed" => "state_installed",
        _ => panic!("unknown crash phase"),
    };
    let store = StateStore::new(crate::layout::ProvenanceLayout::new(Utf8Path::new(&root)));
    let scope = ScopeId::new("default").unwrap();
    test_probes::arm(phase, || std::process::exit(86));
    store.edit_topic(archive_input(&store, &scope)).unwrap();
    panic!("crash phase was not reached");
}

#[test]
fn archive_recovery_restores_complete_old_or_new_state() {
    for (phase, committed) in [("state_marker_prepared", false), ("state_installed", true)] {
        let (directory, _store, scope) = seed();
        let root = Utf8Path::from_path(directory.path()).unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "state_store::tests::shaping_archive_atomicity::crash_child", "--nocapture"])
            .env("PROVENANCE_TOPIC_ARCHIVE_CRASH_ROOT", root.as_str())
            .env("PROVENANCE_TOPIC_ARCHIVE_CRASH_PHASE", phase)
            .stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null())
            .status().unwrap();
        assert_eq!(status.code(), Some(86), "{phase}");
        let reopened = StateStore::new(crate::layout::ProvenanceLayout::new(root));
        if committed { assert_archived(&reopened, &scope); } else { assert_open(&reopened, &scope); }
    }
}
