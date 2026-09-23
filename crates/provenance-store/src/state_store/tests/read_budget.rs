//! The write/read size invariant: a resource write the store accepts stays
//! inside the byte budget the supported resource member and page reads
//! enforce, and an oversized mutation is refused before publication.

use super::{initialized_store, seeded_requirement_store};
use crate::operations::catalog::{
    self, ContextResolver, ExecutionNeeds, PreparedContext, PreparedRead, RequestedContext,
};
use crate::operations::read_policy::ReadPolicy;
use crate::state_store::{
    read_budget::ReadBudget, CreateResolutionInput, CreateSourceInput, PostMessageInput,
    SourceClearField, UpdateSourceInput,
};
use crate::write_error::{SourceFailure, WriteError, WriteFailure};
use provenance_core::model::ProjectionRow;
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::protocol::SDK_PROTOCOL_VERSION;
use provenance_core::{
    Message, MessageRole, NodeType, ResolutionStatus, ScopeId, Source, SourceType, StableId,
    Thread, ThreadParent, ThreadStatus, SUPPORTED_SCHEMA_VERSION,
};
use provenance_macros::verifies;
use serde_json::json;
use std::sync::Arc;

/// The read budget the supported resource member and page reads enforce.
const READ_BUDGET: usize = crate::cache::read::page::RESOURCE_RECORD_BYTES;

/// The transport body budget the HTTP and MCP surfaces accept.
const TRANSPORT_BUDGET: usize = 1024 * 1024;

const OVERSIZED_ID: &str = "source_oversized";

struct Target(camino::Utf8PathBuf);

impl ContextResolver for Target {
    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Ok(PreparedContext::read(PreparedRead {
            root: self.0.clone(),
            scope: ScopeId::new("default").unwrap(),
            policy: ReadPolicy::default(),
            requested_target: "test".into(),
            external: true,
        }))
    }
}

