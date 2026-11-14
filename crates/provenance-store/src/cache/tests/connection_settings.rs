//! The `SQLite` settings of each connection that the store opens to the
//! cache database.

use super::super::*;
use super::fixtures::{open, seeded_layout};

/// The values that `PRAGMA synchronous`, `PRAGMA temp_store` and
/// `PRAGMA cache_size` give for FULL, MEMORY and a page cache of
/// 16,000 KiB.
pub const CACHE_SETTINGS: (i64, i64, i64) = (2, 2, -16_000);

/// The values of the same three settings with the `SQLite` defaults: FULL,
/// DEFAULT and a page cache of 2,000 KiB.
const SQLITE_DEFAULTS: (i64, i64, i64) = (2, 0, -2_000);

/// Reads the three settings from the one connection of the pool, then
/// completes the connection.
pub async fn settings_of(connection: CacheConnection) -> (i64, i64, i64) {
    let pool = connection.pool();
    let read = |pragma: &'static str| sqlx::query_scalar::<_, i64>(pragma).fetch_one(pool);
    let settings = (
        read("PRAGMA synchronous").await.unwrap(),
        read("PRAGMA temp_store").await.unwrap(),
        read("PRAGMA cache_size").await.unwrap(),
    );
    connection.close().await.unwrap();
    settings
}

/// An implementation aid with no Rule binding: the settings tune the cache
/// connection and have no public surface, so no behavioural obligation
/// states them. The test opens the cache through each open function of the
/// store and reads the settings from the connection that the open gives.
/// The stored-read open keeps the `SQLite` defaults, so that its first read
/// of the file is its probe and not a read of the schema inside the open.
#[tokio::test]
async fn each_open_but_the_stored_read_sets_the_cache_settings() {
    let (_dir, layout, _scope) = seeded_layout();
    catch_up_state(&open(&layout)).await.unwrap();
    let found = vec![
        (
            "writable",
            settings_of(open_cache(&layout).await.unwrap()).await,
        ),
        (
            "writable, existing file",
            settings_of(open_existing_cache(&layout).await.unwrap()).await,
        ),
        (
            "stored read, journal mode of the file",
            settings_of(open_stored_cache(&layout).await.unwrap()).await,
        ),
        (
            "immutable image",
            settings_of(open_immutable_cache(&layout).await.unwrap()).await,
        ),
    ];
    let expected = vec![
        ("writable", CACHE_SETTINGS),
        ("writable, existing file", CACHE_SETTINGS),
        ("stored read, journal mode of the file", SQLITE_DEFAULTS),
        ("immutable image", CACHE_SETTINGS),
    ];
    assert_eq!(found, expected);
}
