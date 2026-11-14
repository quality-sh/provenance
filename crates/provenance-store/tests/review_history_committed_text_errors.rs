//! Characterization of the record history read when a committed text is
//! not UTF-8.
//!
//! Implementation aid with no Rule binding: these tests pin the refusal of a
//! history read for a committed text that is not UTF-8 and for a committed
//! object that Git cannot read, and the refusal that wins when a read has
//! more than one of these faults.

mod discussion_support;
use discussion_support::*;
use provenance_core::review::ReviewHistoryQuery;
use provenance_core::NodeType;
use provenance_store::{
    layout::ProvenanceLayout, operations::read_policy::ReadPolicy, review::read_history, shards,
};
use serde_json::json;

/// The refusal text of a history read of `req_a`.
async fn refusal(temp: &tempfile::TempDir) -> String {
    let query = ReviewHistoryQuery {
        record_kind: NodeType::Requirement,
        record_id: id(),
        limit: 50,
        cursor: None,
    };
    let root = camino::Utf8Path::from_path(temp.path()).unwrap();
    let error = read_history(&repository(root), ReadPolicy::default(), query)
        .await
        .map(|_| ())
        .unwrap_err();
    format!("{error:#}")
}

/// Commits `bytes` as the file at `path`, then puts the earlier working
/// text back, so that only the commit holds the bytes.
fn commit_bytes(temp: &tempfile::TempDir, path: &camino::Utf8Path, bytes: &[u8]) {
    let working = std::fs::read(path).unwrap();
    std::fs::write(path, bytes).unwrap();
    git(temp, &["add", path.as_str()]);
    git(temp, &["commit", "-q", "-m", "bytes"]);
    std::fs::write(path, working).unwrap();
}

fn paths(temp: &tempfile::TempDir) -> (camino::Utf8PathBuf, camino::Utf8PathBuf) {
    let layout = ProvenanceLayout::new(camino::Utf8Path::from_path(temp.path()).unwrap());
    (
        shards::path_for(&layout, &scope(), NodeType::Requirement),
        shards::discussions_path(&layout, &scope()),
    )
}

#[tokio::test]
async fn a_committed_record_file_that_is_not_utf8_refuses_the_history_read() {
    let (temp, _store) = fixture();
    commit_state(&temp, "create");
    let (records, _) = paths(&temp);
    commit_bytes(&temp, &records, b"\xff\n");
    commit_state(&temp, "again");
    assert_eq!(
        refusal(&temp).await,
        "invalid utf-8 sequence of 1 bytes from index 0"
    );
}

#[tokio::test]
async fn a_committed_discussion_file_that_is_not_utf8_refuses_the_history_read() {
    let (temp, store) = fixture();
    start(&store, "Use text A.");
    commit_state(&temp, "create");
    store
        .save_requirement(save(&store, json!({"description":"Text A."})))
        .unwrap();
    let (records, discussions) = paths(&temp);
    git(&temp, &["add", records.as_str()]);
    commit_bytes(&temp, &discussions, b"{}\n\xff\n");
    assert_eq!(
        refusal(&temp).await,
        "invalid utf-8 sequence of 1 bytes from index 3"
    );
}

/// The text of the older commit is read first, so its refusal wins: it names
/// byte 2, and the newer text names byte 0.
#[tokio::test]
async fn the_oldest_committed_text_that_is_not_utf8_wins() {
    let (temp, _store) = fixture();
    commit_state(&temp, "create");
    let (records, _) = paths(&temp);
    commit_bytes(&temp, &records, b"ab\xff\n");
    commit_bytes(&temp, &records, b"\xff\n");
    commit_state(&temp, "again");
    assert_eq!(
        refusal(&temp).await,
        "invalid utf-8 sequence of 1 bytes from index 2"
    );
}

/// Damages the stored object of the file at `path` in `commit`, so that Git
/// gives its header and then fails. Gives the object id.
fn damage(temp: &tempfile::TempDir, commit: &str, path: &camino::Utf8Path) -> String {
    let relative = path.strip_prefix(temp.path()).unwrap();
    let object = git(temp, &["rev-parse", &format!("{commit}:{relative}")]);
    let file = temp
        .path()
        .join(".git/objects")
        .join(&object[..2])
        .join(&object[2..]);
    let mut bytes = std::fs::read(&file).unwrap();
    // The last bytes are the check sum of the compressed data.
    *bytes.last_mut().unwrap() ^= 0xff;
    // Git makes an object file read-only; Windows does not remove such a file.
    #[cfg(windows)]
    {
        let mut permissions = std::fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(false);
        std::fs::set_permissions(&file, permissions).unwrap();
    }
    std::fs::remove_file(&file).unwrap();
    std::fs::write(&file, bytes).unwrap();
    object
}

