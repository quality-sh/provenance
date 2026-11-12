//! A repository whose read database is not the file. Until the row readers
//! run on the port, such a read refuses with a fault and does not panic in
//! a reader. No Rule: the limit ends with the pull request that moves the
//! readers.

use super::super::comparison::test_stores::{self, TestStore};
use super::get_through;
use crate::operations::read_policy::{FreshnessPolicy, ReadPolicy};
use crate::read_database::file::FileReadDatabase;
use crate::read_database::{
    ReadDatabase, ReadWork, SqlFuture, SqlRow, SqlTransaction, SqlValue, Statement, Transaction,
    WriteWork,
};
use crate::repository::{Repository, RepositoryIdentity};
use crate::settings::Settings;
use crate::storage::local::{LocalSectionLock, LocalStateStorage};
use std::sync::Arc;

/// Another adapter over the same file: it runs the same SQL, but its
/// transaction is its own.
#[derive(Debug)]
struct Other(FileReadDatabase);

struct OtherTransaction(Transaction);

impl SqlTransaction for OtherTransaction {
    fn fetch<'a>(&'a mut self, sql: &'a str, values: &'a [SqlValue]) -> SqlFuture<'a, Vec<SqlRow>> {
        self.0.fetch(sql, values)
    }

    fn execute<'a>(&'a mut self, sql: &'a str, values: &'a [SqlValue]) -> SqlFuture<'a, u64> {
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
}

impl ReadDatabase for Other {
    fn read<'a>(&'a self, work: ReadWork<'a>) -> SqlFuture<'a, ()> {
        self.0
            .read(Box::new(move |lent| work(Box::new(OtherTransaction(lent)))))
    }

    fn write<'a>(&'a self, work: WriteWork<'a>) -> SqlFuture<'a, ()> {
        self.0
            .write(Box::new(move |lent| work(Box::new(OtherTransaction(lent)))))
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

/// The repository of `store` over the other adapter.
fn over_the_other_adapter(store: &TestStore) -> Repository {
    let layout = store.layout();
    Repository::with_storage(
        RepositoryIdentity::new(store.root.clone(), store.scope.clone(), Settings::default()),
        Arc::new(LocalStateStorage::new(layout.clone())),
        Arc::new(LocalSectionLock::new(layout.clone())),
        Some(Arc::new(Other(FileReadDatabase::new(layout)))),
    )
    .unwrap()
}

#[tokio::test]
async fn a_read_of_another_adapter_refuses_with_a_fault() {
    let store = test_stores::seeded_queries();
    let stored = get_through(&store, ReadPolicy::default()).await.unwrap();
    let repository = over_the_other_adapter(&store);
    let read = |policy| {
        let repository = repository.clone();
        async move {
            crate::operations::reader::answer(
                &repository,
                ReadPolicy::with_freshness(policy),
                |ctx| {
                    Box::pin(async move {
                        crate::operations::queries::records::get(
                            ctx,
                            super::get_query("req_overtime"),
                        )
                        .await
                    })
                },
            )
            .await
        }
    };

    // Under catch-up, the pass refuses to write the other adapter, and the
    // read at the stored serial refuses before a reader runs.
    for policy in [
        FreshnessPolicy::CatchUp,
        FreshnessPolicy::AnnotateOnly,
        FreshnessPolicy::RefuseStale,
    ] {
        let failed = read(policy).await.unwrap_err();
        assert_eq!(
            format!("{failed:#}"),
            "the row readers read only the read database file under .provenance/cache",
            "{policy:?}"
        );
        assert!(
            failed.downcast_ref::<crate::refusal::Fault>().is_some(),
            "{policy:?}"
        );
    }

    // The file keeps the revision that it had.
    let after = get_through(
        &store,
        ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly),
    )
    .await
    .unwrap();
    assert_eq!(after.stamp.serial, stored.stamp.serial);
}