fn blank_source(scope: &ScopeId, id: &str) -> Source {
    Source {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        created: None,
        updated: None,
        declared_by: None,
        declaration_address: None,
        name: String::new(),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

/// A write request whose stored record holds exactly `target` read-accounted
/// bytes, measured the same way the supported reads measure it.
fn source_of_stored_bytes(scope: &ScopeId, id: &str, target: usize) -> CreateSourceInput {
    let base = ReadBudget::read_bytes(&blank_source(scope, id)).unwrap();
    CreateSourceInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        name: "x".repeat(target - base),
        source_type: SourceType::Policy,
        url: None,
        reference: None,
        commit_pin: None,
        effective_date: None,
        review_date: None,
        supersedes: Vec::new(),
        origin_thread: None,
        origin_message: None,
    }
}

fn wire_call(request: serde_json::Value) -> serde_json::Value {
    let mut call = json!({"context": {"repository": "test", "scope": "default"}});
    call["request"] = request;
    call
}

async fn member_read(root: &camino::Utf8Path, id: &str) -> Result<serde_json::Value, String> {
    catalog::invoke_with(
        "get-source-v2",
        SDK_PROTOCOL_VERSION,
        wire_call(json!({"id": id})),
        Arc::new(Target(root.to_path_buf())),
    )
    .await
    .map_err(|failure| failure.error["kind"].as_str().unwrap_or("").to_string())
}

async fn list_read(root: &camino::Utf8Path) -> Result<serde_json::Value, String> {
    catalog::invoke_with(
        "page-sources-v2",
        SDK_PROTOCOL_VERSION,
        wire_call(json!({"limit": 50, "cursor": null})),
        Arc::new(Target(root.to_path_buf())),
    )
    .await
    .map_err(|failure| failure.error["kind"].as_str().unwrap_or("").to_string())
}

/// The trigger case: the request body fits the transport budget while the
/// stored record exceeds the read budget, so only the write-side refusal
/// keeps the accepted record readable.
#[test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
fn trigger_body_fits_the_transport_budget_and_exceeds_the_read_budget() {
    let call = wire_call(json!({
        "scope_id": "default",
        "id": OVERSIZED_ID,
        "name": "x".repeat(READ_BUDGET),
        "source_type": "policy",
        "supersedes": []
    }));
    let body = serde_json::to_vec(&call).unwrap().len();
    assert!(
        body < TRANSPORT_BUDGET,
        "fixture body {body} must stay inside the transport budget"
    );
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn oversized_write_is_refused_before_publication_and_read_stays_clean() {
    let (dir, store, scope) = initialized_store();
    let error = store
        .create_source(source_of_stored_bytes(
            &scope,
            OVERSIZED_ID,
            READ_BUDGET + 1,
        ))
        .expect_err("the oversized write is refused");
    let failure = error
        .downcast_ref::<SourceFailure>()
        .expect("the refusal is typed");
    assert!(matches!(failure.failure, WriteFailure::RecordTooLarge));

    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let kind = member_read(&root, OVERSIZED_ID)
        .await
        .expect_err("the refused record does not exist");
    assert_eq!(kind, "resource_not_found");
    let page = list_read(&root).await.unwrap();
    assert_eq!(page["result"]["items"].as_array().unwrap().len(), 0);
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn record_inside_the_read_budget_is_accepted_and_readable() {
    let (dir, store, scope) = initialized_store();
    // The created stamp rides on the stored record, so the fixture keeps a
    // margin that is larger than any stamp and stays inside the budget.
    let source = store
        .create_source(source_of_stored_bytes(
            &scope,
            OVERSIZED_ID,
            READ_BUDGET - 192,
        ))
        .expect("a record inside the read budget is accepted");
    assert!(ReadBudget::read_bytes(&source).unwrap() <= READ_BUDGET);

    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let member = member_read(&root, OVERSIZED_ID).await.unwrap();
    assert_eq!(member["result"]["id"], OVERSIZED_ID);
    let page = list_read(&root).await.unwrap();
    assert_eq!(page["result"]["items"].as_array().unwrap().len(), 1);
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn write_side_measure_matches_the_read_side_accounting() {
    let (dir, store, scope) = initialized_store();
    let source = store
        .create_source(source_of_stored_bytes(&scope, OVERSIZED_ID, 4096))
        .unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    member_read(&root, OVERSIZED_ID).await.unwrap();

    let cache = crate::cache::open_cache(&store.layout).await.unwrap();
    let expression = crate::cache::read::page::byte_expression(Source::COLUMNS);
    let stored: i64 = sqlx::query_scalar(&format!(
        "SELECT {expression} FROM sources WHERE scope_id = ? AND id = ?"
    ))
    .bind(scope.as_str())
    .bind(OVERSIZED_ID)
    .fetch_one(cache.pool())
    .await
    .unwrap();
    cache.close().await.unwrap();
    assert_eq!(
        usize::try_from(stored).unwrap(),
        ReadBudget::read_bytes(&source).unwrap(),
        "the write-side measure must equal the stored byte count the reads account"
    );
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn oversized_update_is_refused_and_keeps_the_published_record() {
    let (dir, store, scope) = initialized_store();
    store
        .create_source(source_of_stored_bytes(&scope, OVERSIZED_ID, 1024))
        .unwrap();
    let shard = crate::shards::sources_path(&store.layout, &scope);
    let before = std::fs::read_to_string(&shard).unwrap();

    let error = store
        .update_source(UpdateSourceInput {
            scope_id: scope.clone(),
            id: StableId::new(OVERSIZED_ID).unwrap(),
            declared_by: None,
            name: Some("y".repeat(READ_BUDGET)),
            source_type: None,
            url: None,
            reference: None,
            commit_pin: None,
            effective_date: None,
            review_date: None,
            supersedes: None,
            clear_fields: Vec::<SourceClearField>::new(),
        })
        .expect_err("the oversized update is refused");
    let failure = error
        .downcast_ref::<SourceFailure>()
        .expect("the refusal is typed");
    assert!(matches!(failure.failure, WriteFailure::RecordTooLarge));
    assert_eq!(
        std::fs::read_to_string(&shard).unwrap(),
        before,
        "a refused update leaves canonical state unchanged"
    );

    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let member = member_read(&root, OVERSIZED_ID).await.unwrap();
    assert_eq!(member["result"]["id"], OVERSIZED_ID);
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn oversized_message_write_is_refused_and_persists_nothing() {
    let (_dir, store, scope) = initialized_store();
    let thread = StableId::new("thread_x").unwrap();
    let error = store
        .append_discussion_message(&scope, &thread, MessageRole::User, "x".repeat(READ_BUDGET))
        .expect_err("a message above the message read budget is refused");
    let failure = error
        .downcast_ref::<SourceFailure>()
        .expect("the refusal is typed");
    assert!(matches!(failure.failure, WriteFailure::RecordTooLarge));
    assert!(
        store.list_messages(&scope).unwrap().is_empty(),
        "a refused message is not published"
    );

    // A refused write persists nothing, so the accepted message takes the
    // first identity and lands exactly at the message read budget.
    let base = ReadBudget::read_bytes(&Message {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new("msg_000001").unwrap(),
        thread_id: thread,
        role: MessageRole::User,
        body: String::new(),
        created_at: 1,
        ai_metadata: None,
    })
    .unwrap();
    let message = store
        .append_discussion_message(
            &scope,
            &StableId::new("thread_x").unwrap(),
            MessageRole::User,
            "x".repeat(crate::cache::read::page::RECORD_BYTES - base),
        )
        .expect("a message at the message read budget is accepted");
    assert_eq!(message.id.as_str(), "msg_000001");
}

#[test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
fn oversized_post_refuses_before_creating_or_archiving_threads() {
    let (_dir, store, scope) = initialized_store();
    let parent = ThreadParent {
        node_type: NodeType::Requirement,
        node_id: StableId::new("req_overtime").unwrap(),
    };
    let post = |body: String| PostMessageInput {
        scope_id: scope.clone(),
        parent: parent.clone(),
        role: MessageRole::User,
        body,
    };
    let error = store
        .post_thread_message(post("x".repeat(READ_BUDGET)))
        .expect_err("an oversized message is refused before thread creation");
    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::RecordTooLarge
    ));
    assert!(store.list_threads(&scope).unwrap().is_empty());
    assert!(store.list_messages(&scope).unwrap().is_empty());

    let first = store.post_thread_message(post("small".into())).unwrap();
    let sibling = Thread {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope.clone(),
        id: StableId::new("thread_sibling").unwrap(),
        parent: parent.clone(),
        status: ThreadStatus::Active,
        created_at: 2,
    };
    crate::jsonl::write_jsonl_atomic(
        &crate::shards::threads_path(&store.layout, &scope),
        &[first.thread.clone(), sibling],
    )
    .unwrap();
    let before = std::fs::read(crate::shards::threads_path(&store.layout, &scope)).unwrap();
    let error = store
        .post_thread_message(post("x".repeat(READ_BUDGET)))
        .expect_err("an oversized message is refused before changing an existing thread");
    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::RecordTooLarge
    ));
    assert_eq!(
        std::fs::read(crate::shards::threads_path(&store.layout, &scope)).unwrap(),
        before
    );
    assert_eq!(store.list_messages(&scope).unwrap().len(), 1);
    assert_eq!(first.message.id.as_str(), "msg_000001");
}

fn resolution_input(scope: &ScopeId, id: &str, title: String) -> CreateResolutionInput {
    CreateResolutionInput {
        scope_id: scope.clone(),
        id: StableId::new(id).unwrap(),
        title,
        requirement_ids: vec![StableId::new("req_overtime").unwrap()],
        supersedes: Vec::new(),
        position: "Pay overtime".into(),
        rationale: "The requirement says so".into(),
        status: ResolutionStatus::Proposed,
        context: None,
        enforcement: None,
        confidence: Some(1.0),
        inputs: Vec::new(),
        made_by: None,
        approved_by: None,
        approved_at: None,
        origin_thread: None,
        origin_message: None,
    }
}

#[tokio::test]
#[verifies("rule_accepted_writes_stay_readable", examples)]
async fn resolution_real_accounting_matches_sqlite_and_refuses_boundary_overflow() {
    let (dir, store, scope) = seeded_requirement_store();
    let baseline = store
        .create_resolution(resolution_input(&scope, "res_base", String::new()))
        .unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    catalog::invoke_with(
        "get-resolution-v2",
        SDK_PROTOCOL_VERSION,
        wire_call(json!({"id": "res_base"})),
        Arc::new(Target(root)),
    )
    .await
    .unwrap();
    let cache = crate::cache::open_cache(&store.layout).await.unwrap();
    let expression =
        crate::cache::read::page::byte_expression(provenance_core::Resolution::COLUMNS);
    let stored: i64 = sqlx::query_scalar(&format!(
        "SELECT {expression} FROM resolutions WHERE scope_id = ? AND id = ?"
    ))
    .bind(scope.as_str())
    .bind("res_base")
    .fetch_one(cache.pool())
    .await
    .unwrap();
    for score in [0.0, 0.00001, 0.123_456_789_012_345_66, 1.0] {
        let real_bytes: i64 = sqlx::query_scalar("SELECT length(CAST(? AS BLOB))")
            .bind(score)
            .fetch_one(cache.pool())
            .await
            .unwrap();
        let mut candidate = baseline.clone();
        candidate.confidence = Some(score);
        assert_eq!(
            ReadBudget::read_bytes(&candidate).unwrap(),
            usize::try_from(stored - 3 + real_bytes).unwrap(),
            "confidence {score:?} must use SQLite's REAL byte count"
        );
    }
    cache.close().await.unwrap();
    assert_eq!(
        ReadBudget::read_bytes(&baseline).unwrap(),
        usize::try_from(stored).unwrap()
    );

    let title_bytes = READ_BUDGET - usize::try_from(stored).unwrap() + 1;
    let error = store
        .create_resolution(resolution_input(
            &scope,
            "res_edge",
            "x".repeat(title_bytes),
        ))
        .expect_err("the SQLite byte count exceeds the resource read budget");
    assert!(matches!(
        WriteError(error).safe(),
        WriteFailure::RecordTooLarge
    ));
    assert!(store
        .list_resolutions(&scope)
        .unwrap()
        .iter()
        .all(|r| r.id.as_str() != "res_edge"));
}
