use super::{super::initialized_store, proposal_input};
use crate::state_store::CreateDispositionInput;
use provenance_core::SUPPORTED_SCHEMA_VERSION;
use provenance_core::{
    CanonicalArtifact, CanonicalArtifactType, DispositionActor, DispositionDecision,
    ExternalActionCorrelation, IdentityType, PromotionState, ScopeId, StableId,
};
use provenance_macros::verifies;

#[test]
#[verifies("rule_disposition_write_gate", examples)]
fn direct_disposition_rejects_missing_canonical_artifact_without_writing() {
    let (_dir, store, scope) = initialized_store();
    allow_actor(&store);
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_missing_artifact",
            "Missing artifact",
            PromotionState::Proposed,
        ))
        .unwrap();

    let error = store
        .create_disposition(input(&scope, "requirement", "req_missing"))
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("canonical artifact does not exist"),
        "{error}"
    );
    assert!(store.list_dispositions(&scope).unwrap().is_empty());
}

#[test]
fn direct_disposition_accepts_each_existing_canonical_artifact_kind() {
    for (artifact_type, record) in canonical_records("default") {
        let (_dir, store, scope) = initialized_store();
        allow_actor(&store);
        store
            .create_proposal_card(proposal_input(
                &scope,
                "proposal_artifact",
                "Artifact",
                PromotionState::Proposed,
            ))
            .unwrap();
        write_jsonl(
            &canonical_shard_path(&store.layout, &scope, artifact_type),
            record,
        );

        store
            .create_disposition(input(&scope, artifact_type, "artifact_test"))
            .unwrap();

        assert_eq!(store.list_dispositions(&scope).unwrap().len(), 1);
    }
}

#[test]
fn direct_disposition_rejects_each_missing_canonical_artifact_kind() {
    for artifact_type in canonical_artifact_types() {
        let (_dir, store, scope) = initialized_store();
        allow_actor(&store);
        store
            .create_proposal_card(proposal_input(
                &scope,
                "proposal_artifact",
                "Artifact",
                PromotionState::Proposed,
            ))
            .unwrap();

        let error = store
            .create_disposition(input(&scope, artifact_type, "artifact_test"))
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("canonical artifact does not exist"),
            "{artifact_type}: {error}"
        );
        assert!(store.list_dispositions(&scope).unwrap().is_empty());
    }
}

#[test]
fn direct_disposition_rejects_wrong_kind_and_wrong_scope_targets() {
    for (artifact_type, artifact_id) in [
        ("source", "artifact_collision"),
        ("requirement", "req_other"),
    ] {
        let (_dir, store, scope) = initialized_store();
        allow_actor(&store);
        store
            .create_proposal_card(proposal_input(
                &scope,
                "proposal_artifact",
                "Artifact",
                PromotionState::Proposed,
            ))
            .unwrap();
        let state = store.layout.state_dir();
        if artifact_id == "artifact_collision" {
            write_jsonl(
                &state.join("scopes/default/requirements/req.jsonl"),
                serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"artifact_collision","statement":"Collision","status":"active"}).to_string(),
            );
        } else {
            write_jsonl(
                &state.join("scopes/other/requirements/req.jsonl"),
                serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"other","id":"req_other","statement":"Other","status":"active"}).to_string(),
            );
        }

        let error = store
            .create_disposition(input(&scope, artifact_type, artifact_id))
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("canonical artifact does not exist"),
            "{error}"
        );
        assert!(store.list_dispositions(&scope).unwrap().is_empty());
    }
}

#[test]
fn direct_disposition_rejects_every_canonical_kind_misfiled_in_the_scope_shard() {
    for (artifact_type, record) in misfiled_canonical_records() {
        let (_dir, store, scope) = initialized_store();
        allow_actor(&store);
        store
            .create_proposal_card(proposal_input(
                &scope,
                "proposal_artifact",
                "Artifact",
                PromotionState::Proposed,
            ))
            .unwrap();
        write_jsonl(
            &canonical_shard_path(&store.layout, &scope, artifact_type),
            record,
        );
        assert_misfiled_record_is_in_canonical_shard(&store, &scope, artifact_type);

        let error = store
            .create_disposition(input(&scope, artifact_type, "artifact_misfiled"))
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("canonical artifact does not exist"),
            "{artifact_type}: {error}"
        );
        assert!(store.list_dispositions(&scope).unwrap().is_empty());
    }
}

