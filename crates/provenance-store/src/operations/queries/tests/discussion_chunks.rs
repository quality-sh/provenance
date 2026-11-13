//! The discussion list and conversation reads over a store whose list
//! holds fewer values than the parent kinds of the query. Each kind then
//! goes in a statement of its own, and the reader merges the rows by
//! discussion id. The file adapter's list holds 500 values, so its reads
//! run one statement. No Rule: implementation aid for the merge in
//! `review/discussion_discovery.rs`; the pages must be the pages of one
//! statement.

use super::comparison::test_stores::TestStore;
use crate::cache::{catch_up_state, open_cache};
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::read_database::file::FileReadDatabase;
use crate::read_database::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};
use crate::repository::{Repository, RepositoryIdentity};
use crate::review::{read_discussion_conversation, read_discussion_list};
use crate::settings::Settings;
use crate::storage::local::{LocalSectionLock, LocalStateStorage};
use provenance_core::threads::{
    DiscussionConversationQuery, DiscussionListQuery, DiscussionStatusFilter,
};
use provenance_core::{NodeType, StableId};
use std::sync::Arc;

/// The file adapter, with reads whose list holds one value.
#[derive(Debug)]
struct OneValueLists(FileReadDatabase);

/// A read transaction of the file whose list holds one value.
struct OneValueList(Transaction);

impl SqlTransaction for OneValueList {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, Vec<SqlRow>> {
        self.0.fetch(sql, values)
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: Vec<SqlValue>) -> SqlFuture<'a, u64> {
        self.0.execute(sql, values)
    }

    fn attempt<'a>(
        &'a mut self,
        statements: &'a [Statement],
    ) -> SqlFuture<'a, Result<(), anyhow::Error>> {
        self.0.attempt(statements)
    }

    fn commit(self: Box<Self>) -> SqlFuture<'static, ()> {
        self.0.commit()
    }

    fn bound_work(&mut self, steps: u64) -> SqlFuture<'_, ()> {
        self.0.bound_work(steps)
    }

    fn work_unit(&self) -> crate::read_database::WorkUnit {
        self.0.work_unit()
    }

    fn max_list_values(&self) -> usize {
        1
    }
}

impl ReadDatabase for OneValueLists {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        self.0
            .read(Box::new(move |lent| work(Box::new(OneValueList(lent)))))
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        self.0.write(work)
    }

    fn max_bound_values(&self) -> usize {
        self.0.max_bound_values()
    }

    fn cursor_key(&self) -> anyhow::Result<[u8; 32]> {
        self.0.cursor_key()
    }

    fn location(&self) -> String {
        self.0.location()
    }
}

/// The repository of `store` over the file with one-value lists.
fn one_value_lists(store: &TestStore) -> Repository {
    let layout = store.layout();
    Repository::with_storage(
        RepositoryIdentity::new(store.root.clone(), store.scope.clone(), Settings::default()),
        Arc::new(LocalStateStorage::new(layout.clone())),
        Arc::new(LocalSectionLock::new(layout.clone())),
        Some(Arc::new(OneValueLists(FileReadDatabase::new(layout)))),
    )
    .unwrap()
}

fn policy() -> ReadPolicy {
    ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly)
}

/// Six discussions whose ids alternate between three parent kinds, so the
/// rows of each kind's statement interleave by id. Each parent is a record
/// that the pinned store writes through its state store, so a read of the
/// discussion finds its parent.
const DISCUSSIONS: [(&str, &str, &str); 6] = [
    ("discussion_a", "rule", "rule_overtime_001"),
    ("discussion_b", "requirement", "req_overtime"),
    ("discussion_c", "question", "question_threshold"),
    ("discussion_d", "rule", "rule_overtime_001"),
    ("discussion_e", "requirement", "req_overtime"),
    ("discussion_f", "question", "question_threshold"),
];

