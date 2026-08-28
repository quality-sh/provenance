use super::proposal_input;
use crate::state_store::{
    CreateDomainInput, CreateQuestionInput, CreateRequirementInput, CreateTopicInput,
};
use provenance_core::{
    IdeationTargetType, PromotionState, QuestionStatus, RequirementStatus, ResolutionMethod,
    StableId, TopicStatus,
};
use provenance_macros::verifies;

fn seed_topic_territory() -> (
    tempfile::TempDir,
    crate::state_store::StateStore,
    provenance_core::ScopeId,
) {
    let (dir, store, scope) = super::initialized_store();
    seed_topic_records(&store, &scope);
    seed_proposals(&store, &scope);
    (dir, store, scope)
}

fn seed_topic_records(store: &crate::state_store::StateStore, scope: &provenance_core::ScopeId) {
    store
        .create_domain(CreateDomainInput {
            scope_id: scope.clone(),
            id: StableId::new("domain_payroll").unwrap(),
            name: "Payroll".into(),
            description: None,
            color: None,
        })
        .unwrap();
    store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("req_overtime").unwrap(),
            statement: "Overtime must be correct".into(),
            description: None,
            status: RequirementStatus::Active,
            domain_id: Some(StableId::new("domain_payroll").unwrap()),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: StableId::new("topic_overtime").unwrap(),
            requirement_id: StableId::new("req_overtime").unwrap(),
            title: "Overtime".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            scope_id: scope.clone(),
            id: StableId::new("question_threshold").unwrap(),
            topic_id: StableId::new("topic_overtime").unwrap(),
            question: "Which threshold applies?".into(),
            resolution_method: ResolutionMethod::Grill,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
        })
        .unwrap();
}

fn seed_proposals(store: &crate::state_store::StateStore, scope: &provenance_core::ScopeId) {
    let cases = [
        (
            "proposal_evidence",
            IdeationTargetType::Requirement,
            "req_unrelated",
            Some("src/payroll.rs"),
        ),
        (
            "proposal_question",
            IdeationTargetType::Question,
            "question_threshold",
            None,
        ),
        (
            "proposal_domain",
            IdeationTargetType::Domain,
            "domain_payroll",
            None,
        ),
        (
            "proposal_unrelated",
            IdeationTargetType::Question,
            "question_elsewhere",
            Some("src/leave.rs"),
        ),
    ];
    for (id, target_type, target_id, path) in cases {
        store
            .create_proposal_card(proposal_input(
                scope,
                id,
                target_type,
                target_id,
                path,
                PromotionState::Proposed,
            ))
            .unwrap();
    }
}

#[test]
#[verifies("rule_proposal_surfacing", examples)]
fn topic_claim_evaluates_evidence_question_and_domain_triggers() {
    let (_dir, store, scope) = seed_topic_territory();

    let claim = store
        .claim_topic(
            &scope,
            &StableId::new("topic_overtime").unwrap(),
            "agent-one",
            ["src/payroll.rs"],
        )
        .unwrap();

    assert_eq!(
        claim
            .surfaced_proposals
            .iter()
            .map(|surface| surface.proposal.id.as_str())
            .collect::<Vec<_>>(),
        vec!["proposal_domain", "proposal_evidence", "proposal_question"]
    );
    let reasons = claim
        .surfaced_proposals
        .iter()
        .map(|surface| serde_json::to_value(&surface.reasons).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        reasons,
        vec![
            serde_json::json!([{
                "trigger": "territory",
                "target": {"artifact_type": "domain", "artifact_id": "domain_payroll"}
            }]),
            serde_json::json!([{"trigger": "evidence_site", "path": "src/payroll.rs"}]),
            serde_json::json!([{
                "trigger": "territory",
                "target": {"artifact_type": "question", "artifact_id": "question_threshold"}
            }]),
        ]
    );
}
