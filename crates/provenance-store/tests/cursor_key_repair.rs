//! A paged read replaces a cursor key file of the cache that does not hold
//! a valid key, callers at the same time sign with one key, a cursor that
//! the lost key signed refuses as after a delete of the cache, and a read
//! that cannot replace the key names the key file and the cause.
mod discussion_support;
use camino::{Utf8Path, Utf8PathBuf};
use discussion_support::*;
use provenance_core::{
    protocol::{read_failure::ReadFailure, Stamped},
    threads::{Discussion, DiscussionMessagesPage},
};
use provenance_macros::verifies;
use provenance_store::{
    layout::ProvenanceLayout,
    operations::read_policy::{FreshnessPolicy, ReadPolicy},
    review::read_discussion_messages,
};
use std::sync::{mpsc, Arc, Barrier};
use std::time::Duration;

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

/// One page of two Messages of `latest` under `policy`, from `cursor` or
/// from the start.
async fn page(
    root: &Utf8Path,
    policy: ReadPolicy,
    latest: &Discussion,
    cursor: Option<String>,
) -> anyhow::Result<Stamped<DiscussionMessagesPage>> {
    read_discussion_messages(&repository(root), policy, messages_query(latest, 2, cursor)).await
}

/// The policy of a read that takes no publication guard.
const ANNOTATE_ONLY: ReadPolicy = ReadPolicy::with_freshness(FreshnessPolicy::AnnotateOnly);

