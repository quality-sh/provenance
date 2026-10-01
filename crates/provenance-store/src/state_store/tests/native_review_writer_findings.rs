use super::{
    native_review_writers::{id, seed_native_records},
    seeded_requirement_store,
};
use crate::state_store::{
    EditQuestionInput, UpdateBoundaryInput, UpdateDomainInput, UpdateRequirementInput,
    UpdateResolutionInput, UpdateRuleInput, UpdateSourceInput, UpdateTopicInput,
};
use provenance_core::review::{SaveOutcome, REVIEW_SCHEMA_VERSION};
use provenance_core::{NodeType, SchemaVersion, Source};

fn entries(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    kind: NodeType,
) -> Vec<provenance_core::review::ReviewEntry> {
    let mut entries = store
        .review_entries(scope)
        .unwrap()
        .into_iter()
        .filter(|entry| entry.record_kind == kind)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.sequence);
    entries
}

fn assert_lifecycle_then_content(entries: &[provenance_core::review::ReviewEntry]) {
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[1].outcome, SaveOutcome::LifecycleOnly);
    assert_eq!(entries[1].revision, entries[0].revision);
    assert_eq!(entries[2].outcome, SaveOutcome::Changed);
    assert_ne!(entries[2].revision, entries[1].revision);
}

#[test]
fn source_review_date_keeps_revision_and_name_changes_it() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    store
        .update_source(
            serde_json::from_value::<UpdateSourceInput>(serde_json::json!({
                "scope_id":"default", "id":"source_native", "review_date":1
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .update_source(
            serde_json::from_value::<UpdateSourceInput>(serde_json::json!({
                "scope_id":"default", "id":"source_native", "name":"Source B"
            }))
            .unwrap(),
        )
        .unwrap();
    assert_lifecycle_then_content(&entries(&store, &scope, NodeType::Source));
}

#[test]
fn resolution_review_on_keeps_revision_and_position_changes_it() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    store
        .update_resolution(
            serde_json::from_value::<UpdateResolutionInput>(serde_json::json!({
                "scope_id":"default", "id":"resolution_native", "review_on":"2027-01-01"
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .update_resolution(
            serde_json::from_value::<UpdateResolutionInput>(serde_json::json!({
                "scope_id":"default", "id":"resolution_native", "position":"Use position B"
            }))
            .unwrap(),
        )
        .unwrap();
    assert_lifecycle_then_content(&entries(&store, &scope, NodeType::Resolution));
}

#[test]
fn rule_archive_stamp_keeps_revision_after_statement_changes_it() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    store
        .update_rule(
            serde_json::from_value::<UpdateRuleInput>(serde_json::json!({
                "scope_id":"default", "id":"rule_native", "statement":"The system uses position B"
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .update_rule(
            serde_json::from_value::<UpdateRuleInput>(serde_json::json!({
                "scope_id":"default", "id":"rule_native", "status":"archived",
                "archived_in_commit":{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
            }))
            .unwrap(),
        )
        .unwrap();
    let entries = entries(&store, &scope, NodeType::Rule);
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[1].outcome, SaveOutcome::Changed);
    assert_ne!(entries[1].revision, entries[0].revision);
    assert_eq!(entries[2].outcome, SaveOutcome::LifecycleOnly);
    assert_eq!(entries[2].revision, entries[1].revision);
}

#[test]
fn shaping_claims_keep_revisions_and_text_changes_them() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    store
        .claim_topic(&scope, &id("topic_native"), "author")
        .unwrap();
    store
        .edit_topic(
            serde_json::from_value::<UpdateTopicInput>(serde_json::json!({
                "scope_id":"default", "id":"topic_native", "title":"Topic B"
            }))
            .unwrap(),
        )
        .unwrap();
    assert_lifecycle_then_content(&entries(&store, &scope, NodeType::Topic));

    store
        .claim_question(&scope, &id("question_native"), "author")
        .unwrap();
    store
        .edit_question(
            serde_json::from_value::<EditQuestionInput>(serde_json::json!({
                "scope_id":"default", "id":"question_native", "question":"Is B correct?"
            }))
            .unwrap(),
        )
        .unwrap();
    assert_lifecycle_then_content(&entries(&store, &scope, NodeType::Question));
}

#[test]
fn domain_color_and_requirement_status_keep_revisions() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    store
        .update_domain(
            serde_json::from_value::<UpdateDomainInput>(serde_json::json!({
                "scope_id":"default", "id":"domain_native", "color":"blue"
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .update_domain(
            serde_json::from_value::<UpdateDomainInput>(serde_json::json!({
                "scope_id":"default", "id":"domain_native", "name":"Domain B"
            }))
            .unwrap(),
        )
        .unwrap();
    assert_lifecycle_then_content(&entries(&store, &scope, NodeType::Domain));

    store
        .update_requirement(
            serde_json::from_value::<UpdateRequirementInput>(serde_json::json!({
                    "scope_id":"default", "id":"req_overtime", "status":"refinement"
            }))
            .unwrap(),
        )
        .unwrap();
    store
        .update_requirement(
            serde_json::from_value::<UpdateRequirementInput>(serde_json::json!({
                "scope_id":"default", "id":"req_overtime", "statement":"Overtime is recorded."
            }))
            .unwrap(),
        )
        .unwrap();
    assert_lifecycle_then_content(&entries(&store, &scope, NodeType::Requirement));
}

#[test]
fn boundary_content_change_creates_a_revision() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    store
        .update_boundary(
            serde_json::from_value::<UpdateBoundaryInput>(serde_json::json!({
                "scope_id":"default", "id":"boundary_native", "statement":"Boundary B"
            }))
            .unwrap(),
        )
        .unwrap();
    let entries = entries(&store, &scope, NodeType::Boundary);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].outcome, SaveOutcome::Changed);
    assert_ne!(entries[1].revision, entries[0].revision);
}

#[test]
fn bulk_replacement_reviews_existing_unenrolled_and_enrolled_records() {
    let (_dir, store, scope) = seeded_requirement_store();
    let path = crate::shards::sources_path(&store.layout, &scope);
    let source = Source {
        created: None,
        updated: None,
        schema_version: SchemaVersion(2),
        scope_id: scope.clone(),
        id: id("source_bulk"),
        declared_by: None,
        declaration_address: None,
        name: "Source A".into(),
        source_type: provenance_core::SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    };

    store
        .replace_graph_records(&path, vec![source.clone()])
        .unwrap();
    assert_eq!(entries(&store, &scope, NodeType::Source), [] as [provenance_core::review::ReviewEntry; 0]);

    let mut changed = source;
    changed.name = "Source B".into();
    store
        .replace_graph_records(&path, vec![changed.clone()])
        .unwrap();
    let first = entries(&store, &scope, NodeType::Source);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].outcome, SaveOutcome::Enrolled);

    changed.name = "Source C".into();
    store.replace_graph_records(&path, vec![changed]).unwrap();
    let second = entries(&store, &scope, NodeType::Source);
    assert_eq!(second.len(), 2);
    assert_eq!(second[1].outcome, SaveOutcome::Changed);
    assert_eq!(
        store.list_sources(&scope).unwrap()[0].schema_version,
        REVIEW_SCHEMA_VERSION
    );
}
