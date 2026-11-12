//! Cases on the order of reads and writes: a read pinned to one committed
//! state, and a write followed by a read.

use super::{fetch, integers, statement, write, Harness, WorkFailed};
use crate::read_database::{ReadDatabase, ReadWork, SqlValue, WriteWork};
use std::future::Future;
use std::pin::pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::Poll;

/// Runs both futures until both end. Each poll polls `first` before
/// `second`.
async fn both<A, B>(first: impl Future<Output = A>, second: impl Future<Output = B>) -> (A, B) {
    let (mut first, mut second) = (pin!(first), pin!(second));
    let (mut a, mut b) = (None, None);
    std::future::poll_fn(|context| {
        if a.is_none() {
            if let Poll::Ready(value) = first.as_mut().poll(context) {
                a = Some(value);
            }
        }
        if b.is_none() {
            if let Poll::Ready(value) = second.as_mut().poll(context) {
                b = Some(value);
            }
        }
        match (a.take(), b.take()) {
            (Some(a), Some(b)) => Poll::Ready((a, b)),
            (left, right) => {
                (a, b) = (left, right);
                Poll::Pending
            }
        }
    })
    .await
}

/// Ends when `flag` is on.
async fn until(flag: &AtomicBool) {
    std::future::poll_fn(|context| {
        if flag.load(Ordering::SeqCst) {
            Poll::Ready(())
        } else {
            context.waker().wake_by_ref();
            Poll::Pending
        }
    })
    .await;
}

/// Every row that a read gets is from the state that was committed when
/// its first statement ran.
///
/// A write commits while the read runs. It runs beside the read, never
/// inside its work: it starts after the first statement of the read, and
/// the read goes on until the write has committed, or for a fixed number
/// of statements. When the harness says that reads interleave, the write
/// must commit while the read is open. A store that runs a read to its
/// end in one call runs the write after it.
pub async fn a_read_answers_from_one_committed_state<H: Harness>(harness: H) {
    const MOST_STATEMENTS: usize = 1000;
    let database = harness.database();
    write(
        database,
        vec![
            statement("CREATE TABLE pinned (n INTEGER)", Vec::new()),
            statement("INSERT INTO pinned (n) VALUES (1)", Vec::new()),
        ],
    )
    .await
    .unwrap();
    let (first_read, committed) = (AtomicBool::new(false), AtomicBool::new(false));
    let (first_read, committed) = (&first_read, &committed);
    let mut seen = Vec::new();
    let slot = &mut seen;
    let read = database.read(Box::new(move |mut transaction| {
        Box::pin(async move {
            let sql = "SELECT n FROM pinned ORDER BY n";
            slot.push(integers(&transaction.fetch(sql, Vec::new()).await?));
            first_read.store(true, Ordering::SeqCst);
            for _ in 0..MOST_STATEMENTS {
                if committed.load(Ordering::SeqCst) {
                    break;
                }
                slot.push(integers(&transaction.fetch(sql, Vec::new()).await?));
            }
            slot.push(integers(&transaction.fetch(sql, Vec::new()).await?));
            Ok(())
        })
    }));
    let later = async {
        until(first_read).await;
        let written = write(
            database,
            vec![statement("INSERT INTO pinned (n) VALUES (2)", Vec::new())],
        )
        .await;
        committed.store(true, Ordering::SeqCst);
        written
    };
    let (read, written) = both(read, later).await;
    read.unwrap();
    written.unwrap();
    assert!(seen.iter().all(|rows| rows == &[1]), "{seen:?}");
    if harness.reads_interleave() {
        assert!(
            seen.len() < MOST_STATEMENTS + 2,
            "the write did not commit while the read was open"
        );
    }
    assert_eq!(
        integers(&fetch(database, "SELECT n FROM pinned ORDER BY n").await),
        vec![1, 2]
    );
}

/// A write that inserts `n` into `sequenced` and commits.
fn insert<'a>(n: i64) -> WriteWork<'a> {
    Box::new(move |mut transaction| {
        Box::pin(async move {
            transaction
                .execute(
                    "INSERT INTO sequenced (n) VALUES (?)",
                    vec![SqlValue::Integer(n)],
                )
                .await?;
            transaction.commit().await
        })
    })
}

