use super::initialized_store;
use crate::cache::ProjectionFamily;
use crate::layout::ProvenanceLayout;
use crate::publication::with_repository_publication;
use crate::shards;
use crate::state_store::{CreateBoundaryInput, CreateRequirementInput};
use crate::state_store::{ScopeShards, StateStore};
use provenance_core::{RequirementStatus, StableId};
use provenance_macros::verifies;

fn staged_store() -> (
    tempfile::TempDir,
    StateStore,
    StateStore,
    provenance_core::ScopeId,
    Vec<u8>,
) {
    let (dir, live_store, scope) = initialized_store();
    let manifest_before = std::fs::read(live_store.layout.manifest_path()).unwrap();
    let staged_root = camino::Utf8PathBuf::from_path_buf(dir.path().join("staged-repo")).unwrap();
    let staged_layout = ProvenanceLayout::new(staged_root);
    std::fs::create_dir_all(staged_layout.state_dir()).unwrap();
    std::fs::write(staged_layout.manifest_path(), &manifest_before).unwrap();
    let staged_store = StateStore::new(staged_layout);
    (dir, live_store, staged_store, scope, manifest_before)
}

#[test]
fn scope_import_writes_only_the_imported_shards() {
    let (_dir, live_store, staged_store, scope, manifest_before) = staged_store();

    with_repository_publication(&live_store.layout, || {
        staged_store.import_scope(&scope, &ScopeShards::default())
    })
    .unwrap();

    for family in ProjectionFamily::ALL {
        let path = family.shard_path(&staged_store.layout, &scope);
        if matches!(
            family,
            ProjectionFamily::RequirementReviews | ProjectionFamily::ReviewJournal
        ) {
            assert!(!path.exists(), "{path} must stay outside scope import");
        } else {
            assert!(std::fs::read(path).unwrap().is_empty());
        }
    }
    assert_eq!(
        std::fs::read(staged_store.layout.manifest_path()).unwrap(),
        manifest_before
    );
    assert!(!shards::ideation_landings_path(&staged_store.layout, &scope).exists());
}

#[test]
fn scope_import_does_not_create_a_staged_repository_lock() {
    let (_dir, live_store, staged_store, scope, _manifest_before) = staged_store();

    with_repository_publication(&live_store.layout, || {
        staged_store.import_scope(&scope, &ScopeShards::default())
    })
    .unwrap();

    assert!(!staged_store.layout.publication_lock_path().exists());
}

#[test]
#[verifies("rule_porcelain_id_unique_in_repository", examples)]
fn scope_import_rejects_an_id_shared_by_two_canonical_kinds() {
    let (_dir, live_store, staged_store, scope, _manifest_before) = staged_store();
    let requirement = live_store
        .create_requirement(CreateRequirementInput {
            scope_id: scope.clone(),
            id: StableId::new("shared_import_id").unwrap(),
            statement: "The imported record has one identity.".to_owned(),
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
    let mut boundary = live_store
        .create_boundary(CreateBoundaryInput {
            scope_id: scope.clone(),
            id: StableId::new("boundary_before_import").unwrap(),
            requirement_id: requirement.id.clone(),
            statement: "The import does not reuse canonical IDs.".to_owned(),
            source_ref: None,
        })
        .unwrap();
    boundary.id = requirement.id.clone();
    let requirements = [requirement];
    let boundaries = [boundary];

    let error = with_repository_publication(&live_store.layout, || {
        staged_store.import_scope(
            &scope,
            &ScopeShards {
                requirements: &requirements,
                boundaries: &boundaries,
                ..ScopeShards::default()
            },
        )
    })
    .unwrap_err();

    assert!(error.to_string().contains("record ID already exists"));
}

#[test]
fn scope_import_does_not_grandfather_a_keyword_id_from_another_canonical_kind() {
    let (_dir, live, staged, scope, _) = staged_store();
    let old: provenance_core::Source = serde_json::from_value(serde_json::json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default", "id": "search", "name": "Old source",
        "source_type": "document"
    }))
    .unwrap();
    let old_path = shards::sources_path(&staged.layout, &scope);
    crate::jsonl::write_jsonl_atomic(&old_path, std::slice::from_ref(&old)).unwrap();
    let replacement: provenance_core::Requirement = serde_json::from_value(serde_json::json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default", "id": "search", "statement": "A new requirement exists.",
        "status": "active"
    }))
    .unwrap();

    let error = with_repository_publication(&live.layout, || {
        staged.import_scope(
            &scope,
            &ScopeShards {
                requirements: std::slice::from_ref(&replacement),
                ..ScopeShards::default()
            },
        )
    })
    .unwrap_err();
    assert!(error.to_string().contains("reserved record ID search"), "{error}");
    assert_eq!(staged.list_sources(&scope).unwrap(), vec![old]);
    assert!(staged.list_requirements(&scope).unwrap().is_empty());
    assert!(!staged.layout.publication_lock_path().exists());
}

#[test]
fn scope_import_preserves_a_keyword_id_from_a_landing_only_record() {
    let (_dir, live, staged, scope, _) = staged_store();
    let old: provenance_core::Contribution = serde_json::from_value(serde_json::json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default", "id": "search",
        "target": {"artifact_type": "source", "artifact_id": "source_anchor"},
        "participant_slot": "reviewer", "stance": "support", "strongest_finding": "Evidence",
        "evidence_references": [], "material_claims": [], "risks": [], "objections": [],
        "challenges": [], "suggested_artifact_changes": [],
        "unsupported_recommendations": [],
        "uncertainty": {"level": "low", "rationale": "Direct"}, "open_questions": []
    }))
    .unwrap();
    let landing = crate::state_store::IdeationLandingBatch {
        contributions: vec![old.clone()],
        synthesis_packets: Vec::new(),
        proposals: Vec::new(),
        assertions: Vec::new(),
        dispositions: Vec::new(),
    };
    let landing_path = shards::ideation_landings_path(&staged.layout, &scope);
    crate::jsonl::write_jsonl_atomic(&landing_path, &[landing]).unwrap();

    with_repository_publication(&live.layout, || {
        staged.import_scope(
            &scope,
            &ScopeShards {
                contributions: std::slice::from_ref(&old),
                ..ScopeShards::default()
            },
        )
    })
    .unwrap();
    assert_eq!(staged.list_contributions(&scope).unwrap(), vec![old]);
    assert!(!staged.layout.publication_lock_path().exists());
}

#[test]
fn scope_import_preserves_a_keyword_id_from_a_legacy_disposition() {
    let (_dir, live, staged, scope, _) = staged_store();
    let legacy_path = shards::legacy_promotion_decisions_path(&staged.layout, &scope);
    std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
    let old: provenance_core::DispositionRecord = serde_json::from_value(serde_json::json!({
        "schema_version": provenance_core::SUPPORTED_SCHEMA_VERSION.0,
        "scope_id": "default", "id": "search", "proposal_id": "proposal_old",
        "decision": "rejected", "rationale": "Old decision",
        "actor": {"identity_type": "human", "id": "reviewer"}
    }))
    .unwrap();
    std::fs::write(&legacy_path, format!("{}\n", serde_json::to_string(&old).unwrap())).unwrap();

    with_repository_publication(&live.layout, || {
        staged.import_scope(
            &scope,
            &ScopeShards {
                dispositions: std::slice::from_ref(&old),
                ..ScopeShards::default()
            },
        )
    })
    .unwrap();
    assert_eq!(staged.list_dispositions(&scope).unwrap(), vec![old]);
    assert!(!staged.layout.publication_lock_path().exists());
}
