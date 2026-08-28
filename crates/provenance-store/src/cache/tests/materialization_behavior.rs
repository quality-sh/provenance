use super::super::*;
use super::fixtures::*;
use crate::state_store::{
    CreateQuestionInput, CreateResolutionInput, CreateSourceInput, CreateTopicInput, StateStore,
};
use provenance_core::{
    QuestionStatus, ResolutionInput, ResolutionInputType, ResolutionMethod, ResolutionStatus,
    SourceType, TopicStatus,
};

#[tokio::test]
async fn materialize_rejects_missing_disposition_canonical_artifact_before_cache_changes() {
    let (_dir, layout, scope) = empty_layout();
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let proposals = crate::shards::proposal_cards_path(&layout, &scope);
    std::fs::create_dir_all(proposals.parent().unwrap()).unwrap();
    std::fs::write(
        proposals,
        r#"{"schema_version":1,"scope_id":"default","id":"proposal_a","proposal_key":"a","proposal_type":"requirement_candidate","title":"A","summary":"A","traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_missing"},"source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},"promotion_state":"proposed"}
"#,
    )
    .unwrap();
    let dispositions = crate::shards::dispositions_path(&layout, &scope);
    std::fs::write(
        dispositions,
        r#"{"schema_version":1,"scope_id":"default","id":"disposition_a","proposal_id":"proposal_a","decision":"rejected","rationale":"Reviewed","actor":{"identity_type":"human","id":"reviewer"},"canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_missing"}}
"#,
    )
    .unwrap();

    let error = materialize_state(&layout).await.unwrap_err().to_string();

    assert!(
        error.contains("canonical artifact does not exist"),
        "{error}"
    );
    assert!(!layout.cache_db_path().exists());
}

#[tokio::test]
async fn materialize_rejects_misfiled_disposition_target_before_cache_changes() {
    let (_dir, layout, scope) = empty_layout();
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let requirements = crate::shards::requirements_path(&layout, &scope);
    std::fs::create_dir_all(requirements.parent().unwrap()).unwrap();
    std::fs::write(
        requirements,
        r#"{"schema_version":1,"scope_id":"other","id":"req_misfiled","statement":"Misfiled","status":"active"}
"#,
    )
    .unwrap();
    let proposals = crate::shards::proposal_cards_path(&layout, &scope);
    std::fs::create_dir_all(proposals.parent().unwrap()).unwrap();
    std::fs::write(
        proposals,
        r#"{"schema_version":1,"scope_id":"default","id":"proposal_a","proposal_key":"a","proposal_type":"requirement_candidate","title":"A","summary":"A","traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_misfiled"},"source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},"promotion_state":"proposed"}
"#,
    )
    .unwrap();
    std::fs::write(
        crate::shards::dispositions_path(&layout, &scope),
        r#"{"schema_version":1,"scope_id":"default","id":"disposition_a","proposal_id":"proposal_a","decision":"rejected","rationale":"Reviewed","actor":{"identity_type":"human","id":"reviewer"},"canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_misfiled"}}
"#,
    )
    .unwrap();

    let error = materialize_state(&layout).await.unwrap_err().to_string();

    assert!(
        error.contains("canonical artifact does not exist"),
        "{error}"
    );
    assert!(!layout.cache_db_path().exists());
}

#[tokio::test]
async fn materialize_caches_generic_disposition_external_action() {
    let (_dir, layout, scope) = seeded_layout();
    let mut manifest: provenance_core::Manifest =
        serde_json::from_slice(&std::fs::read(layout.manifest_path()).unwrap()).unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
    let proposals = crate::shards::proposal_cards_path(&layout, &scope);
    std::fs::create_dir_all(proposals.parent().unwrap()).unwrap();
    std::fs::write(
        proposals,
        r#"{"schema_version":1,"scope_id":"default","id":"proposal_a","proposal_key":"a","proposal_type":"requirement_candidate","title":"A","summary":"A","traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_schads_overtime"},"source_ids":[],"evidence_references":[],"supporting_claim_ids":[]},"promotion_state":"proposed"}
"#,
    )
    .unwrap();
    std::fs::write(
        crate::shards::dispositions_path(&layout, &scope),
        r#"{"schema_version":1,"scope_id":"default","id":"disposition_a","proposal_id":"proposal_a","decision":"rejected","rationale":"Reviewed","actor":{"identity_type":"human","id":"reviewer"},"canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_schads_overtime"},"external_action":{"system":"github","scope":"acme/payroll","kind":"issue","key":"44"}}
"#,
    )
    .unwrap();

    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let action: String =
        sqlx::query_scalar("SELECT external_action FROM dispositions WHERE id = ?")
            .bind("disposition_a")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&action).unwrap(),
        serde_json::json!({"system":"github","scope":"acme/payroll","kind":"issue","key":"44"})
    );
}