/// A read that puts the rows of `sequenced` in `slot`.
fn read_into(slot: &mut Option<Vec<i64>>) -> ReadWork<'_> {
    Box::new(move |mut transaction| {
        Box::pin(async move {
            let rows = transaction
                .fetch("SELECT n FROM sequenced ORDER BY n", Vec::new())
                .await?;
            *slot = Some(integers(&rows));
            Ok(())
        })
    })
}

async fn sequenced(database: &dyn ReadDatabase) {
    write(
        database,
        vec![statement("CREATE TABLE sequenced (n INTEGER)", Vec::new())],
    )
    .await
    .unwrap();
}

/// The read of `write_then_read` sees what the write committed.
pub async fn a_write_then_read_reads_its_write<H: Harness>(harness: H) {
    let database = harness.database();
    sequenced(database).await;
    let mut seen = None;
    database
        .write_then_read(insert(1), read_into(&mut seen))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(seen, Some(vec![1]));
}

/// A write of `write_then_read` that fails stops the read.
///
/// The outer error is the error of the write, the read does not run, and
/// the write keeps nothing.
pub async fn a_failed_write_stops_the_read<H: Harness>(harness: H) {
    let database = harness.database();
    sequenced(database).await;
    let mut seen = None;
    let failing: WriteWork<'_> = Box::new(|mut transaction| {
        Box::pin(async move {
            transaction
                .execute("INSERT INTO sequenced (n) VALUES (1)", Vec::new())
                .await?;
            Err(WorkFailed.into())
        })
    });
    let failed = database
        .write_then_read(failing, read_into(&mut seen))
        .await
        .unwrap_err();
    assert!(failed.downcast_ref::<WorkFailed>().is_some(), "{failed:#}");
    assert_eq!(seen, None);
    assert_eq!(
        integers(&fetch(database, "SELECT n FROM sequenced").await),
        Vec::<i64>::new()
    );
}

/// A write whose commit fails keeps nothing, and fails.
///
/// Serial 1 is kept. A work writes the next serial and commits, and the
/// harness makes that commit fail. [`ReadDatabase::write`] then fails, and
/// [`ReadDatabase::write_then_read`] fails with the outer error and does
/// not run its read. A later read finds serial 1 only, so a caller that
/// stamps a read with the serial it finds never stamps one that was not
/// kept.
pub async fn a_write_whose_commit_fails_keeps_nothing<H: Harness + Sync>(harness: H) {
    let database = harness.database();
    sequenced(database).await;
    database.write(insert(1)).await.unwrap();
    let failing = |n: i64| -> WriteWork<'_> {
        let harness = &harness;
        Box::new(move |mut transaction| {
            Box::pin(async move {
                transaction
                    .execute(
                        "INSERT INTO sequenced (n) VALUES (?)",
                        vec![SqlValue::Integer(n)],
                    )
                    .await?;
                harness.fail_commit(&mut transaction).await?;
                transaction.commit().await
            })
        })
    };
    database.write(failing(2)).await.unwrap_err();
    let mut seen = None;
    database
        .write_then_read(failing(2), read_into(&mut seen))
        .await
        .unwrap_err();
    assert_eq!(seen, None, "the read ran after a write that failed");
    assert_eq!(
        integers(&fetch(database, "SELECT n FROM sequenced ORDER BY n").await),
        vec![1]
    );
}

/// A read of `write_then_read` that fails keeps the write.
///
/// The inner error is the error of the read, and the write stays committed.
pub async fn a_failed_read_keeps_the_write<H: Harness>(harness: H) {
    let database = harness.database();
    sequenced(database).await;
    let failing: ReadWork<'_> = Box::new(|_| Box::pin(async { Err(WorkFailed.into()) }));
    let failed = database
        .write_then_read(insert(2), failing)
        .await
        .unwrap()
        .unwrap_err();
    assert!(failed.downcast_ref::<WorkFailed>().is_some(), "{failed:#}");
    assert_eq!(
        integers(&fetch(database, "SELECT n FROM sequenced").await),
        vec![2]
    );
}
