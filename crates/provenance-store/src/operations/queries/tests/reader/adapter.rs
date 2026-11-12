//! A repository whose read database is not the file.
//!
//! The catch-up, the schema check and every reader run on the port, so a
//! read over another adapter answers as the file's does under every policy.
//! No Rule: these hold the move of the read database onto its port.

use super::super::comparison::test_stores::{self, TestStore};
use super::get_through;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::read_database::contract::{Other, OtherTransaction};
use crate::read_database::file::FileReadDatabase;
use crate::read_database::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};
use crate::repository::{Repository, RepositoryIdentity};
use crate::settings::Settings;
use crate::storage::local::{LocalSectionLock, LocalStateStorage};
use provenance_core::protocol::StampPolicy;
use provenance_core::threads::{DiscussionListQuery, DiscussionStatusFilter};
use provenance_core::{NodeType, RequirementStatus};
use std::sync::Arc;

/// The repository of `store` over `database`.
fn over(store: &TestStore, database: Option<Arc<dyn ReadDatabase>>) -> Repository {
    let layout = store.layout();
    Repository::with_storage(
        RepositoryIdentity::new(store.root.clone(), store.scope.clone(), Settings::default()),
        Arc::new(LocalStateStorage::new(layout.clone())),
        Arc::new(LocalSectionLock::new(layout)),
        database,
    )
    .unwrap()
}

/// The repository of `store` over the other adapter.
fn over_the_other_adapter(store: &TestStore) -> Repository {
    over(
        store,
        Some(Arc::new(Other(FileReadDatabase::new(store.layout())))),
    )
}

fn assert_fault(failed: &anyhow::Error, text: &str) {
    assert_eq!(format!("{failed:#}"), text);
    assert!(
        failed.downcast_ref::<crate::refusal::Fault>().is_some(),
        "not a fault: {failed:#}"
    );
}

async fn get(
    repository: &Repository,
    policy: FreshnessPolicy,
) -> anyhow::Result<provenance_core::protocol::Stamped<provenance_core::protocol::GetResult>>
{
    crate::operations::reader::answer(repository, ReadPolicy::with_freshness(policy), |ctx| {
        Box::pin(async move {
            crate::operations::queries::records::get(ctx, super::get_query("req_overtime")).await
        })
    })
    .await
}

fn discussions() -> DiscussionListQuery {
    DiscussionListQuery {
        parent: None,
        allowed_parent_kinds: vec![NodeType::Requirement],
        status: DiscussionStatusFilter::All,
        limit: 10,
        cursor: None,
    }
}

fn add_requirement(store: &TestStore, id: &str) {
    crate::cache::tests::fixtures::create_requirement(
        &store.state_store(),
        &store.scope,
        id,
        RequirementStatus::Active,
    );
}

/// From no database, a read over the other adapter catches up, and then
/// answers under each policy as the file does. The discussion reads
/// answer too.
#[tokio::test]
async fn a_read_over_another_adapter_answers_under_every_policy() {
    let store = test_stores::seeded_queries();
    let repository = over_the_other_adapter(&store);
    let caught = get(&repository, FreshnessPolicy::CatchUp).await.unwrap();
    assert_eq!(caught.stamp.policy, StampPolicy::CatchUp);
    assert!(caught.freshness_error.is_none());
    for (policy, word) in [
        (FreshnessPolicy::AnnotateOnly, StampPolicy::AnnotateOnly),
        (FreshnessPolicy::RefuseStale, StampPolicy::RefuseStale),
        (FreshnessPolicy::CatchUp, StampPolicy::CatchUp),
    ] {
        let other = get(&repository, policy).await.unwrap();
        let file = get_through(&store, ReadPolicy::with_freshness(policy))
            .await
            .unwrap();
        assert_eq!(other.stamp.policy, word);
        assert_eq!(other.stamp.serial, caught.stamp.serial);
        assert_eq!(
            serde_json::to_value(&other.result).unwrap(),
            serde_json::to_value(&file.result).unwrap()
        );
    }
    add_requirement(&store, "req_after_the_pass");
    let refused = get(&repository, FreshnessPolicy::RefuseStale)
        .await
        .unwrap_err();
    assert!(refused.to_string().contains("; moved: "), "{refused}");
    let moved = get(&repository, FreshnessPolicy::CatchUp).await.unwrap();
    assert_eq!(moved.stamp.serial, caught.stamp.serial + 1);
    let other = crate::review::read_discussion_list(&repository, ReadPolicy::default(), discussions())
        .await
        .unwrap();
    let file =
        crate::review::read_discussion_list(&store.repository(), ReadPolicy::default(), discussions())
            .await
            .unwrap();
    assert_eq!(
        serde_json::to_value(&other.result).unwrap(),
        serde_json::to_value(&file.result).unwrap()
    );
}

/// The rebuild and the catch-up pass write another adapter as they write
/// the file.
#[tokio::test]
async fn materialize_and_catch_up_write_another_adapter() {
    let store = test_stores::seeded_queries();
    let repository = over_the_other_adapter(&store);
    let rebuilt = crate::cache::materialize_state(&repository).await.unwrap();
    assert!(rebuilt.cache_recreated);
    assert!(rebuilt.records_loaded > 0);
    let unchanged = crate::cache::catch_up_state(&repository).await.unwrap();
    assert_eq!((unchanged.serial, unchanged.revision_committed), (1, false));
    add_requirement(&store, "req_after_the_rebuild");
    let caught = crate::cache::catch_up_state(&repository).await.unwrap();
    assert_eq!((caught.serial, caught.revision_committed), (2, true));
    let file = crate::cache::catch_up_state(&store.repository()).await.unwrap();
    assert_eq!(
        (file.serial, file.digest, file.revision_committed),
        (caught.serial, caught.digest, false)
    );
}

