use super::{root_of, seeded_store, sid};
use crate::operations::{queries, read_policy::ReadPolicy};
use crate::state_store::{
    AddSourceReferenceInput, CreateQuestionInput, CreateRequirementInput, CreateResolutionInput,
    CreateRuleInput, CreateTopicInput,
};
use provenance_core::{NodeType, RequirementStatus};
use provenance_macros::verifies;
use serde_json::json;

fn input<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> T {
    serde_json::from_value(value).unwrap()
}

#[tokio::test]
#[verifies("rule_agent_review_request_includes_link", examples)]
async fn inverse_document_read_follows_each_canonical_membership_rule() {
    let (dir, store, scope) = seeded_store();
    store
        .create_source(input(json!({
            "scope_id":scope, "id":"source_policy", "name":"Policy",
            "source_type":"policy", "supersedes":[]
        })))
        .unwrap();
    store
        .add_source_reference(AddSourceReferenceInput {
            scope_id: scope.clone(),
            source_id: sid("source_policy"),
            requirement_id: sid("req_overtime"),
            clause: None,
        })
        .unwrap();
    store
        .create_resolution(input::<CreateResolutionInput>(json!({
            "scope_id":scope, "id":"resolution_policy", "title":"Policy decision",
            "position":"Use the policy.", "rationale":"The policy is current.",
            "status":"draft", "requirement_ids":["req_overtime"], "supersedes":[],
            "inputs":[]
        })))
        .unwrap();
    store
        .create_rule(input::<CreateRuleInput>(json!({
            "scope_id":scope, "id":"rule_via_resolution",
            "statement":"The system uses the decision.", "status":"draft",
            "severity":"medium", "requirement_ids":["req_overtime"],
            "resolution_ids":["resolution_policy"]
        })))
        .unwrap();
    store
        .create_topic(input::<CreateTopicInput>(json!({
            "scope_id":scope, "id":"topic_policy", "requirement_id":"req_overtime",
            "title":"Policy topic", "status":"open", "links":[]
        })))
        .unwrap();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: sid("req_contradicted"),
            statement: "The other requirement applies.".into(),
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
        .create_question(input::<CreateQuestionInput>(json!({
            "scope_id":scope, "id":"question_policy", "topic_id":"topic_policy",
            "question":"Which policy?", "resolution_method":"research", "status":"open",
            "links":[], "contradicts":"req_contradicted"
        })))
        .unwrap();

    let root = root_of(&dir);
    for (kind, id) in [
        (NodeType::Source, "source_policy"),
        (NodeType::Requirement, "req_overtime"),
        (NodeType::Resolution, "resolution_policy"),
        (NodeType::Rule, "rule_via_resolution"),
        (NodeType::Domain, "domain_payroll"),
        (NodeType::Boundary, "boundary_no_backpay"),
        (NodeType::Topic, "topic_policy"),
        (NodeType::Question, "question_policy"),
    ] {
        let roots = queries::containing_review_documents(
            Some(root.clone()),
            &scope,
            ReadPolicy::default(),
            kind,
            &sid(id),
        )
        .await
        .unwrap()
        .result;
        assert_eq!(roots, [sid("req_overtime")], "wrong root for {id}");
    }
}
