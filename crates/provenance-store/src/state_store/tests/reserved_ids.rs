use super::initialized_store;
use crate::state_store::{CreateSourceInput, ScopeShards};
use provenance_core::{Message, MessageRole, NodeType, SourceType, StableId, Thread, ThreadParent, ThreadStatus};
use provenance_macros::verifies;

#[test]
#[verifies("rule_record_id_excludes_command_keywords", examples)]
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
    assert!(
        error.to_string().contains("reserved record ID search"),
        "{error}"
    );
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
    assert!(
        error.to_string().contains("reserved record ID check"),
        "{error}"
    );
}

#[test]
fn an_existing_keyword_id_survives_a_scope_import() {
    let (_directory, store, scope) = initialized_store();
    let source = provenance_core::Source {
        created: None,
        updated: None,
        schema_version: provenance_core::SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new("search").unwrap(),
        declared_by: None,
        declaration_address: None,
        name: "Existing record".into(),
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
    let path = crate::shards::sources_path(&store.layout, &scope);
    crate::jsonl::write_jsonl_atomic(&path, std::slice::from_ref(&source)).unwrap();
    let shards = ScopeShards {
        sources: std::slice::from_ref(&source),
        ..ScopeShards::default()
    };
    store.import_scope(&scope, &shards).unwrap();
    assert_eq!(store.list_sources(&scope).unwrap()[0].id.as_str(), "search");
}

#[test]
fn a_keyword_id_from_another_record_kind_is_not_preserved() {
    let (_directory, store, scope) = initialized_store();
    let message = Message {
        schema_version: provenance_core::SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new("search").unwrap(),
        thread_id: StableId::new("thread_old").unwrap(),
        role: MessageRole::User,
        body: "Existing message".into(),
        created_at: 1,
        ai_metadata: None,
    };
    let path = crate::shards::messages_path(&store.layout, &scope);
    crate::jsonl::write_jsonl_atomic(&path, std::slice::from_ref(&message)).unwrap();
    let thread = Thread {
        schema_version: provenance_core::SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new("search").unwrap(),
        parent: ThreadParent {
            node_type: NodeType::Source,
            node_id: StableId::new("source_old").unwrap(),
        },
        status: ThreadStatus::Active,
        created_at: 1,
    };
    let shards = ScopeShards {
        threads: std::slice::from_ref(&thread),
        ..ScopeShards::default()
    };
    let error = store.import_scope(&scope, &shards).unwrap_err();
    assert!(error.to_string().contains("reserved record ID search"), "{error}");
}
