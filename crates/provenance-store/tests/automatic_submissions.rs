use provenance_core::{ScopeId, StableId};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{
        AddSourceReferenceInput, CreateRequirementInput, CreateResolutionInput, CreateSourceInput,
        StateStore, UpdateRequirementInput,
    },
};
use serde_json::json;

fn fixture() -> (tempfile::TempDir, StateStore, ScopeId) {
    let temp = tempfile::tempdir().unwrap();
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.state_dir()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        r#"{"schema_version":2,"scopes":[{"id":"default","path_prefix":"."}]}"#,
    )
    .unwrap();
    let scope = ScopeId::new("default").unwrap();
    (temp, StateStore::new(layout), scope)
}

fn requirement(scope: &ScopeId, id: &str) -> CreateRequirementInput {
    serde_json::from_value(json!({
        "scope_id":scope, "id":id,
        "statement":format!("The system stores {id}."),
        "status":"discovery", "depends_on":[], "supersedes":[]
    }))
    .unwrap()
}

fn pending(store: &StateStore, scope: &ScopeId, id: &StableId) -> StableId {
    store
        .requirement_decision_state(scope, id)
        .unwrap()
        .pending
        .unwrap()
        .proposal_id
}

fn assert_new_pending(
    store: &StateStore,
    scope: &ScopeId,
    id: &StableId,
    previous: &StableId,
) -> StableId {
    let current = pending(store, scope, id);
    assert_ne!(&current, previous);
    current
}

#[test]
fn direct_create_and_all_named_content_writers_open_submissions() {
    let (_temp, store, scope) = fixture();
    let id = StableId::new("req_a").unwrap();
    store
        .create_requirement(requirement(&scope, "req_a"))
        .unwrap();
    let mut proposal = pending(&store, &scope, &id);

    let update: UpdateRequirementInput = serde_json::from_value(json!({
        "scope_id":"default", "id":"req_a", "description":"Updated"
    }))
    .unwrap();
    store.update_requirement(update).unwrap();
    proposal = assert_new_pending(&store, &scope, &id, &proposal);

    store
        .set_requirement_fog(&scope, &id, Some("Unknown condition.".to_owned()))
        .unwrap();
    proposal = assert_new_pending(&store, &scope, &id, &proposal);

    store
        .create_source(
            serde_json::from_value::<CreateSourceInput>(json!({
                "scope_id":"default", "id":"source_a", "name":"Policy",
                "source_type":"policy", "reference":"section 1", "supersedes":[]
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .add_source_reference(AddSourceReferenceInput {
            scope_id: scope.clone(),
            source_id: StableId::new("source_a").unwrap(),
            requirement_id: id.clone(),
            clause: None,
        })
        .unwrap();
    proposal = assert_new_pending(&store, &scope, &id, &proposal);

    for target in ["req_b", "req_c", "req_d"] {
        store
            .create_requirement(requirement(&scope, target))
            .unwrap();
    }
    store
        .set_requirement_refines(&scope, &id, StableId::new("req_b").unwrap())
        .unwrap();
    proposal = assert_new_pending(&store, &scope, &id, &proposal);
    store
        .add_requirement_depends_on(&scope, &id, StableId::new("req_c").unwrap())
        .unwrap();
    proposal = assert_new_pending(&store, &scope, &id, &proposal);
    store
        .add_requirement_supersedes(&scope, &id, StableId::new("req_d").unwrap())
        .unwrap();
    proposal = assert_new_pending(&store, &scope, &id, &proposal);

    store
        .create_resolution(
            serde_json::from_value::<CreateResolutionInput>(json!({
                "scope_id":"default", "id":"resolution_a", "title":"Origin",
                "position":"Store records.", "rationale":"Records are required.",
                "requirement_ids":["req_a"], "supersedes":[], "status":"proposed",
                "inputs":[]
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .set_requirement_spawned_by(&scope, &id, StableId::new("resolution_a").unwrap())
        .unwrap();
    assert_new_pending(&store, &scope, &id, &proposal);
}
