//! The git adapter at a Provenance root in a subdirectory of its work tree:
//! each text read names the file with the path of the root inside the work
//! tree.

use super::git_harness::{git_harness, git_harness_in};
use provenance_store::review::history::contract::{self, graph, Harness, RECORDS};

#[tokio::test]
async fn git_in_a_subdirectory_versions_follow_the_history_of_head() {
    contract::versions_follow_the_history_of_head(&git_harness_in("nested/root/")).await;
}

#[tokio::test]
async fn git_in_a_subdirectory_text_gives_the_file_at_a_commit() {
    contract::text_gives_the_file_at_a_commit(&git_harness_in("nested/root/")).await;
}

/// The version read of another root, at the top of its work tree, is the
/// last read when the root in a subdirectory reads its first text. The text
/// read must not take the path of that other root.
#[tokio::test]
async fn git_in_a_subdirectory_text_does_not_take_the_path_of_another_root() {
    let graph = graph();
    // A harness keeps its repositories until it is dropped.
    let (top, nested) = (git_harness(), git_harness_in("own/prefix/"));
    let top = top.adapter(&graph);
    let listed = top.versions.versions(RECORDS).await.unwrap();
    assert!(!listed.is_empty());
    let nested = nested.adapter(&graph);
    let text = nested
        .versions
        .text(RECORDS, &nested.ids["create"])
        .await
        .unwrap();
    assert_eq!(text.as_deref(), Some("r1\n"));
}
