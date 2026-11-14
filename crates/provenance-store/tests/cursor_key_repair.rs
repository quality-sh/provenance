//! A paged read replaces a cursor key file of the cache that does not hold
//! a valid key, and a cursor that the lost key signed refuses as after a
//! delete of the cache.
mod discussion_support;
use camino::{Utf8Path, Utf8PathBuf};
use discussion_support::*;
use provenance_core::{
    protocol::{read_failure::ReadFailure, Stamped},
    threads::{Discussion, DiscussionMessagesPage, DiscussionMessagesQuery, DiscussionSelector},
};
use provenance_macros::verifies;
use provenance_store::{
    layout::ProvenanceLayout, operations::read_policy::ReadPolicy, review::read_discussion_messages,
};
use std::sync::{Arc, Barrier};

/// The key file in the cache of the repository at `root`.
fn key_path(root: &Utf8Path) -> Utf8PathBuf {
    ProvenanceLayout::new(root)
        .cache_dir()
        .join("read-cursor.key")
}

/// Writes `bytes` as the key file of the repository at `root`.
fn write_key_file(root: &Utf8Path, bytes: &[u8]) {
    let path = key_path(root);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

/// A Discussion with five Messages: three pages at a limit of two.
fn discussion(store: &provenance_store::state_store::StateStore) -> Discussion {
    let mut latest = start(store, "a");
    for n in 0..4 {
        latest = store
            .write_discussion(reply(&latest, &format!("reply_{n}")))
            .unwrap();
    }
    latest
}

/// One page of two Messages of `latest`, from `cursor` or from the start.
async fn page(
    root: &Utf8Path,
    latest: &Discussion,
    cursor: Option<String>,
) -> anyhow::Result<Stamped<DiscussionMessagesPage>> {
    read_discussion_messages(
        &repository(root),
        ReadPolicy::default(),
        DiscussionMessagesQuery {
            parent: latest.parent.clone(),
            selector: DiscussionSelector::Discussion {
                discussion_id: latest.discussion_id.clone(),
            },
            limit: 2,
            cursor,
        },
    )
    .await
}

/// A read with the key file `bad` succeeds, the file then holds a key of
/// 32 bytes, and the next read accepts the cursor of the first read.
async fn assert_replaced(bad: &[u8]) {
    let (temp, store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let latest = discussion(&store);
    write_key_file(root, bad);
    let first = page(root, &latest, None).await.unwrap();
    assert_eq!(first.result.entries.len(), 2);
    assert_eq!(std::fs::read(key_path(root)).unwrap().len(), 32);
    let cursor = first.result.next_cursor.expect("page one continues");
    let second = page(root, &latest, Some(cursor)).await.unwrap();
    assert_eq!(second.result.entries.len(), 2);
    assert!(second.result.next_cursor.is_some(), "page two continues");
}

#[tokio::test]
#[verifies("rule_read_replaces_an_invalid_cursor_key", examples)]
async fn a_read_replaces_an_empty_key_file() {
    assert_replaced(b"").await;
}

#[tokio::test]
#[verifies("rule_read_replaces_an_invalid_cursor_key", examples)]
async fn a_read_replaces_a_key_file_of_a_wrong_length() {
    assert_replaced(&[7; 31]).await;
    assert_replaced(&[7; 33]).await;
}

/// The refusal of a cursor of page one after `lose` acts on the cache of
/// the repository at its root.
async fn refusal_after(lose: impl FnOnce(&Utf8Path)) -> ReadFailure {
    let (temp, store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let latest = discussion(&store);
    let first = page(root, &latest, None).await.unwrap();
    let cursor = first.result.next_cursor.expect("page one continues");
    lose(root);
    let error = page(root, &latest, Some(cursor)).await.unwrap_err();
    let refusal = error
        .downcast_ref::<ReadFailure>()
        .unwrap_or_else(|| panic!("a typed read refusal: {error:#}"))
        .clone();
    assert_eq!(std::fs::read(key_path(root)).unwrap().len(), 32);
    let restarted = page(root, &latest, None).await.unwrap();
    assert_eq!(restarted.result.entries.len(), 2);
    refusal
}

#[tokio::test]
#[verifies("rule_read_replaces_an_invalid_cursor_key", examples)]
async fn a_cursor_of_a_lost_key_gets_the_refusal_of_a_cursor_after_a_cache_delete() {
    let after_delete = refusal_after(|root| {
        std::fs::remove_dir_all(ProvenanceLayout::new(root).cache_dir()).unwrap();
    })
    .await;
    let after_loss = refusal_after(|root| write_key_file(root, b"")).await;
    assert_eq!(after_delete, ReadFailure::CursorInvalid);
    assert_eq!(after_loss, after_delete);
}

#[test]
#[verifies("rule_read_replaces_an_invalid_cursor_key", examples)]
fn reads_that_replace_a_key_file_at_the_same_time_use_one_key() {
    const READS: usize = 8;
    let (temp, store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap().to_owned();
    let latest = discussion(&store);
    write_key_file(&root, b"");
    let barrier = Arc::new(Barrier::new(READS));
    // Each read starts on its own thread before any read is joined, so the
    // reads meet at the barrier.
    let mut readers = Vec::with_capacity(READS);
    for _ in 0..READS {
        let (root, latest, barrier) = (root.clone(), latest.clone(), barrier.clone());
        readers.push(std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            barrier.wait();
            runtime
                .block_on(page(&root, &latest, None))
                .unwrap()
                .result
                .next_cursor
                .expect("page one continues")
        }));
    }
    let mut cursors = Vec::with_capacity(READS);
    for reader in readers {
        cursors.push(reader.join().unwrap());
    }
    let key = std::fs::read(key_path(&root)).unwrap();
    assert_eq!(key.len(), 32);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    for cursor in cursors {
        let next = runtime
            .block_on(page(&root, &latest, Some(cursor)))
            .unwrap();
        assert_eq!(next.result.entries.len(), 2);
    }
    assert_eq!(std::fs::read(key_path(&root)).unwrap(), key);
}