/// Asserts that `refusal` is the Git failure for the object `object`.
fn assert_git_failure(refusal: &str, object: &str) {
    assert!(
        refusal.starts_with("git cat-file failed: ") && refusal.contains(object),
        "{refusal}"
    );
}

#[tokio::test]
async fn a_committed_record_object_that_git_cannot_read_refuses_the_history_read() {
    let (temp, store) = fixture();
    let created = commit_state(&temp, "create");
    store
        .save_requirement(save(&store, json!({"description":"Text A."})))
        .unwrap();
    commit_state(&temp, "change");
    let (records, _) = paths(&temp);
    let object = damage(&temp, &created, &records);
    assert_git_failure(&refusal(&temp).await, &object);
}

/// One Git process read every text of the file, so its failure wins over the
/// older text that is not UTF-8.
#[tokio::test]
async fn a_git_failure_wins_over_an_older_record_text_that_is_not_utf8() {
    let (temp, store) = fixture();
    commit_state(&temp, "create");
    let (records, _) = paths(&temp);
    commit_bytes(&temp, &records, b"\xff\n");
    store
        .save_requirement(save(&store, json!({"description":"Text A."})))
        .unwrap();
    let changed = commit_state(&temp, "change");
    let object = damage(&temp, &changed, &records);
    assert_git_failure(&refusal(&temp).await, &object);
}

/// Commits a change of `req_a` with `bytes` as the Discussion file.
fn commit_change(
    temp: &tempfile::TempDir,
    store: &provenance_store::state_store::StateStore,
    description: &str,
    bytes: &[u8],
) -> String {
    store
        .save_requirement(save(store, json!({ "description": description })))
        .unwrap();
    let (records, discussions) = paths(temp);
    git(temp, &["add", records.as_str()]);
    commit_bytes(temp, &discussions, bytes);
    git(temp, &["rev-parse", "HEAD"])
}

#[tokio::test]
async fn a_git_failure_wins_over_an_older_discussion_text_that_is_not_utf8() {
    let (temp, store) = fixture();
    start(&store, "Use text A.");
    commit_state(&temp, "create");
    commit_change(&temp, &store, "Text A.", b"{}\n\xff\n");
    // Git reads a small object whole with its header and does not see the
    // damage, so this text is larger.
    let large = format!("{{}}\n{}\n", "x".repeat(4096));
    let damaged = commit_change(&temp, &store, "Text B.", large.as_bytes());
    let (_, discussions) = paths(&temp);
    let object = damage(&temp, &damaged, &discussions);
    assert_git_failure(&refusal(&temp).await, &object);
}

/// The record texts are read before the Discussion texts: the record text
/// names byte 0, and the Discussion text names byte 3.
#[tokio::test]
async fn a_record_text_error_wins_over_a_discussion_text_error() {
    let (temp, store) = fixture();
    start(&store, "Use text A.");
    commit_state(&temp, "create");
    commit_change(&temp, &store, "Text A.", b"{}\n\xff\n");
    let (records, _) = paths(&temp);
    commit_bytes(&temp, &records, b"\xff\n");
    assert_eq!(
        refusal(&temp).await,
        "invalid utf-8 sequence of 1 bytes from index 0"
    );
}

/// The Discussion text of the older version names byte 2, and the newer one
/// names byte 0.
#[tokio::test]
async fn the_oldest_committed_discussion_text_that_is_not_utf8_wins() {
    let (temp, store) = fixture();
    start(&store, "Use text A.");
    commit_state(&temp, "create");
    commit_change(&temp, &store, "Text A.", b"ab\xff\n");
    commit_change(&temp, &store, "Text B.", b"\xff\n");
    assert_eq!(
        refusal(&temp).await,
        "invalid utf-8 sequence of 1 bytes from index 2"
    );
}