#[test]
fn scope_validation_rejects_a_persisted_missing_canonical_artifact() {
    let (_dir, store, scope) = initialized_store();
    allow_actor(&store);
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_artifact",
            "Artifact",
            PromotionState::Proposed,
        ))
        .unwrap();
    write_jsonl(
        &crate::shards::dispositions_path(&store.layout, &scope),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"disposition_artifact","proposal_id":"proposal_artifact","decision":"rejected","rationale":"Reviewed","actor":{"identity_type":"human","id":"reviewer"},"canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_missing"}}).to_string(),
    );

    let error = store
        .validate_ideation_scope(&scope)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("canonical artifact does not exist"),
        "{error}"
    );
}

#[test]
fn scope_validation_rejects_a_misfiled_canonical_artifact() {
    let (_dir, store, scope) = initialized_store();
    allow_actor(&store);
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_artifact",
            "Artifact",
            PromotionState::Proposed,
        ))
        .unwrap();
    write_jsonl(
        &store
            .layout
            .state_dir()
            .join("scopes/default/requirements/req.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"other","id":"artifact_misfiled","statement":"Misfiled","status":"active"}).to_string(),
    );
    write_jsonl(
        &crate::shards::dispositions_path(&store.layout, &scope),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"disposition_artifact","proposal_id":"proposal_artifact","decision":"rejected","rationale":"Reviewed","actor":{"identity_type":"human","id":"reviewer"},"canonical_artifact":{"artifact_type":"requirement","artifact_id":"artifact_misfiled"}}).to_string(),
    );

    let error = store
        .validate_ideation_scope(&scope)
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("canonical artifact does not exist"),
        "{error}"
    );
}

#[test]
fn batch_rejects_a_misfiled_canonical_artifact_without_landing() {
    let (_dir, store, scope) = initialized_store();
    allow_actor(&store);
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_artifact",
            "Artifact",
            PromotionState::Proposed,
        ))
        .unwrap();
    write_jsonl(
        &store
            .layout
            .state_dir()
            .join("scopes/default/requirements/req.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"other","id":"artifact_misfiled","statement":"Misfiled","status":"active"}).to_string(),
    );
    let batch = serde_json::from_value(serde_json::json!({
        "dispositions": [{
            "schema_version": SUPPORTED_SCHEMA_VERSION.0, "scope_id": "default", "id": "disposition_artifact",
            "proposal_id": "proposal_artifact", "decision": "rejected", "rationale": "Reviewed",
            "actor": {"identity_type": "human", "id": "reviewer"},
            "canonical_artifact": {"artifact_type": "requirement", "artifact_id": "artifact_misfiled"}
        }]
    }))
    .unwrap();

    let error = store
        .land_ideation_batch(&scope, batch, false)
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("canonical artifact does not exist"),
        "{error}"
    );
    assert!(store.list_dispositions(&scope).unwrap().is_empty());
}

#[test]
#[verifies("rule_disposition_write_gate", examples)]
fn duplicate_disposition_cannot_mutate_frozen_external_action() {
    let (_dir, store, scope) = initialized_store();
    allow_actor(&store);
    write_jsonl(
        &store
            .layout
            .state_dir()
            .join("scopes/default/requirements/req.jsonl"),
        serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":"default","id":"req_existing","statement":"Existing","status":"active"}).to_string(),
    );
    store
        .create_proposal_card(proposal_input(
            &scope,
            "proposal_artifact",
            "Artifact",
            PromotionState::Proposed,
        ))
        .unwrap();
    let mut original = input(&scope, "requirement", "req_existing");
    original.external_action = Some(ExternalActionCorrelation {
        system: "github".into(),
        scope: "acme/payroll".into(),
        kind: "issue".into(),
        key: "44".into(),
    });
    store.create_disposition(original).unwrap();
    let mut replacement = input(&scope, "requirement", "req_existing");
    replacement.external_action = Some(ExternalActionCorrelation {
        system: "linear".into(),
        scope: "payroll".into(),
        kind: "ticket".into(),
        key: "PAY-44".into(),
    });

    let error = store
        .create_disposition(replacement)
        .unwrap_err()
        .to_string();

    assert!(error.contains("authoritative disposition"), "{error}");
    assert_eq!(
        store.list_dispositions(&scope).unwrap()[0]
            .external_action
            .as_ref()
            .unwrap()
            .system,
        "github"
    );
}

