//! The cache pool runs `SQLite` in WAL mode, so a read transaction pins a
//! snapshot without blocking a writer and a writer never blocks a reader.

use super::super::*;
use super::fixtures::*;
use crate::layout::ProvenanceLayout;
use provenance_core::protocol::{GetQuery, SDK_PROTOCOL_VERSION};
use provenance_core::NodeType;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode};
use sqlx::{Connection, SqliteConnection};
use std::str::FromStr;
use std::time::Duration;

/// The `-wal` and `-shm` files `SQLite` keeps beside an open WAL database
/// and removes when the last connection closes.
pub fn sidecars(layout: &ProvenanceLayout) -> Vec<String> {
    let database = layout.cache_db_path();
    [format!("{database}-wal"), format!("{database}-shm")]
        .into_iter()
        .filter(|path| std::path::Path::new(path).exists())
        .collect()
}

#[tokio::test]
async fn the_cache_pool_runs_in_wal_mode() {
    let (_dir, layout, _scope) = empty_layout();
    let pool = open_cache(&layout).await.unwrap();
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mode, "wal");
    pool.close().await;
    assert_eq!(sidecars(&layout), Vec::<String>::new());
    let pool = open_cache(&layout).await.unwrap();
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mode, "wal", "the mode persists in the file");
    pool.close().await;
    assert_eq!(sidecars(&layout), Vec::<String>::new());
}

/// A read opens one pool and closes it. The pool holds one connection, so
/// the close is one `sqlite3_close`, which takes the exclusive lock and
/// removes the sidecar files. A pool that grew to two connections closed
/// them on two worker threads at once, and the second closer could not
/// take the lock, so about half of all reads left the files behind.
#[tokio::test]
async fn a_completed_read_leaves_no_sidecar_files() {
    let (_dir, layout, scope) = seeded_layout();
    let root = Some(layout.root().to_path_buf());
    for round in 0..20 {
        let answer = crate::operations::queries::get(
            root.clone(),
            &scope,
            GetQuery {
                protocol_version: Some(SDK_PROTOCOL_VERSION),
                node_type: NodeType::Requirement,
                id: "req_schads_overtime".into(),
                include_retired: false,
            },
        )
        .await
        .unwrap();
        assert!(answer.result.found);
        assert_eq!(
            sidecars(&layout),
            Vec::<String>::new(),
            "read {round} left sidecar files behind"
        );
    }
}

/// Holds a DELETE-mode read transaction on the database named by
/// `PROVENANCE_TEST_HOLD_DB` for `PROVENANCE_TEST_HOLD_MS`, then commits.
/// The WAL-switch test runs this test binary as a second process with
/// those variables set; without them the test does nothing.
#[tokio::test]
async fn hold_a_delete_mode_read_lock_for_a_parent_process() {
    let Ok(path) = std::env::var("PROVENANCE_TEST_HOLD_DB") else {
        return;
    };
    let millis: u64 = std::env::var("PROVENANCE_TEST_HOLD_MS")
        .unwrap()
        .parse()
        .unwrap();
    let options = SqliteConnectOptions::from_str(&format!("sqlite://{path}"))
        .unwrap()
        .journal_mode(SqliteJournalMode::Delete);
    let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
    let mut tx = connection.begin().await.unwrap();
    sqlx::query("SELECT count(*) FROM probe")
        .execute(&mut *tx)
        .await
        .unwrap();
    println!("holding");
    std::thread::sleep(Duration::from_millis(millis));
    tx.commit().await.unwrap();
    connection.close().await.unwrap();
}

/// Runs this test binary as a second process that holds a DELETE-mode
/// read lock on `path` for `millis`, and returns once the child says it
/// holds the lock.
fn second_process_holding(path: &str, millis: u64) -> std::process::Child {
    use std::io::{BufRead, BufReader};
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "cache::tests::wal_behavior::hold_a_delete_mode_read_lock_for_a_parent_process",
            "--exact",
            "--nocapture",
        ])
        .env("PROVENANCE_TEST_HOLD_DB", path)
        .env("PROVENANCE_TEST_HOLD_MS", millis.to_string())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut lines = BufReader::new(stdout).lines();
    let holding = lines
        .by_ref()
        .map_while(Result::ok)
        .any(|line| line.trim() == "holding");
    assert!(
        holding,
        "the second process must take the lock before the test goes on"
    );
    std::thread::spawn(move || for _ in lines {});
    child
}

/// Switching a DELETE-mode file to WAL needs an exclusive lock. A second
/// process holding a read transaction longer than the busy timeout makes
/// the first connect fail as busy; the retry gets through once the holder
/// commits, and a retry with no deadline does not.
#[tokio::test]
async fn a_second_opener_survives_the_wal_switch() {
    let (_dir, layout, _scope) = empty_layout();
    std::fs::create_dir_all(layout.cache_dir()).unwrap();
    let path = layout.cache_db_path().to_string();
    let delete_mode = SqliteConnectOptions::from_str(&format!("sqlite://{path}"))
        .unwrap()
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Delete);
    let mut holder = SqliteConnection::connect_with(&delete_mode).await.unwrap();
    sqlx::query("CREATE TABLE probe (x INTEGER)")
        .execute(&mut holder)
        .await
        .unwrap();
    holder.close().await.unwrap();

    let short_wait = Duration::from_millis(50);
    let mut child = second_process_holding(&path, 600);
    let refused = open_cache_with(
        &layout,
        WalSwitchRetry {
            busy_timeout: short_wait,
            pause: short_wait,
            deadline: Duration::ZERO,
        },
    )
    .await;
    assert!(
        refused.is_err(),
        "with no deadline the busy switch must surface"
    );
    let pool = open_cache_with(
        &layout,
        WalSwitchRetry {
            busy_timeout: short_wait,
            pause: short_wait,
            deadline: Duration::from_secs(10),
        },
    )
    .await
    .unwrap();
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mode, "wal");
    pool.close().await;
    assert!(child.wait().unwrap().success());
}
