//! The page budget refusal at each place where a page maps the stop of a
//! statement by the work bound.
//!
//! Each test reads one page through its public operation twice: first
//! with a work bound of no steps on its own thread, then with the engine's
//! bound. With no steps, the file stops the first statement of the page
//! that runs 1,000 steps of the engine. The records of each test make that
//! statement the one that the test names, and keep each statement before
//! it under 1,000 steps. A page whose statement walks an index in the
//! order of the page stops at its limit, so those pages ask for more rows
//! than the test writes. The page budget refusal is the typed
//! `page_budget_exceeded` failure, which the transport answers with 409.

use super::{repository_at, root_of, seeded_store};
use crate::cache::tests::fixtures::append_record;
use crate::operations::{queries, read_policy::ReadPolicy};
use crate::review::{read_discussion_list, read_discussion_messages, read_discussions};
use crate::state_store::StateStore;
use crate::test_probes::set_page_work_steps;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::threads::{
    Discussion, DiscussionListQuery, DiscussionMessagesQuery, DiscussionQuery, DiscussionSelector,
    DiscussionStatusFilter,
};
use provenance_core::{NodeType, ThreadParent};
use provenance_macros::verifies;
use serde_json::{json, Value};
use std::future::Future;

/// The records of each kind that make the named statement run past 1,000
/// steps.
const MANY: usize = 80;

/// Reads `page` with a work bound of no steps, then with the engine's
/// bound. The first read gives the page budget refusal, and the second
/// answers.
async fn refused_then_answered<T, P, F>(page: P)
where
    P: Fn() -> F,
    F: Future<Output = anyhow::Result<T>>,
{
    set_page_work_steps(Some(0));
    let bounded = page().await;
    set_page_work_steps(None);
    let Err(refused) = bounded else {
        panic!("the page answered within a bound of no steps");
    };
    assert_eq!(
        refused.downcast_ref::<ReadFailure>(),
        Some(&ReadFailure::PageBudgetExceeded),
        "{refused:#}"
    );
    if let Err(failed) = page().await {
        panic!("the page failed with the engine's bound: {failed:#}");
    }
}

fn parent() -> ThreadParent {
    serde_json::from_value(json!({"node_type": "requirement", "node_id": "req_overtime"})).unwrap()
}

fn write_discussion(store: &StateStore, action: Value) -> Discussion {
    store
        .write_discussion(
            serde_json::from_value(json!({
                "scope_id": "default", "parent": parent(), "actor": "ben", "action": action
            }))
            .unwrap(),
        )
        .unwrap()
}

/// [`MANY`] Discussions on `req_overtime`, each with one message.
fn many_discussions(store: &StateStore) {
    for i in 0..MANY {
        write_discussion(
            store,
            json!({"kind": "start", "role": "user", "body": format!("Concern {i}")}),
        );
    }
}

/// [`MANY`] copies of `req_overtime` that refine it, each matching the
/// text `overtime`.
fn many_requirements(store: &StateStore, scope: &provenance_core::ScopeId) {
    let path = crate::shards::requirements_path(&store.layout, scope);
    let mut record = json!(store.list_requirements(scope).unwrap()[0]);
    for i in 0..MANY {
        record["id"] = json!(format!("req_many_{i:03}"));
        record["refines"] = json!(["req_overtime"]);
        append_record(&path, &record);
    }
}

/// A page of the Discussions of `req_overtime`. Its limit is over
/// [`MANY`], so the statement walks every row and does not stop at the
/// limit.
fn discussions_query() -> DiscussionQuery {
    DiscussionQuery {
        parent: parent(),
        limit: 200,
        cursor: None,
    }
}

/// `operations/queries/records.rs`: the search page.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_search_page_past_the_bound_is_the_page_budget_refusal() {
    let (dir, store, scope) = seeded_store();
    many_requirements(&store, &scope);
    let root = root_of(&dir);
    refused_then_answered(|| async {
        queries::search(
            &repository_at(&root),
            ReadPolicy::default(),
            serde_json::from_value(json!({"text": "overtime", "limit": 10})).unwrap(),
        )
        .await
    })
    .await;
}