/// A read with the key file `bad` succeeds, the file then holds a key of
/// 32 bytes, and the next read accepts the cursor of the first read.
async fn assert_replaced(bad: &[u8]) {
    let (temp, store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let latest = five_messages(&store);
    write_key_file(root, bad);
    let first = page(root, ReadPolicy::default(), &latest, None)
        .await
        .unwrap();
    assert_eq!(first.result.entries.len(), 2);
    assert_eq!(std::fs::read(key_path(root)).unwrap().len(), 32);
    let cursor = first.result.next_cursor.expect("page one continues");
    let second = page(root, ReadPolicy::default(), &latest, Some(cursor))
        .await
        .unwrap();
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
    let latest = five_messages(&store);
    let first = page(root, ReadPolicy::default(), &latest, None)
        .await
        .unwrap();
    let cursor = first.result.next_cursor.expect("page one continues");
    lose(root);
    let error = page(root, ReadPolicy::default(), &latest, Some(cursor))
        .await
        .unwrap_err();
    let refusal = error
        .downcast_ref::<ReadFailure>()
        .unwrap_or_else(|| panic!("a typed read refusal: {error:#}"))
        .clone();
    assert_eq!(std::fs::read(key_path(root)).unwrap().len(), 32);
    let restarted = page(root, ReadPolicy::default(), &latest, None)
        .await
        .unwrap();
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

/// A runtime for one thread of the concurrent test.
fn new_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

/// In each round, eight reads on eight threads start at one barrier with an
/// empty key file. One read with the catch-up policy first makes the
/// projection current; the reads of a round take no publication guard, so
/// nothing puts them in a line before they reach the key. The test thread
/// goes to a barrier only after each reader thread sends the message that
/// comes before that barrier. Thus, when a reader thread stops, the test
/// gives an error message and does not wait with no limit.
#[test]
#[verifies("rule_read_replaces_an_invalid_cursor_key", examples)]
fn reads_that_replace_a_key_file_at_the_same_time_use_one_key() {
    const READS: usize = 8;
    const ROUNDS: usize = 10;
    const LIMIT: Duration = Duration::from_secs(60);
    let (temp, store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap().to_owned();
    let latest = five_messages(&store);
    let runtime = new_runtime();
    runtime
        .block_on(page(&root, ReadPolicy::default(), &latest, None))
        .unwrap();
    let (start, end) = (
        Arc::new(Barrier::new(READS + 1)),
        Arc::new(Barrier::new(READS + 1)),
    );
    let (at_start, arrivals) = mpsc::channel();
    let (sender, cursors) = mpsc::channel();
    let readers: Vec<_> = (0..READS)
        .map(|_| {
            let (root, latest) = (root.clone(), latest.clone());
            let (start, end) = (start.clone(), end.clone());
            let (at_start, sender) = (at_start.clone(), sender.clone());
            std::thread::spawn(move || {
                let runtime = new_runtime();
                let repository = repository(&root);
                for _ in 0..ROUNDS {
                    at_start.send(()).unwrap();
                    start.wait();
                    let query = messages_query(&latest, 2, None);
                    let read = runtime.block_on(read_discussion_messages(
                        &repository,
                        ANNOTATE_ONLY,
                        query,
                    ));
                    // A send and not a panic, so that each thread reaches the
                    // barrier at the end of each round.
                    let cursor = read.map(|page| page.result.next_cursor);
                    sender
                        .send(cursor.map_err(|error| format!("{error:#}")))
                        .unwrap();
                    end.wait();
                }
            })
        })
        .collect();
    drop((at_start, sender));
    for round in 0..ROUNDS {
        write_key_file(&root, b"");
        for _ in 0..READS {
            arrivals.recv_timeout(LIMIT).unwrap_or_else(|error| {
                panic!("round {round}: a reader thread did not reach the start: {error}")
            });
        }
        start.wait();
        let reads: Vec<_> = (0..READS)
            .map(|_| {
                cursors.recv_timeout(LIMIT).unwrap_or_else(|error| {
                    panic!("round {round}: a reader thread did not give its read: {error}")
                })
            })
            .collect();
        end.wait();
        let key = std::fs::read(key_path(&root)).unwrap();
        assert_eq!(key.len(), 32, "round {round}");
        for read in reads {
            let cursor = read.unwrap().expect("page one continues");
            let next = runtime
                .block_on(page(&root, ReadPolicy::default(), &latest, Some(cursor)))
                .unwrap_or_else(|error| panic!("round {round}: {error:#}"));
            assert_eq!(next.result.entries.len(), 2);
        }
        assert_eq!(
            std::fs::read(key_path(&root)).unwrap(),
            key,
            "round {round}"
        );
    }
    for reader in readers {
        reader.join().unwrap();
    }
}

/// The cache of a repository and the error of a read with an empty key
/// file when that cache is read-only. The repository opens first, as in a
/// process that serves reads, and `prepare` then acts on the cache. The
/// cause of the error follows the key file of the cache.
#[cfg(unix)]
fn refusal_in_a_read_only_cache(prepare: impl FnOnce(&Utf8Path)) -> (Utf8PathBuf, String) {
    use std::os::unix::fs::PermissionsExt;
    let (temp, store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    let latest = five_messages(&store);
    let runtime = new_runtime();
    runtime
        .block_on(page(root, ReadPolicy::default(), &latest, None))
        .unwrap();
    write_key_file(root, b"");
    let repository = repository(root);
    let cache = ProvenanceLayout::new(root).cache_dir();
    prepare(&cache);
    let mode = |mode| std::fs::Permissions::from_mode(mode);
    std::fs::set_permissions(&cache, mode(0o555)).unwrap();
    let writable = std::fs::File::create(cache.join("probe")).is_ok();
    let query = messages_query(&latest, 2, None);
    let read = runtime.block_on(read_discussion_messages(&repository, ANNOTATE_ONLY, query));
    std::fs::set_permissions(&cache, mode(0o755)).unwrap();
    assert!(
        !writable,
        "this user can write a directory of mode 0555, so the test cannot make a read-only cache"
    );
    let error = format!("{:#}", read.map(|_| ()).unwrap_err());
    assert_eq!(std::fs::read(key_path(root)).unwrap(), b"");
    let context = format!(
        "the cursor key file {} is absent or does not hold a key, and the store cannot replace \
         it: ",
        key_path(root)
    );
    let cause = error
        .strip_prefix(&context)
        .unwrap_or_else(|| panic!("an error that names the key file: {error}"));
    (cache, cause.to_owned())
}

#[cfg(unix)]
#[test]
#[verifies("rule_read_replaces_an_invalid_cursor_key", examples)]
fn a_read_that_cannot_replace_a_key_file_names_the_key_file_and_the_path_that_failed() {
    let remove_locks = |cache: &Utf8Path| std::fs::remove_dir_all(cache.join("locks")).unwrap();
    let (cache, cause) = refusal_in_a_read_only_cache(remove_locks);
    let locks = cache.join("locks");
    let expected = format!("create advisory lock directory {locks}: Permission denied");
    assert!(cause.starts_with(&expected), "{cause}");
    let (cache, cause) = refusal_in_a_read_only_cache(|_| {});
    assert!(cause.contains("Permission denied"), "{cause}");
    assert!(cause.contains(cache.as_str()), "{cause}");
}
