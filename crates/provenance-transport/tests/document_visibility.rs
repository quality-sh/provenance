//! Document reads over records that other proposals name and over references
//! that no longer resolve.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
    pub mod resource_http;
}

use provenance_core::{
    NodeType, QuestionStatus, RequirementStatus, ResolutionMethod, ScopeId, StableId, TopicStatus,
};
use provenance_macros::verifies;
use provenance_store::state_store::{
    CreateQuestionInput, CreateRequirementInput, CreateTopicInput, StateStore,
};
use serde_json::{json, Value};
use support::records::{allow_reviewer, Repository};
use support::resource_http::{call, host};

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn entry_ids(document: &Value) -> Vec<String> {
    document["data"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|entry| entry["node"]["id"].as_str().map(str::to_owned))
        .collect()
}

fn requirement(id: &str, depends_on: Vec<StableId>) -> CreateRequirementInput {
    CreateRequirementInput {
        scope_id: scope(),
        id: sid(id),
        statement: "The system keeps each copy.".into(),
        description: None,
        status: RequirementStatus::Active,
        domain_id: None,
        refines: Some(sid("req_shared")),
        depends_on,
        supersedes: vec![],
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

#[tokio::test]
#[verifies("rule_review_document_hides_rejected_new_records", examples)]
async fn rejected_candidate_proposal_keeps_its_target_in_the_filtered_document() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let store = StateStore::new(repo.layout.clone());
    store
        .create_topic(CreateTopicInput {
            scope_id: scope(),
            id: sid("topic_store"),
            requirement_id: sid("req_shared"),
            title: "Storage".into(),
            status: TopicStatus::Open,
            links: vec![],
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            scope_id: scope(),
            id: sid("question_store"),
            topic_id: sid("topic_store"),
            question: "Which store holds the record?".into(),
            resolution_method: ResolutionMethod::Research,
            status: QuestionStatus::Answered,
            answer: Some("The local store.".into()),
            links: vec![],
            resolution_id: None,
            contradicts: None,
        })
        .unwrap();
    // A candidate answer names the Question; rejecting it decides nothing about the Question.
    store
        .create_proposal_card(
            serde_json::from_value(json!({
                "scope_id":"default","id":"prop_answer","proposal_key":"question/answer",
                "proposal_type":"resolution_candidate","title":"Candidate answer",
                "summary":"The candidate answer.","promotion_state":"proposed",
                "traceability":{"target":{"artifact_type":"question","artifact_id":"question_store"},
                    "source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},
                "builds_on":[]
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .create_disposition(
            serde_json::from_value(json!({
                "scope_id":"default","id":"disp_answer","proposal_id":"prop_answer",
                "decision":"rejected","rationale":"Not this answer.",
                "actor":{"identity_type":"human","id":"reviewer"}
            }))
            .unwrap(),
        )
        .unwrap();
    let host = host(&repo, false);

    let (status, filtered) = call(
        &host,
        "GET",
        "/requirements/req_shared/document?exclude_terminal=true",
        None,
    )
    .await;
    assert_eq!(status, 200, "{filtered}");
    assert!(
        entry_ids(&filtered).contains(&"question_store".to_owned()),
        "{filtered}"
    );
    assert_eq!(
        filtered["data"]["review_totals"]["rejected"], 0,
        "{filtered}"
    );
    let (status, all) = call(
        &host,
        "GET",
        "/requirements/req_shared/document?exclude_terminal=false",
        None,
    )
    .await;
    assert_eq!(status, 200, "{all}");
    assert_eq!(entry_ids(&all), entry_ids(&filtered));
}

/// Implementation aid: this checks that a reference to a missing record leaves the document readable.
#[tokio::test]
async fn document_skips_a_reference_to_a_missing_record() {
    let repo = Repository::new("The shared graph is readable.");
    let store = StateStore::new(repo.layout.clone());
    store
        .create_requirement(requirement("req_cited", vec![]))
        .unwrap();
    store
        .create_requirement(requirement("req_citing", vec![sid("req_cited")]))
        .unwrap();
    // The state a merge leaves: one branch removed req_cited while the other kept citing it.
    let path = provenance_store::shards::path_for(&repo.layout, &scope(), NodeType::Requirement);
    let kept = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .filter(|line| !line.contains("\"id\":\"req_cited\""))
        .fold(String::new(), |mut kept, line| {
            kept.push_str(line);
            kept.push('\n');
            kept
        });
    std::fs::write(&path, kept).unwrap();
    let host = host(&repo, false);

    let (status, document) = call(
        &host,
        "GET",
        "/requirements/req_shared/document?exclude_terminal=true",
        None,
    )
    .await;
    assert_eq!(status, 200, "{document}");
    let ids = entry_ids(&document);
    assert!(ids.contains(&"req_citing".to_owned()), "{document}");
    assert!(!ids.contains(&"req_cited".to_owned()), "{document}");
}

#[tokio::test]
#[verifies("rule_review_document_hides_rejected_new_records", examples)]
async fn legacy_review_acceptance_keeps_a_record_after_a_rejected_change() {
    let repo = Repository::new("The shared graph is readable.");
    allow_reviewer(&repo);
    let store = StateStore::new(repo.layout.clone());
    store
        .create_requirement(requirement("req_reviewed", vec![]))
        .unwrap();
    // An older review submission carries no revision binding; its acceptance
    // is still a decision of the record's own review.
    let records = provenance_store::shards::path_for(&repo.layout, &scope(), NodeType::Requirement);
    let ideation = records.parent().unwrap().parent().unwrap().join("ideation");
    std::fs::create_dir_all(&ideation).unwrap();
    let append = |name: &str, row: Value| {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(ideation.join(name))
            .unwrap();
        writeln!(file, "{row}").unwrap();
    };
    append(
        "proposal_cards.jsonl",
        json!({
            "schema_version":2,"scope_id":"default","id":"prop_legacy_review",
            "proposal_key":"legacy-review","proposal_type":"record_revision",
            "title":"Review req_reviewed","summary":"The older review.",
            "traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_reviewed"},
                "source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},
            "promotion_state":"accepted"
        }),
    );
    append(
        "dispositions.jsonl",
        json!({
            "schema_version":2,"scope_id":"default","id":"disp_legacy_review",
            "proposal_id":"prop_legacy_review","decision":"accepted","rationale":"Ratified offline.",
            "actor":{"identity_type":"human","id":"reviewer"},
            "canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_reviewed"}
        }),
    );
    let submitted = store
        .submit_record_review(
            serde_json::from_value(json!({
                "scope_id":"default","actor":"author","record_kind":"requirement",
                "record_id":"req_reviewed","declared_by":null,"title":"Review again",
                "summary":"Review the changed record.","source_ids":[],
                "evidence_references":[],"builds_on":[],"expected_revision":null,"revises":null
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .decide_record_review(
            serde_json::from_value(json!({
                "scope_id":"default","actor":{"identity_type":"human","id":"reviewer"},
                "proposal_id":submitted.id,"decision":"rejected","rationale":"Not this change.",
                "canonical_artifact":null,"declared_by":null,"feedback":null
            }))
            .unwrap(),
        )
        .unwrap();
    let host = host(&repo, false);

    let (status, filtered) = call(
        &host,
        "GET",
        "/requirements/req_shared/document?exclude_terminal=true",
        None,
    )
    .await;
    assert_eq!(status, 200, "{filtered}");
    assert!(
        entry_ids(&filtered).contains(&"req_reviewed".to_owned()),
        "{filtered}"
    );
}
