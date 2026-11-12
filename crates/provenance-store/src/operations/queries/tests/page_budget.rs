//! The work bound of a page. A statement past it stops with the page
//! budget refusal, at the same count as before the bound went through the
//! port, on the file and on another transaction type over the file.

use super::comparison::test_stores::TestStore;
use crate::cache::{catch_up_state, open_cache};
use crate::operations::reader::{page_error, ReadSnapshot};
use crate::read_database::contract::OtherTransaction;
use crate::read_database::SqlValue;
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_macros::verifies;
use sqlx::SqlitePool;

/// A query whose work grows with `n`: it counts from 1 to `n`.
const COUNT_TO: &str = "WITH RECURSIVE count_to (n) AS (SELECT 1 UNION ALL \
                        SELECT n + 1 FROM count_to WHERE n < ?) SELECT COUNT(*) FROM count_to";

/// The smallest count that the bound of one page stops, found by the
/// recorded run before the bound went through the port.
const FIRST_STOPPED_COUNT: i64 = 0;

/// Whether the bound of one page stops the count to `n`. Any other error
/// fails the test; the refusal is the page budget refusal.
async fn stops(pool: &SqlitePool, store: &TestStore, port_only: bool, n: i64) -> bool {
    let mut transaction = crate::read_database::file::begin(pool).await.unwrap();
    if port_only {
        transaction = Box::new(OtherTransaction(transaction));
    }
    let snapshot = ReadSnapshot::open(transaction, &store.scope)
        .await
        .unwrap()
        .expect("a revision");
    snapshot.bound_page_work().await.unwrap();
    match snapshot.fetch(COUNT_TO, vec![SqlValue::Integer(n)]).await {
        Ok(_) => false,
        Err(error) => {
            let error = page_error(error);
            assert_eq!(
                error.downcast_ref::<ReadFailure>(),
                Some(&ReadFailure::PageBudgetExceeded),
                "{error:#}"
            );
            assert_eq!(
                format!("{error:#}"),
                ReadFailure::PageBudgetExceeded.to_string()
            );
            true
        }
    }
}

/// The smallest count that the bound stops.
async fn first_stopped(pool: &SqlitePool, store: &TestStore, port_only: bool) -> i64 {
    let (mut low, mut high) = (1, 10_000_000);
    assert!(stops(pool, store, port_only, high).await);
    while low < high {
        let middle = low + (high - low) / 2;
        if stops(pool, store, port_only, middle).await {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[tokio::test]
#[verifies("rule_query_pages_bound_shared_reads", examples)]
async fn the_page_bound_stops_a_statement_at_the_same_count() {
    let store = TestStore::pinned();
    catch_up_state(&store.repository()).await.unwrap();
    let cache = open_cache(&store.layout()).await.unwrap();
    let file = first_stopped(cache.pool(), &store, false).await;
    let port_only = first_stopped(cache.pool(), &store, true).await;
    cache.close().await.unwrap();
    println!("=== FIRST STOPPED COUNT {file}");
    assert_eq!(
        (file, port_only),
        (FIRST_STOPPED_COUNT, FIRST_STOPPED_COUNT)
    );
}