/// `operations/queries/document.rs`: the document page.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_document_page_past_the_bound_is_the_page_budget_refusal() {
    let (dir, store, scope) = seeded_store();
    many_requirements(&store, &scope);
    let root = root_of(&dir);
    refused_then_answered(|| async {
        queries::read_document(
            &repository_at(&root),
            ReadPolicy::default(),
            serde_json::from_value(json!({"id": "req_overtime", "cursor": null, "limit": 50}))
                .unwrap(),
        )
        .await
    })
    .await;
}

/// `review/discussion_reads.rs`, the addressed Discussions of a parent.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_page_of_addressed_discussions_past_the_bound_is_the_page_budget_refusal() {
    let (dir, store, _scope) = seeded_store();
    many_discussions(&store);
    let root = root_of(&dir);
    refused_then_answered(|| async {
        read_discussions(
            &repository_at(&root),
            ReadPolicy::default(),
            discussions_query(),
        )
        .await
    })
    .await;
}

/// `review/discussion_reads.rs`, the legacy threads of a parent. The
/// parent has no Discussion, so the statement of the addressed stage stays
/// small.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_page_of_legacy_threads_past_the_bound_is_the_page_budget_refusal() {
    let (dir, store, scope) = seeded_store();
    let threads = crate::shards::threads_path(&store.layout, &scope);
    let messages = store
        .layout
        .state_dir()
        .join("scopes/default/threads/2026-09.jsonl");
    for i in 0..MANY {
        let thread = format!("thread_legacy_{i:03}");
        append_record(
            &threads,
            &json!({"scope_id": "default", "id": thread, "parent": parent(),
                "status": "active", "created_at": i}),
        );
        append_record(
            &messages,
            &json!({"scope_id": "default", "id": format!("msg_legacy_{i:03}"),
                "thread_id": thread, "role": "user", "body": "Legacy message", "created_at": i}),
        );
    }
    let root = root_of(&dir);
    refused_then_answered(|| async {
        read_discussions(
            &repository_at(&root),
            ReadPolicy::default(),
            discussions_query(),
        )
        .await
    })
    .await;
}

/// `review/discussion_messages.rs`: the messages of one Discussion.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_page_of_messages_past_the_bound_is_the_page_budget_refusal() {
    let (dir, store, _scope) = seeded_store();
    let mut discussion = write_discussion(
        &store,
        json!({"kind": "start", "role": "user", "body": "Concern"}),
    );
    for i in 0..MANY {
        discussion = write_discussion(
            &store,
            json!({"kind": "reply", "discussion_id": discussion.discussion_id,
                "expected_version": discussion.version, "role": "user",
                "body": format!("Reply {i}")}),
        );
    }
    let root = root_of(&dir);
    let selector = DiscussionSelector::Discussion {
        discussion_id: discussion.discussion_id,
    };
    refused_then_answered(|| async {
        read_discussion_messages(
            &repository_at(&root),
            ReadPolicy::default(),
            DiscussionMessagesQuery {
                parent: parent(),
                selector: selector.clone(),
                limit: 10,
                cursor: None,
            },
        )
        .await
    })
    .await;
}

/// `review/discussion_discovery.rs`: the Discussion list of a scope.
#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn a_discussion_list_past_the_bound_is_the_page_budget_refusal() {
    let (dir, store, _scope) = seeded_store();
    many_discussions(&store);
    let root = root_of(&dir);
    refused_then_answered(|| async {
        read_discussion_list(
            &repository_at(&root),
            ReadPolicy::default(),
            DiscussionListQuery {
                parent: None,
                allowed_parent_kinds: vec![NodeType::Requirement],
                status: DiscussionStatusFilter::All,
                limit: 200,
                cursor: None,
            },
        )
        .await
    })
    .await;
}
