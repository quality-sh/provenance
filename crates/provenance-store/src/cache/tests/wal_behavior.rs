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

/// A database still in DELETE mode, held open by a reader on another
/// connection, must still open. On the `SQLite` build observed here the
/// busy timeout waits for the reader before the switch to WAL; a build
/// that refuses the switch at once is covered by the bounded retry in
/// `open_cache`. Either way the second opener sees WAL.
#[tokio::test]
async fn a_second_opener_survives_the_wal_switch() {
    let (_dir, layout, _scope) = empty_layout();
    std::fs::create_dir_all(layout.cache_dir()).unwrap();
    let url = format!("sqlite://{}", layout.cache_db_path());
    let delete_mode = SqliteConnectOptions::from_str(&url)
        .unwrap()
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Delete);
    let mut holder = SqliteConnection::connect_with(&delete_mode).await.unwrap();
    sqlx::query("CREATE TABLE probe (x INTEGER)")
        .execute(&mut holder)
        .await
        .unwrap();
    holder.close().await.unwrap();

    // The reader holds a shared lock for a while on its own thread.
    let reader_url = url.clone();
    let reader = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let options = SqliteConnectOptions::from_str(&reader_url)
                .unwrap()
                .journal_mode(SqliteJournalMode::Delete);
            let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
            let mut tx = connection.begin().await.unwrap();
            sqlx::query("SELECT count(*) FROM probe")
                .execute(&mut *tx)
                .await
                .unwrap();
            std::thread::sleep(Duration::from_millis(400));
            tx.commit().await.unwrap();
            connection.close().await.unwrap();
        });
    });
    std::thread::sleep(Duration::from_millis(50));

    let pool = open_cache(&layout).await.unwrap();
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(mode, "wal");
    reader.join().unwrap();
}
