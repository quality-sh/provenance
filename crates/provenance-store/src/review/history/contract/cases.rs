//! The cases over the commit graph [`graph`].

use super::{
    assert_versions, graph, named, Harness, Named, ABSENT, DISCUSSIONS, MERGED, MOVED_FROM,
    MOVED_TO, RECORDS,
};

/// The versions of a file list each commit in the history of `HEAD` that
/// changed it, with the nearest listed commit on each parent line, each
/// parent before its child.
///
/// A merge that keeps the file of one parent, first or second, is not
/// listed, and the commits that only its other parents reach are not
/// listed. A merge with new text is listed with each parent line. A
/// removal is listed with no text. When more than one parent of a merge has
/// its file, the history goes on through the first of them. A moved file
/// starts at the move. The last version is the first commit that changed the
/// file on the walk from `HEAD`, which need not be `HEAD`. Each version
/// carries the author name and the commit time of its commit.
pub async fn versions_follow_the_history_of_head<H: Harness + Sync>(harness: &H) {
    let graph = graph();
    let adapter = harness.adapter(&graph);
    let records = adapter.versions(RECORDS).await;
    assert_eq!(
        records.last().map(|version| version.0.as_str()),
        Some("revert"),
        "{RECORDS}: the last version is not the first commit that changed the file on the walk from HEAD"
    );
    assert_versions(
        RECORDS,
        records,
        vec![
            named(&graph, "create", &[], Some("r1\n")),
            named(&graph, "left", &["create"], Some("r2\n")),
            named(&graph, "right", &["create"], Some("r3\n")),
            named(&graph, "merge", &["left", "right"], Some("r4\n")),
            named(&graph, "delete", &["merge"], None),
            named(&graph, "again", &["delete"], Some("r9\n")),
            named(&graph, "drift", &["again"], Some("r10\n")),
            named(&graph, "revert", &["drift"], Some("r9\n")),
        ],
    );
    let discussions = adapter.versions(DISCUSSIONS).await;
    assert_eq!(
        discussions,
        vec![
            named(&graph, "start", &[], Some("d0\n")),
            named(&graph, "left", &["start"], Some("d2\n")),
            named(&graph, "discuss", &["left"], Some("d5\n")),
        ]
    );
    assert_versions(
        MERGED,
        adapter.versions(MERGED).await,
        vec![
            named(&graph, "split", &[], Some("m1\n")),
            named(&graph, "pick", &["split"], Some("m2\n")),
        ],
    );
    assert_versions(
        MOVED_FROM,
        adapter.versions(MOVED_FROM).await,
        vec![
            named(&graph, "draft", &[], Some("v1\n")),
            named(&graph, "edit", &["draft"], Some("v2\n")),
            named(&graph, "move", &["edit"], None),
        ],
    );
    assert_versions(
        MOVED_TO,
        adapter.versions(MOVED_TO).await,
        vec![named(&graph, "move", &[], Some("v2\n"))],
    );
}

/// The text of a file is its text at the named commit, whether or not that
/// commit changed it: `None` before the file exists and after its removal.
pub async fn text_gives_the_file_at_a_commit<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(&graph());
    let expected = [
        ("again", Some("r9\n")),
        ("start", None),
        ("discuss", Some("r4\n")),
        ("ours", Some("r4\n")),
        ("delete", None),
        ("right", Some("r3\n")),
        ("theirs", Some("r6\n")),
        ("join", Some("r9\n")),
        ("other", Some("r11\n")),
    ];
    for (commit, text) in expected {
        assert_eq!(
            adapter.text(RECORDS, commit).await.as_deref(),
            text,
            "{RECORDS}: the text at {commit} is not the file at that commit"
        );
    }
    let expected = [
        ("create", Some("d0\n")),
        ("right", Some("d0\n")),
        ("merge", Some("d2\n")),
        ("join", Some("d5\n")),
    ];
    for (commit, text) in expected {
        assert_eq!(
            adapter.text(DISCUSSIONS, commit).await.as_deref(),
            text,
            "{DISCUSSIONS}: the text at {commit} is not the file at that commit"
        );
    }
}

/// A file that no commit holds has no versions and no text.
pub async fn a_file_that_no_commit_holds_has_no_versions<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(&graph());
    assert_eq!(adapter.versions(ABSENT).await, Vec::<Named>::new());
    assert_eq!(adapter.text(ABSENT, "move").await, None);
}