fn input(scope: &ScopeId, artifact_type: &str, artifact_id: &str) -> CreateDispositionInput {
    CreateDispositionInput {
        scope_id: scope.clone(),
        id: StableId::new("disposition_artifact").unwrap(),
        proposal_id: StableId::new(if artifact_id == "req_missing" {
            "proposal_missing_artifact"
        } else {
            "proposal_artifact"
        })
        .unwrap(),
        decision: DispositionDecision::Rejected,
        rationale: Some("Reviewed".into()),
        actor: DispositionActor {
            identity_type: IdentityType::Human,
            id: "reviewer".into(),
            name: None,
        },
        canonical_artifact: Some(CanonicalArtifact {
            artifact_type: CanonicalArtifactType::parse(artifact_type).unwrap(),
            artifact_id: StableId::new(artifact_id).unwrap(),
        }),
        external_action: None,
    }
}

fn allow_actor(store: &crate::state_store::StateStore) {
    let mut manifest = store.manifest().unwrap();
    manifest.disposition_actor_ids.push("reviewer".into());
    std::fs::write(
        store.layout.manifest_path(),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}

fn write_jsonl(path: &camino::Utf8Path, record: impl AsRef<str>) {
    let record = record.as_ref();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("{record}\n")).unwrap();
}

fn assert_misfiled_record_is_in_canonical_shard(
    store: &crate::state_store::StateStore,
    scope: &ScopeId,
    artifact_type: &str,
) {
    let loaded = match artifact_type {
        "source" => serde_json::to_value(store.list_sources(scope).unwrap()),
        "requirement" => serde_json::to_value(store.list_requirements(scope).unwrap()),
        "resolution" => serde_json::to_value(store.list_resolutions(scope).unwrap()),
        "rule" => serde_json::to_value(store.list_rules(scope).unwrap()),
        "domain" => serde_json::to_value(store.list_domains(scope).unwrap()),
        "boundary" => serde_json::to_value(store.list_boundaries(scope).unwrap()),
        "topic" => serde_json::to_value(store.list_topics(scope).unwrap()),
        "question" => serde_json::to_value(store.list_questions(scope).unwrap()),
        _ => unreachable!("all canonical artifact kinds are covered"),
    }
    .unwrap();
    let records = loaded.as_array().unwrap();
    assert_eq!(records.len(), 1, "{artifact_type}");
    assert_eq!(records[0]["id"], "artifact_misfiled", "{artifact_type}");
    assert_eq!(records[0]["scope_id"], "other", "{artifact_type}");
}

fn canonical_shard_path(
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

fn canonical_records(scope: &str) -> [(&'static str, String); 8] {
    [
        (
            "source",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":scope,"id":"artifact_test","name":"Test","source_type":"document","url":null}).to_string(),
        ),
        (
            "requirement",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":scope,"id":"artifact_test","statement":"Test","status":"active"}).to_string(),
        ),
        (
            "resolution",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":scope,"id":"artifact_test","title":"Test","position":"Test","rationale":"Test","status":"approved","inputs":[],"review_on":null}).to_string(),
        ),
        (
            "rule",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":scope,"id":"artifact_test","statement":"Test","status":"draft","severity":"high"}).to_string(),
        ),
        (
            "domain",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":scope,"id":"artifact_test","name":"Test"}).to_string(),
        ),
        (
            "boundary",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":scope,"id":"artifact_test","requirement_id":"requirement_test","statement":"Test"}).to_string(),
        ),
        (
            "topic",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":scope,"id":"artifact_test","requirement_id":"requirement_test","title":"Test","status":"open","links":[]}).to_string(),
        ),
        (
            "question",
            serde_json::json!({"schema_version": SUPPORTED_SCHEMA_VERSION.0,"scope_id":scope,"id":"artifact_test","topic_id":"topic_test","requirement_id":"requirement_test","question":"Test?","resolution_method":"research","status":"open","links":[]}).to_string(),
        ),
    ]
}

fn misfiled_canonical_records() -> [(&'static str, String); 8] {
    canonical_records("other")
        .map(|(kind, record)| (kind, record.replace("artifact_test", "artifact_misfiled")))
}
