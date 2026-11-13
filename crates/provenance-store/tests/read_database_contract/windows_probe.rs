//! TEMPORARY diagnostic for the Windows failure of
//! a_read_answers_from_one_committed_state. Removed before the PR is ready.

use super::File;
use provenance_store::read_database::{ReadDatabase, SqlValue};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

fn ms(start: Instant, at: Instant) -> f64 {
    at.duration_since(start).as_secs_f64() * 1000.0
}

async fn write_rows(database: &dyn ReadDatabase, sql: &'static [&'static str]) {
    database
        .write(Box::new(move |mut transaction| {
            Box::pin(async move {
                for sql in sql {
                    transaction.execute(sql, Vec::new()).await?;
                }
                transaction.commit().await
            })
        }))
        .await
        .unwrap();
}

/// One run: a read that runs statements until the write returns or until
/// `most` statements or `wall` time, and a write beside it.
async fn run(most: usize, wall: Duration, report: &mut String, label: &str) {
    let file = File::default();
    let database: &dyn ReadDatabase = &file.database;
    write_rows(
        database,
        &[
            "CREATE TABLE pinned (n INTEGER)",
            "INSERT INTO pinned (n) VALUES (1)",
        ],
    )
    .await;
    let start = Instant::now();
    let statements = AtomicUsize::new(0);
    let first = AtomicBool::new(false);
    let done = AtomicBool::new(false);
    let phases: Mutex<Vec<(String, f64, usize)>> = Mutex::new(Vec::new());
    let mark = |name: &str| {
        phases.lock().unwrap().push((
            name.to_owned(),
            ms(start, Instant::now()),
            statements.load(Ordering::SeqCst),
        ));
    };
    let (statements, first, done, mark) = (&statements, &first, &done, &mark);
    let read = database.read(Box::new(move |mut transaction| {
        Box::pin(async move {
            mark("read work entered");
            let sql = "SELECT n FROM pinned ORDER BY n";
            transaction.fetch(sql, Vec::new()).await?;
            statements.fetch_add(1, Ordering::SeqCst);
            first.store(true, Ordering::SeqCst);
            mark("read first statement");
            let began = Instant::now();
            let mut seen_two = false;
            while statements.load(Ordering::SeqCst) < most
                && began.elapsed() < wall
                && !done.load(Ordering::SeqCst)
            {
                let rows = transaction.fetch(sql, Vec::new()).await?;
                if rows.len() != 1 {
                    seen_two = true;
                }
                statements.fetch_add(1, Ordering::SeqCst);
            }
            mark(if seen_two {
                "read loop end SAW TWO ROWS"
            } else {
                "read loop end"
            });
            Ok(())
        })
    }));
    let later = async {
        while !first.load(Ordering::SeqCst) {
            tokio::task::yield_now().await;
        }
        mark("write start");
        database
            .write(Box::new(move |mut transaction| {
                Box::pin(async move {
                    mark("write work entered (opened, begun)");
                    transaction
                        .execute("INSERT INTO pinned (n) VALUES (2)", Vec::new())
                        .await?;
                    mark("write insert done");
                    let committed = transaction.commit().await;
                    mark("write commit returned");
                    committed
                })
            }))
            .await
            .unwrap();
        mark("write returned (closed)");
        done.store(true, Ordering::SeqCst);
    };
    let (read, ()) = tokio::join!(read, later);
    mark("read returned (closed)");
    read.unwrap();
    let _ = writeln!(report, "--- {label} most={most} wall={wall:?}");
    for (name, at, count) in phases.lock().unwrap().iter() {
        let _ = writeln!(report, "  {at:9.2} ms  statements={count:6}  {name}");
    }
}

/// A lone write, and a lone read of 1000 statements, for the base times.
async fn base(report: &mut String) {
    let file = File::default();
    let database: &dyn ReadDatabase = &file.database;
    write_rows(database, &["CREATE TABLE pinned (n INTEGER)"]).await;
    let start = Instant::now();
    write_rows(database, &["INSERT INTO pinned (n) VALUES (2)"]).await;
    let write = ms(start, Instant::now());
    let start = Instant::now();
    database
        .read(Box::new(|mut transaction| {
            Box::pin(async move {
                for _ in 0..1000 {
                    transaction.fetch("SELECT n FROM pinned", vec![]).await?;
                }
                Ok(())
            })
        }))
        .await
        .unwrap();
    let read = ms(start, Instant::now());
    let _ = writeln!(
        report,
        "--- base: lone write {write:.2} ms, lone read of 1000 statements {read:.2} ms"
    );
    let _ = SqlValue::Null;
}

#[tokio::test]
async fn windows_probe_read_and_write_timing() {
    let mut report = String::new();
    let _ = writeln!(report, "os={}", std::env::consts::OS);
    for _ in 0..3 {
        base(&mut report).await;
    }
    for round in 0..5 {
        run(
            1000,
            Duration::from_secs(60),
            &mut report,
            &format!("as the case, round {round}"),
        )
        .await;
    }
    run(
        usize::MAX,
        Duration::from_secs(30),
        &mut report,
        "unbounded statements for 30 s",
    )
    .await;
    panic!("PROBE REPORT\n{report}END PROBE REPORT");
}
