use super::{super::initialized_store, proposal_input};
use crate::state_store::CreateDispositionInput;
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
        .create_proposal_card(
            None,
            proposal_input(
                &scope,
                "proposal_missing_artifact",
                "Missing artifact",
                PromotionState::Proposed,
            ),
        )
        .unwrap();

    let error = store
        .create_disposition(None, input(&scope, "requirement", "req_missing"))
        .unwrap_err()
        .to_string();

    assert!(
        error.contains("canonical artifact does not exist"),
        "{error}"
    );
    assert!(store.list_dispositions(&scope).unwrap().is_empty());
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
            .create_proposal_card(
                None,
                proposal_input(
                    &scope,
                    "proposal_artifact",
                    "Artifact",
                    PromotionState::Proposed,
                ),
            )
            .unwrap();
        let state = store.layout.state_dir();
        if artifact_id == "artifact_collision" {
            write_jsonl(
                &state.join("scopes/default/requirements/req.jsonl"),
                r#"{"schema_version":1,"scope_id":"default","id":"artifact_collision","statement":"Collision","status":"active"}"#,
            );
        } else {
            write_jsonl(
                &state.join("scopes/other/requirements/req.jsonl"),
                r#"{"schema_version":1,"scope_id":"other","id":"req_other","statement":"Other","status":"active"}"#,
            );
        }

        let error = store
            .create_disposition(None, input(&scope, artifact_type, artifact_id))
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
            .create_proposal_card(
                None,
                proposal_input(
                    &scope,
                    "proposal_artifact",
                    "Artifact",
                    PromotionState::Proposed,
                ),
            )
            .unwrap();
        write_jsonl(
            &canonical_shard_path(&store.layout, &scope, artifact_type),
            record,
        );
        assert_misfiled_record_is_in_canonical_shard(&store, &scope, artifact_type);

        let error = store
            .create_disposition(None, input(&scope, artifact_type, "artifact_misfiled"))
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
        .create_proposal_card(
            None,
            proposal_input(
                &scope,
                "proposal_artifact",
                "Artifact",
                PromotionState::Proposed,
            ),
        )
        .unwrap();
    write_jsonl(
        &crate::shards::dispositions_path(&store.layout, &scope),
        r#"{"schema_version":1,"scope_id":"default","id":"disposition_artifact","proposal_id":"proposal_artifact","decision":"rejected","rationale":"Reviewed","actor":{"identity_type":"human","id":"reviewer"},"canonical_artifact":{"artifact_type":"requirement","artifact_id":"req_missing"}}"#,
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
        .create_proposal_card(
            None,
            proposal_input(
                &scope,
                "proposal_artifact",
                "Artifact",
                PromotionState::Proposed,
            ),
        )
        .unwrap();
    write_jsonl(
        &store
            .layout
            .state_dir()
            .join("scopes/default/requirements/req.jsonl"),
        r#"{"schema_version":1,"scope_id":"other","id":"artifact_misfiled","statement":"Misfiled","status":"active"}"#,
    );
    write_jsonl(
        &crate::shards::dispositions_path(&store.layout, &scope),
        r#"{"schema_version":1,"scope_id":"default","id":"disposition_artifact","proposal_id":"proposal_artifact","decision":"rejected","rationale":"Reviewed","actor":{"identity_type":"human","id":"reviewer"},"canonical_artifact":{"artifact_type":"requirement","artifact_id":"artifact_misfiled"}}"#,
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
        .create_proposal_card(
            None,
            proposal_input(
                &scope,
                "proposal_artifact",
                "Artifact",
                PromotionState::Proposed,
            ),
        )
        .unwrap();
    write_jsonl(
        &store
            .layout
            .state_dir()
            .join("scopes/default/requirements/req.jsonl"),
        r#"{"schema_version":1,"scope_id":"other","id":"artifact_misfiled","statement":"Misfiled","status":"active"}"#,
    );
    let batch = serde_json::from_value(serde_json::json!({
        "dispositions": [{
            "schema_version": 1, "scope_id": "default", "id": "disposition_artifact",
            "proposal_id": "proposal_artifact", "decision": "rejected", "rationale": "Reviewed",
            "actor": {"identity_type": "human", "id": "reviewer"},
            "canonical_artifact": {"artifact_type": "requirement", "artifact_id": "artifact_misfiled"}
        }]
    }))
    .unwrap();

    let error = store
        .land_ideation_batch(None, &scope, batch, false)
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
        r#"{"schema_version":1,"scope_id":"default","id":"req_existing","statement":"Existing","status":"active"}"#,
    );
    store
        .create_proposal_card(
            None,
            proposal_input(
                &scope,
                "proposal_artifact",
                "Artifact",
                PromotionState::Proposed,
            ),
        )
        .unwrap();
    let mut original = input(&scope, "requirement", "req_existing");
    original.external_action = Some(ExternalActionCorrelation {
        system: "github".into(),
        scope: "acme/payroll".into(),
        kind: "issue".into(),
        key: "44".into(),
    });
    store.create_disposition(None, original).unwrap();
    let mut replacement = input(&scope, "requirement", "req_existing");
    replacement.external_action = Some(ExternalActionCorrelation {
        system: "linear".into(),
        scope: "payroll".into(),
        kind: "ticket".into(),
        key: "PAY-44".into(),
    });

    let error = store
        .create_disposition(None, replacement)
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
        rationale: "Reviewed".into(),
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

fn write_jsonl(path: &camino::Utf8Path, record: &str) {
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
        _ => unreachable!("all canonical artifact kinds are covered"),
    }
}

fn misfiled_canonical_records() -> [(&'static str, &'static str); 4] {
    [
        (
            "source",
            r#"{"schema_version":1,"scope_id":"other","id":"artifact_misfiled","name":"Misfiled","source_type":"document","url":null}"#,
        ),
        (
            "requirement",
            r#"{"schema_version":1,"scope_id":"other","id":"artifact_misfiled","statement":"Misfiled","status":"active"}"#,
        ),
        (
            "resolution",
            r#"{"schema_version":1,"scope_id":"other","id":"artifact_misfiled","title":"Misfiled","position":"No","rationale":"No","status":"approved","inputs":[],"review_on":null}"#,
        ),
        (
            "rule",
            r#"{"schema_version":1,"scope_id":"other","id":"artifact_misfiled","statement":"Misfiled","status":"draft","severity":"high"}"#,
        ),
    ]
}
