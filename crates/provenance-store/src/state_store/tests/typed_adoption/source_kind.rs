//! Adoption of an unowned Source that does not have the document kind.

use super::*;

use provenance_core::authoring::{
    requirement as declare_requirement, source as declare_source, spec as declare_spec,
};

const BRIEF_ID: &str = "source_workflowd_integration_brief";
const BRIEF_KEY: &str = "workflowd-integration-brief";
const BRIEF_NAME: &str =
    "workflowd integration brief (agent-authored, relayed by Ben Nasraoui 2026-08-19)";
const BRIEF_REFERENCE: &str = "session:824f8174 workflowd-agent brief";
const REQUIREMENT_ID: &str = "req_env_key_at_invocation";
const REQUIREMENT_KEY: &str = "env-key-at-invocation";
const REQUIREMENT_STATEMENT: &str = "The provider reads the environment key at invocation";

fn create_unowned_brief(store: &StateStore, scope: &ScopeId) {
    store
        .create_source(CreateSourceInput {
            scope_id: scope.clone(),
            id: StableId::new(BRIEF_ID).unwrap(),
            name: BRIEF_NAME.to_string(),
            source_type: SourceType::ExternalIntegration,
            url: None,
            reference: Some(BRIEF_REFERENCE.to_string()),
            commit_pin: None,
            effective_date: None,
            review_date: None,
            supersedes: Vec::new(),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap();
}

fn fluent_input() -> TypedSpecInput {
    let brief = declare_source(BRIEF_KEY)
        .name(BRIEF_NAME)
        .adopt_unowned(BRIEF_ID)
        .kind(SourceType::ExternalIntegration);
    declare_spec("noscope")
        .requirements([declare_requirement(REQUIREMENT_KEY)
            .adopt_unowned(REQUIREMENT_ID)
            .statement(REQUIREMENT_STATEMENT)
            .from(brief)])
        .build()
        .unwrap()
        .materialize(OWNER)
}

#[test]
fn fluent_adoption_of_an_external_integration_source_keeps_its_kind() {
    let (_dir, store, scope) = initialized_store();
    create_unowned_brief(&store, &scope);
    create_unowned_requirement(&store, &scope, REQUIREMENT_ID, REQUIREMENT_STATEMENT);
    store
        .add_source_reference(AddSourceReferenceInput {
            scope_id: scope.clone(),
            source_id: StableId::new(BRIEF_ID).unwrap(),
            requirement_id: StableId::new(REQUIREMENT_ID).unwrap(),
            clause: None,
        })
        .unwrap();
    let input = fluent_input();

    // `kind` adds no optional URL or reference metadata.
    assert_eq!(input.sources[0].kind, "external_integration");
    assert_eq!(input.sources[0].reference, None);
    assert_eq!(input.sources[0].url, None);

    let edge_before = store.list_edges().unwrap()[0].clone();

    // Adoption moves both records into declaration ownership and
    // changes nothing else.
    let plan = store.plan_typed_spec(&scope, input.clone()).unwrap();
    assert_eq!(
        (
            plan.created,
            plan.updated,
            plan.retired,
            plan.conflicts,
            plan.moved
        ),
        (0, 0, 0, 0, 2)
    );

    store.apply_typed_spec(&scope, input.clone()).unwrap();

    let sources = store.list_sources(&scope).unwrap();
    assert_eq!(sources.len(), 1);
    let brief = &sources[0];
    assert_eq!(brief.id.as_str(), BRIEF_ID);
    assert_eq!(brief.source_type, SourceType::ExternalIntegration);
    assert_eq!(brief.name, BRIEF_NAME);
    assert_eq!(brief.reference.as_deref(), Some(BRIEF_REFERENCE));
    assert_eq!(brief.url, None);
    assert!(!brief.retired);
    assert_eq!(brief.declared_by.as_deref(), Some(OWNER));
    assert_eq!(
        brief.declaration_address.as_ref().unwrap().segments(),
        ["noscope", "source", BRIEF_KEY]
    );

    let requirements = store.list_requirements(&scope).unwrap();
    assert_eq!(requirements.len(), 1);
    assert_eq!(requirements[0].id.as_str(), REQUIREMENT_ID);
    assert_eq!(requirements[0].statement, REQUIREMENT_STATEMENT);
    assert_eq!(requirements[0].declared_by.as_deref(), Some(OWNER));

    // The citation edge survives adoption unchanged, identity included.
    let edges = store.list_edges().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0], edge_before);
    assert_eq!(edges[0].from_id.as_str(), BRIEF_ID);
    assert_eq!(edges[0].to_id.as_str(), REQUIREMENT_ID);

    let replay = store.plan_typed_spec(&scope, input).unwrap();
    assert_eq!(
        (
            replay.created,
            replay.updated,
            replay.moved,
            replay.retired,
            replay.conflicts
        ),
        (0, 0, 0, 0, 0)
    );
    assert_eq!(replay.unchanged, 2);
    assert!(replay
        .resources
        .iter()
        .all(|resource| resource.state == ReconcileState::Unchanged));
}