#[tokio::test]
async fn materialize_state_caches_fog_resolution_method_and_claim_state() {
    let (_dir, layout, scope) = seeded_layout();
    let store = StateStore::new(layout.clone());
    store
        .set_requirement_fog(
            &scope,
            &sid("req_schads_overtime"),
            Some("sleepover rules; something about broken shifts".into()),
        )
        .unwrap();
    store
        .create_topic(CreateTopicInput {
            scope_id: scope.clone(),
            id: sid("topic_overtime"),
            requirement_id: sid("req_schads_overtime"),
            title: "Overtime eligibility".into(),
            status: TopicStatus::Open,
            links: Vec::new(),
        })
        .unwrap();
    store
        .create_question(CreateQuestionInput {
            scope_id: scope.clone(),
            id: sid("question_threshold"),
            topic_id: sid("topic_overtime"),
            question: "Which threshold applies?".into(),
            resolution_method: ResolutionMethod::Verify,
            status: QuestionStatus::Open,
            answer: None,
            links: Vec::new(),
            resolution_id: None,
        })
        .unwrap();
    store
        .claim_topic(
            &scope,
            &sid("topic_overtime"),
            "agent-one",
            Vec::<String>::new(),
        )
        .unwrap();
    store
        .claim_question(&scope, &sid("question_threshold"), "agent-two")
        .unwrap();

    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let fog: Option<String> = sqlx::query_scalar("SELECT fog FROM requirements WHERE id = ?")
        .bind("req_schads_overtime")
        .fetch_one(&pool)
        .await
        .unwrap();
    let topic: (Option<String>, Option<i64>) =
        sqlx::query_as("SELECT claimed_by, claimed_at FROM topics WHERE id = ?")
            .bind("topic_overtime")
            .fetch_one(&pool)
            .await
            .unwrap();
    let question: (String, Option<String>, Option<i64>) = sqlx::query_as(
        "SELECT resolution_method, claimed_by, claimed_at FROM questions WHERE id = ?",
    )
    .bind("question_threshold")
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        fog.as_deref(),
        Some("sleepover rules; something about broken shifts")
    );
    assert_eq!(topic.0.as_deref(), Some("agent-one"));
    assert!(topic.1.unwrap() > 0);
    assert_eq!(question.0, "verify");
    assert_eq!(question.1.as_deref(), Some("agent-two"));
    assert!(question.2.unwrap() > 0);
}

