use super::{
    native_review_writers::{id, seed_native_records},
    seeded_requirement_store,
};
use crate::state_store::{
    EditQuestionInput, UpdateBoundaryInput, UpdateDomainInput, UpdateRequirementInput,
    UpdateResolutionInput, UpdateRuleInput, UpdateSourceInput, UpdateTopicInput,
};
use provenance_core::review::REVIEW_SCHEMA_VERSION;
use provenance_core::{NodeType, SchemaVersion, Source};

fn revision(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    kind: NodeType,
    record: &str,
) -> Option<provenance_core::StableId> {
    store
        .record_edit_state(scope, kind, &id(record))
        .unwrap()
        .revision
}

/// Runs a lifecycle-only update, then a content update, and checks that only
/// the second one changes the review revision.
fn assert_lifecycle_then_content(
    store: &crate::state_store::StateStore,
    scope: &provenance_core::ScopeId,
    kind: NodeType,
    record: &str,
    lifecycle: impl FnOnce(),
    content: impl FnOnce(),
) {
    let created = revision(store, scope, kind, record);
    assert!(created.is_some(), "{kind:?}");
    lifecycle();
    let kept = revision(store, scope, kind, record);
    assert_eq!(kept, created, "{kind:?}");
    content();
    assert_ne!(revision(store, scope, kind, record), kept, "{kind:?}");
}

#[test]
fn source_review_date_keeps_revision_and_name_changes_it() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    let update = |value: serde_json::Value| {
        store
            .update_source(serde_json::from_value::<UpdateSourceInput>(value).unwrap())
            .unwrap();
    };
    assert_lifecycle_then_content(
        &store,
        &scope,
        NodeType::Source,
        "source_native",
        || update(serde_json::json!({"scope_id":"default", "id":"source_native", "review_date":1})),
        || {
            update(
                serde_json::json!({"scope_id":"default", "id":"source_native", "name":"Source B"}),
            );
        },
    );
}

#[test]
fn resolution_review_on_keeps_revision_and_position_changes_it() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    let update = |value: serde_json::Value| {
        store
            .update_resolution(serde_json::from_value::<UpdateResolutionInput>(value).unwrap())
            .unwrap();
    };
    assert_lifecycle_then_content(
        &store,
        &scope,
        NodeType::Resolution,
        "resolution_native",
        || {
            update(serde_json::json!({
                "scope_id":"default", "id":"resolution_native", "review_on":"2027-01-01"
            }));
        },
        || {
            update(serde_json::json!({
                "scope_id":"default", "id":"resolution_native", "position":"Use position B"
            }));
        },
    );
}

#[test]
fn rule_archive_stamp_keeps_revision_after_statement_changes_it() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    let created = revision(&store, &scope, NodeType::Rule, "rule_native");
    store
        .update_rule(
            serde_json::from_value::<UpdateRuleInput>(serde_json::json!({
                "scope_id":"default", "id":"rule_native", "statement":"The system uses position B"
            }))
            .unwrap(),
        )
        .unwrap();
    let changed = revision(&store, &scope, NodeType::Rule, "rule_native");
    assert_ne!(changed, created);
    store
        .update_rule(
            serde_json::from_value::<UpdateRuleInput>(serde_json::json!({
                "scope_id":"default", "id":"rule_native", "status":"archived",
                "archived_in_commit":{"commit":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}
            }))
            .unwrap(),
        )
        .unwrap();
    assert_eq!(
        revision(&store, &scope, NodeType::Rule, "rule_native"),
        changed
    );
}

#[test]
fn shaping_claims_keep_revisions_and_text_changes_them() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    assert_lifecycle_then_content(
        &store,
        &scope,
        NodeType::Topic,
        "topic_native",
        || {
            store
                .claim_topic(&scope, &id("topic_native"), "author")
                .unwrap();
        },
        || {
            store
                .edit_topic(
                    serde_json::from_value::<UpdateTopicInput>(serde_json::json!({
                        "scope_id":"default", "id":"topic_native", "title":"Topic B"
                    }))
                    .unwrap(),
                )
                .unwrap();
        },
    );
    assert_lifecycle_then_content(
        &store,
        &scope,
        NodeType::Question,
        "question_native",
        || {
            store
                .claim_question(&scope, &id("question_native"), "author")
                .unwrap();
        },
        || {
            store
                .edit_question(
                    serde_json::from_value::<EditQuestionInput>(serde_json::json!({
                        "scope_id":"default", "id":"question_native", "question":"Is B correct?"
                    }))
                    .unwrap(),
                )
                .unwrap();
        },
    );
}

#[test]
fn domain_color_and_requirement_status_keep_revisions() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    let domain = |value: serde_json::Value| {
        store
            .update_domain(serde_json::from_value::<UpdateDomainInput>(value).unwrap())
            .unwrap();
    };
    assert_lifecycle_then_content(
        &store,
        &scope,
        NodeType::Domain,
        "domain_native",
        || domain(serde_json::json!({"scope_id":"default", "id":"domain_native", "color":"blue"})),
        || {
            domain(
                serde_json::json!({"scope_id":"default", "id":"domain_native", "name":"Domain B"}),
            );
        },
    );
    let requirement = |value: serde_json::Value| {
        store
            .update_requirement(serde_json::from_value::<UpdateRequirementInput>(value).unwrap())
            .unwrap();
    };
    assert_lifecycle_then_content(
        &store,
        &scope,
        NodeType::Requirement,
        "req_overtime",
        || {
            requirement(serde_json::json!({
                "scope_id":"default", "id":"req_overtime", "status":"refinement"
            }));
        },
        || {
            requirement(serde_json::json!({
                "scope_id":"default", "id":"req_overtime", "statement":"Overtime is recorded."
            }));
        },
    );
}

#[test]
fn boundary_content_change_creates_a_revision() {
    let (_dir, store, scope) = seeded_requirement_store();
    seed_native_records(&store, &scope);
    let created = revision(&store, &scope, NodeType::Boundary, "boundary_native");
    store
        .update_boundary(
            serde_json::from_value::<UpdateBoundaryInput>(serde_json::json!({
                "scope_id":"default", "id":"boundary_native", "statement":"Boundary B"
            }))
            .unwrap(),
        )
        .unwrap();
    assert_ne!(
        revision(&store, &scope, NodeType::Boundary, "boundary_native"),
        created
    );
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
    assert_eq!(
        revision(&store, &scope, NodeType::Source, "source_bulk"),
        None
    );

    let mut changed = source;
    changed.name = "Source B".into();
    store
        .replace_graph_records(&path, vec![changed.clone()])
        .unwrap();
    let enrolled = revision(&store, &scope, NodeType::Source, "source_bulk");
    assert!(enrolled.is_some());

    changed.name = "Source C".into();
    store.replace_graph_records(&path, vec![changed]).unwrap();
    assert_ne!(
        revision(&store, &scope, NodeType::Source, "source_bulk"),
        enrolled
    );
    assert_eq!(
        store.list_sources(&scope).unwrap()[0].schema_version,
        REVIEW_SCHEMA_VERSION
    );
}