/// The rows of the discussions, one thread and opening message each,
/// written in reverse id order.
async fn seed(store: &TestStore) {
    let cache = open_cache(&store.layout()).await.unwrap();
    let pool = cache.pool();
    for (index, (id, kind, parent)) in DISCUSSIONS.iter().enumerate().rev() {
        let thread = format!("thread_{id}");
        let message = format!("message_{id}");
        sqlx::query(
            "INSERT INTO threads (scope_id, id, parent_type, parent_id, status, created_at, payload) \
             VALUES ('default', ?, ?, ?, 'active', ?, '{}')",
        )
        .bind(&thread)
        .bind(kind)
        .bind(parent)
        .bind(i64::try_from(index).unwrap())
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO messages (scope_id, id, thread_id, role, body, created_at, payload) \
             VALUES ('default', ?, ?, 'user', ?, ?, '{}')",
        )
        .bind(&message)
        .bind(&thread)
        .bind(format!("Opening of {id}"))
        .bind(i64::try_from(index).unwrap())
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO discussions (scope_id, discussion_id, parent, thread_id, \
             root_message_id, message_ids, status, version, actor, outcomes, disposition_id) \
             VALUES ('default', ?, ?, ?, ?, ?, 'active', 1, 'ben', '[]', NULL)",
        )
        .bind(id)
        .bind(serde_json::json!({"node_type": kind, "node_id": parent}).to_string())
        .bind(&thread)
        .bind(&message)
        .bind(serde_json::json!([message]).to_string())
        .execute(pool)
        .await
        .unwrap();
    }
    cache.close().await.unwrap();
}

fn kinds() -> Vec<NodeType> {
    vec![NodeType::Rule, NodeType::Requirement, NodeType::Question]
}

/// Every page of the list read over `repository`, as JSON, and the
/// discussion ids in the order the pages gave them.
async fn pages(repository: &Repository, limit: usize) -> (Vec<serde_json::Value>, Vec<String>) {
    let (mut pages, mut ids, mut cursor) = (Vec::new(), Vec::new(), None);
    for _ in 0..=DISCUSSIONS.len() {
        let answer = read_discussion_list(
            repository,
            policy(),
            DiscussionListQuery {
                parent: None,
                allowed_parent_kinds: kinds(),
                status: DiscussionStatusFilter::All,
                limit,
                cursor: cursor.clone(),
            },
        )
        .await
        .unwrap();
        let mut page = serde_json::to_value(&answer.result).unwrap();
        for entry in page["entries"].as_array().unwrap() {
            ids.push(entry["discussion_id"].as_str().unwrap().to_owned());
        }
        cursor = page["next_cursor"].as_str().map(str::to_owned);
        page["next_cursor"] = serde_json::Value::from(cursor.is_some());
        pages.push(page);
        if cursor.is_none() {
            break;
        }
    }
    (pages, ids)
}

#[tokio::test]
async fn a_list_in_one_statement_a_kind_gives_the_pages_of_one_statement() {
    let store = TestStore::pinned();
    catch_up_state(&store.repository()).await.unwrap();
    seed(&store).await;
    let all: Vec<String> = DISCUSSIONS
        .iter()
        .map(|(id, ..)| (*id).to_owned())
        .collect();
    for limit in [1, 2, 4, 50] {
        let (file, file_ids) = pages(&store.repository(), limit).await;
        let (chunked, chunked_ids) = pages(&one_value_lists(&store), limit).await;
        assert_eq!(file_ids, all, "limit {limit}");
        assert_eq!(chunked_ids, all, "limit {limit}");
        assert_eq!(chunked, file, "limit {limit}");
    }
}

#[tokio::test]
async fn a_conversation_finds_its_discussion_in_a_later_statement() {
    let store = TestStore::pinned();
    catch_up_state(&store.repository()).await.unwrap();
    seed(&store).await;
    // The kinds go in byte order, one a statement: question, requirement,
    // rule. A rule's discussion is found by the last statement.
    for (id, ..) in DISCUSSIONS {
        let query = || DiscussionConversationQuery {
            discussion_id: StableId::new(id).unwrap(),
            allowed_parent_kinds: kinds(),
            limit: 10,
            cursor: None,
        };
        let file = read_discussion_conversation(&store.repository(), policy(), query())
            .await
            .unwrap_or_else(|failure| panic!("the file read finds {id}: {failure:#}"));
        let file = serde_json::to_value(&file.result).unwrap();
        assert_eq!(file["head"]["discussion_id"], id, "{id}");
        let chunked = read_discussion_conversation(&one_value_lists(&store), policy(), query())
            .await
            .unwrap_or_else(|failure| panic!("the chunked read finds {id}: {failure:#}"));
        assert_eq!(serde_json::to_value(&chunked.result).unwrap(), file, "{id}");
    }
}