#[tokio::test]
async fn materialize_state_caches_enriched_source_and_resolution_fields() {
    let (_dir, layout, scope) = empty_layout();
    let store = StateStore::new(layout.clone());
    store
        .create_source(CreateSourceInput {
            scope_id: scope.clone(),
            id: sid("source_sah"),
            name: "Support at Home".into(),
            source_type: SourceType::Legislation,
            url: Some("https://example.test/sah".into()),
            reference: Some("Department guidance".into()),
            commit_pin: None,
            effective_date: Some(1_714_521_600_000),
            review_date: Some(1_717_200_000_000),
            superseded_by: Some(sid("source_sah_2025")),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    store
        .create_resolution(CreateResolutionInput {
            scope_id: scope,
            id: sid("res_sah"),
            title: "SAH extraction".into(),
            requirement_id: None,
            position: "Keep as draft extraction".into(),
            rationale: "Needs human review".into(),
            status: ResolutionStatus::Draft,
            context: Some("Codebase scan".into()),
            enforcement: Some("specification".into()),
            confidence: Some(0.91),
            inputs: vec![ResolutionInput {
                input_type: ResolutionInputType::Regulatory,
                reference: "SAH program manual".into(),
                summary: "Program rules reviewed".into(),
            }],
            made_by: Some("Analyst One".into()),
            approved_by: Some("Approver Two".into()),
            approved_at: Some(1_714_780_800_000),
            superseded_by: Some(sid("res_sah_2025")),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let source: (Option<String>, Option<i64>, Option<i64>, Option<String>) = sqlx::query_as(
        "SELECT reference, effective_date, review_date, superseded_by FROM sources WHERE id = ?",
    )
    .bind("source_sah")
    .fetch_one(&pool)
    .await
    .unwrap();
    let resolution: (String, Option<String>, Option<String>, Option<i64>, Option<String>) = sqlx::query_as("SELECT inputs, made_by, approved_by, approved_at, superseded_by FROM resolutions WHERE id = ?").bind("res_sah").fetch_one(&pool).await.unwrap();
    assert_eq!(source.0.as_deref(), Some("Department guidance"));
    assert_eq!(source.1, Some(1_714_521_600_000));
    assert_eq!(source.2, Some(1_717_200_000_000));
    assert_eq!(source.3.as_deref(), Some("source_sah_2025"));
    assert!(resolution.0.contains(r#""input_type":"regulatory""#));
    assert_eq!(resolution.1.as_deref(), Some("Analyst One"));
    assert_eq!(resolution.2.as_deref(), Some("Approver Two"));
    assert_eq!(resolution.3, Some(1_714_780_800_000));
    assert_eq!(resolution.4.as_deref(), Some("res_sah_2025"));
}

#[tokio::test]
async fn materialize_state_caches_commit_pin_and_confidence_scores() {
    let (_dir, layout, scope) = empty_layout();
    let sources_path = crate::shards::sources_path(&layout, &scope);
    std::fs::create_dir_all(sources_path.parent().unwrap()).unwrap();
    std::fs::write(&sources_path, r#"{"schema_version":1,"scope_id":"default","id":"source_codebase","name":"Example API","source_type":"project_artifact","commit_pin":"5e1f2a9c4b6d8e0f1234567890abcdef12345678"}
"#).unwrap();
    let contributions_path = crate::shards::contributions_path(&layout, &scope);
    std::fs::create_dir_all(contributions_path.parent().unwrap()).unwrap();
    std::fs::write(&contributions_path, r#"{"schema_version":1,"scope_id":"default","id":"contrib_reviewer_001","target":{"artifact_type":"requirement","artifact_id":"req_overtime"},"participant_slot":"reviewer","stance":"support","strongest_finding":"Supported by code evidence.","evidence_references":[],"material_claims":[{"claim_id":"claim_overtime_threshold","statement":"Overtime starts after the award threshold.","evidence_type":"artifact","evidence_reference_ids":[],"confidence":0.87}],"risks":[],"objections":[],"challenges":[],"suggested_artifact_changes":[],"unsupported_recommendations":[],"uncertainty":{"level":"low","rationale":"Direct code evidence."},"open_questions":[]}
"#).unwrap();
    let proposals_path = crate::shards::proposal_cards_path(&layout, &scope);
    std::fs::create_dir_all(proposals_path.parent().unwrap()).unwrap();
    std::fs::write(&proposals_path, r#"{"schema_version":1,"scope_id":"default","id":"proposal_overtime_traceability","proposal_key":"req-overtime-traceability","proposal_type":"requirement_candidate","title":"Clarify overtime traceability","summary":"Add source-backed threshold language.","confidence":0.83,"traceability":{"target":{"artifact_type":"requirement","artifact_id":"req_overtime"},"source_ids":["source_codebase"],"evidence_references":[],"supporting_claim_ids":["claim_overtime_threshold"]},"promotion_state":"proposed"}
"#).unwrap();
    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let commit_pin: Option<String> =
        sqlx::query_scalar("SELECT commit_pin FROM sources WHERE id = ?")
            .bind("source_codebase")
            .fetch_one(&pool)
            .await
            .unwrap();
    let confidence: Option<f64> =
        sqlx::query_scalar("SELECT confidence FROM proposal_cards WHERE id = ?")
            .bind("proposal_overtime_traceability")
            .fetch_one(&pool)
            .await
            .unwrap();
    let payload: String = sqlx::query_scalar("SELECT payload FROM contributions WHERE id = ?")
        .bind("contrib_reviewer_001")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        commit_pin.as_deref(),
        Some("5e1f2a9c4b6d8e0f1234567890abcdef12345678")
    );
    assert_eq!(confidence, Some(0.83));
    assert!(payload.contains(r#""confidence":0.87"#));
}

#[tokio::test]
async fn materialize_state_caches_proposal_lineage() {
    let (_dir, layout, scope) = empty_layout();
    let records = [
        (
            crate::shards::contributions_path(&layout, &scope),
            serde_json::json!({
                "schema_version": 1, "scope_id": "default", "id": "contribution_a",
                "target": {"artifact_type": "requirement", "artifact_id": "req_a"},
                "participant_slot": "extractor", "stance": "support", "strongest_finding": "Observed",
                "evidence_references": [{"reference_id": "evidence_a", "evidence_type": "source", "summary": "Pinned"}],
                "material_claims": [{"claim_id": "claim_a", "statement": "Observed", "evidence_type": "source", "evidence_reference_ids": ["evidence_a"]}],
                "risks": [], "objections": [], "challenges": [], "suggested_artifact_changes": [],
                "unsupported_recommendations": [], "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
            }),
        ),
        (
            crate::shards::synthesis_packets_path(&layout, &scope),
            serde_json::json!({
                "schema_version": 1, "scope_id": "default", "id": "synthesis_a",
                "target": {"artifact_type": "requirement", "artifact_id": "req_a"}, "summary": "Adjudicated",
                "consensus": [], "contested_claims": [], "minority_objections": [], "evidence_gaps": [],
                "unsupported_speculation": [], "open_questions": [],
                "suggested_artifacts": [{"proposal_id": "proposal_a", "proposal_key": "proposal-a", "proposal_type": "requirement_candidate", "summary": "Candidate", "origin_participant_slots": ["extractor"]}],
                "required_human_decisions": []
            }),
        ),
        (
            crate::shards::proposal_cards_path(&layout, &scope),
            serde_json::json!([
                {
                    "schema_version": 1, "scope_id": "default", "id": "proposal_a", "proposal_key": "proposal-a",
                    "proposal_type": "requirement_candidate", "title": "Candidate", "summary": "Candidate",
                    "traceability": {"target": {"artifact_type": "requirement", "artifact_id": "req_a"}, "source_ids": [], "evidence_references": [], "supporting_claim_ids": ["claim_a"]},
                    "promotion_state": "proposed"
                },
                {
                    "schema_version": 1, "scope_id": "default", "id": "proposal_b", "proposal_key": "proposal-b",
                    "proposal_type": "requirement_candidate", "title": "Descendant", "summary": "Descendant",
                    "traceability": {"target": {"artifact_type": "requirement", "artifact_id": "req_a"}, "source_ids": [], "evidence_references": [], "supporting_claim_ids": []},
                    "promotion_state": "proposed", "builds_on": ["assertion_a"]
                }
            ]),
        ),
        (
            crate::shards::assertion_records_path(&layout, &scope),
            serde_json::json!({
                "schema_version": 1, "scope_id": "default", "id": "assertion_a", "proposal_id": "proposal_a",
                "synthesis_packet_id": "synthesis_a", "supporting_claim_ids": ["claim_a"]
            }),
        ),
    ];
    for (path, value) in records {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let contents = match value {
            serde_json::Value::Array(values) => values
                .into_iter()
                .map(|value| serde_json::to_string(&value).unwrap())
                .collect::<Vec<_>>()
                .join("\n"),
            value => serde_json::to_string(&value).unwrap(),
        };
        std::fs::write(path, format!("{contents}\n")).unwrap();
    }

    materialize_state(&layout).await.unwrap();
    let pool = open_cache(&layout).await.unwrap();
    let builds_on: String = sqlx::query_scalar("SELECT builds_on FROM proposal_cards WHERE id = ?")
        .bind("proposal_b")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(builds_on, r#"["assertion_a"]"#);
}
