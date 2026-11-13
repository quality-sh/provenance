//! Characterization of the record history read when a committed text is
//! not UTF-8.
//!
//! Implementation aid with no Rule binding: these tests pin the refusal of a
//! history read for a committed text that is not UTF-8, and the refusal that
//! wins when two committed texts are not UTF-8.

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