/// A repository built with no read database refuses each read, under each
/// policy, with a fault, and so do the rebuild and the catch-up.
#[tokio::test]
async fn a_repository_with_no_read_database_refuses_a_read() {
    let store = test_stores::seeded_queries();
    let repository = over(&store, None);
    for policy in [
        FreshnessPolicy::CatchUp,
        FreshnessPolicy::AnnotateOnly,
        FreshnessPolicy::RefuseStale,
    ] {
        let failed = get(&repository, policy).await.unwrap_err();
        assert_fault(&failed, "this repository has no read database");
    }
    let failed = crate::cache::materialize_state(&repository)
        .await
        .unwrap_err();
    assert_fault(&failed, "this repository has no read database");
    let failed = crate::cache::catch_up_state(&repository).await.unwrap_err();
    assert_fault(&failed, "this repository has no read database");
    assert!(!store.layout().cache_db_path().exists());
}

/// A store whose commit is durable only after the work ends, and fails
/// there: the work runs and gives its report, nothing is kept, and the
/// write fails.
#[derive(Debug)]
struct LateCommitFails(FileReadDatabase);

/// A transaction whose commit keeps nothing: the transaction rolls back
/// when it drops.
struct Uncommitted(Transaction);

impl SqlTransaction for Uncommitted {
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
        drop(self);
        Box::pin(async { Ok(()) })
    }

    fn bound_work(&mut self, steps: u64) -> SqlFuture<'_, ()> {
        self.0.bound_work(steps)
    }

    fn max_list_values(&self) -> usize {
        self.0.max_list_values()
    }
}

impl ReadDatabase for LateCommitFails {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        self.0
            .read(Box::new(move |lent| work(Box::new(OtherTransaction(lent)))))
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        Box::pin(async move {
            self.0
                .write(Box::new(move |lent| work(Box::new(Uncommitted(lent)))))
                .await?;
            Err(crate::refusal::fault!("the store did not commit the write"))
        })
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

/// A catch-up pass whose commit fails after its work gave its report is a
/// failed freshness step: the read answers at the stored serial with
/// `catch_up_failed` and the commit error, as for any failed pass.
#[tokio::test]
async fn a_commit_that_fails_after_the_report_is_a_failed_catch_up() {
    let store = test_stores::seeded_queries();
    let stored = get_through(&store, ReadPolicy::default()).await.unwrap();
    add_requirement(&store, "req_after_the_read");
    let repository = over(
        &store,
        Some(Arc::new(LateCommitFails(FileReadDatabase::new(store.layout())))),
    );
    let read = get(&repository, FreshnessPolicy::CatchUp).await.unwrap();
    assert_eq!(read.stamp.policy, StampPolicy::CatchUpFailed);
    assert_eq!(
        (read.stamp.serial, read.stamp.digest),
        (stored.stamp.serial, stored.stamp.digest)
    );
    assert_eq!(
        read.freshness_error.as_deref(),
        Some("the store did not commit the write")
    );
    let failed = crate::cache::catch_up_state(&repository).await.unwrap_err();
    assert_fault(&failed, "the store did not commit the write");
}

/// A transaction of the file that allows no values in a list.
struct NoList(crate::read_database::Transaction);

impl crate::read_database::SqlTransaction for NoList {
    fn fetch<'a>(
        &'a mut self,
        sql: &'a str,
        values: Vec<crate::read_database::SqlValue>,
    ) -> crate::read_database::SqlFuture<'a, Vec<crate::read_database::SqlRow>> {
        self.0.fetch(sql, values)
    }

    fn execute<'a>(
        &'a mut self,
        sql: &'a str,
        values: Vec<crate::read_database::SqlValue>,
    ) -> crate::read_database::SqlFuture<'a, u64> {
        self.0.execute(sql, values)
    }

    fn attempt<'a>(
        &'a mut self,
        statements: &'a [crate::read_database::Statement],
    ) -> crate::read_database::SqlFuture<'a, Result<(), anyhow::Error>> {
        self.0.attempt(statements)
    }

    fn commit(self: Box<Self>) -> crate::read_database::SqlFuture<'static, ()> {
        self.0.commit()
    }

    fn bound_work(&mut self, steps: u64) -> crate::read_database::SqlFuture<'_, ()> {
        self.0.bound_work(steps)
    }

    fn max_list_values(&self) -> usize {
        0
    }
}

/// An adapter that allows no values in a list is refused with a fault when
/// the snapshot opens, and does not panic at its first list.
#[tokio::test]
async fn an_adapter_that_allows_no_values_in_a_list_is_refused() {
    let store = test_stores::seeded_queries();
    get_through(&store, ReadPolicy::default()).await.unwrap();
    let cache = crate::cache::open_cache(&store.layout()).await.unwrap();
    let transaction = crate::read_database::file::begin(cache.pool())
        .await
        .unwrap();
    let opened =
        crate::operations::reader::ReadSnapshot::open(Box::new(NoList(transaction)), &store.scope)
            .await;
    let Err(failed) = opened else {
        panic!("the snapshot opened");
    };
    cache.close().await.unwrap();
    assert_fault(
        &failed,
        "the read database adapter allows no values in a list",
    );
}
