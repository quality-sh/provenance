use super::initialized_store;
use crate::state_store::{CreateSourceInput, ScopeShards};
use provenance_core::{SourceType, StableId};

#[test]
fn a_writer_refuses_a_command_keyword_as_a_new_record_id() {
    let (_directory, store, scope) = initialized_store();
    let error = store
        .create_source(CreateSourceInput {
            scope_id: scope.clone(),
            id: StableId::new("search").unwrap(),
            name: "A search record".into(),
            source_type: SourceType::Document,
            url: None,
            reference: None,
            commit_pin: None,
            effective_date: None,
            review_date: None,
            supersedes: Vec::new(),
            origin_thread: None,
            origin_message: None,
        })
        .unwrap_err();
    assert!(error.to_string().contains("reserved record ID search"), "{error}");
    assert!(store.list_sources(&scope).unwrap().is_empty());
}

#[test]
fn scope_import_refuses_a_new_command_keyword_id() {
    let (_directory, store, scope) = initialized_store();
    let source = provenance_core::Source {
        created: None,
        updated: None,
        schema_version: provenance_core::SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new("check").unwrap(),
        declared_by: None,
        declaration_address: None,
        name: "A check record".into(),
        source_type: SourceType::Document,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    };
    let shards = ScopeShards {
        sources: std::slice::from_ref(&source),
        ..ScopeShards::default()
    };
    let error = store.import_scope(&scope, &shards).unwrap_err();
    assert!(error.to_string().contains("reserved record ID check"), "{error}");
}
