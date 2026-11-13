//! The cases over the commit graph [`edge_graph`]: an octopus merge, two
//! parent lines that meet, a change of mode only, a commit that the adapter
//! does not hold, and a text that is not UTF-8.

use super::{assert_versions, commit, named, Commit, Entry, Harness};

/// A file that an octopus merge takes from its third parent.
pub const OCTOPUS: &str = ".provenance/state/octopus.jsonl";
/// A file that two parent lines change after they leave one commit.
pub const TWO_LINES: &str = ".provenance/state/two_lines.jsonl";
/// A file that a commit makes executable with no change of its text.
pub const MODE: &str = ".provenance/state/mode.jsonl";
/// A file whose text is not UTF-8.
pub const UNREADABLE: &str = ".provenance/state/unreadable.jsonl";
/// A file that each commit of the graph holds.
pub const UNHELD: &str = OCTOPUS;
/// A commit id that no history of the cases holds.
pub const UNKNOWN_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// The commit graph of these cases, each parent before its children. `HEAD`
/// is the last commit, `exec`.
///
/// - `base` holds each file. Its [`UNREADABLE`] text is not UTF-8.
/// - `one`, `two` and `three` change [`OCTOPUS`] on three lines from `base`.
///   `octopus` merges them and keeps the file of its third parent `three`,
///   so only `base` and `three` are listed.
/// - `side_a` and `side_b` change another file on two lines from
///   `octopus`; `line_a` and `line_b` then change [`TWO_LINES`] and `meet`
///   merges them with new text. Both lines reach `base`, which is listed
///   once.
/// - `exec` makes [`MODE`] executable with the same text.
#[must_use]
pub fn edge_graph() -> Vec<Commit> {
    const OTHER: &str = ".provenance/state/other.jsonl";
    let executable = Entry::new(b"m0\n".to_vec(), true);
    let unreadable = Entry::new(b"\xff\n".to_vec(), false);
    vec![
        commit("base", &[], "ann", 100)
            .with_file(OCTOPUS, "o0\n")
            .with_file(TWO_LINES, "t0\n")
            .with_file(MODE, "m0\n")
            .with_entry(UNREADABLE, unreadable),
        commit("one", &["base"], "bea", 200).with_file(OCTOPUS, "o1\n"),
        commit("two", &["base"], "cal", 300).with_file(OCTOPUS, "o2\n"),
        commit("three", &["base"], "dan", 400).with_file(OCTOPUS, "o3\n"),
        commit("octopus", &["one", "two", "three"], "ann", 500).with_file(OCTOPUS, "o3\n"),
        commit("side_a", &["octopus"], "bea", 600).with_file(OTHER, "a\n"),
        commit("side_b", &["octopus"], "cal", 700).with_file(OTHER, "b\n"),
        commit("line_a", &["side_a"], "dan", 800).with_file(TWO_LINES, "t1\n"),
        commit("line_b", &["side_b"], "ann", 900).with_file(TWO_LINES, "t2\n"),
        commit("meet", &["line_a", "line_b"], "bea", 1000)
            .with_file(TWO_LINES, "t3\n")
            .with_file(OTHER, "ab\n"),
        commit("exec", &["meet"], "cal", 1100).with_entry(MODE, executable),
    ]
}

/// An octopus merge that keeps the file of its third parent is not listed,
/// and the history goes on through that parent only.
pub async fn an_octopus_merge_goes_on_through_the_parent_with_its_file<H: Harness + Sync>(
    harness: &H,
) {
    let graph = edge_graph();
    let adapter = harness.adapter(&graph);
    assert_versions(
        OCTOPUS,
        adapter.versions(OCTOPUS).await,
        vec![
            named(&graph, "base", &[], Some("o0\n")),
            named(&graph, "three", &["base"], Some("o3\n")),
        ],
    );
}

/// A commit that two parent lines of a merge reach is listed once.
pub async fn a_commit_on_two_parent_lines_is_listed_once<H: Harness + Sync>(harness: &H) {
    let graph = edge_graph();
    let adapter = harness.adapter(&graph);
    assert_versions(
        TWO_LINES,
        adapter.versions(TWO_LINES).await,
        vec![
            named(&graph, "base", &[], Some("t0\n")),
            named(&graph, "line_a", &["base"], Some("t1\n")),
            named(&graph, "line_b", &["base"], Some("t2\n")),
            named(&graph, "meet", &["line_a", "line_b"], Some("t3\n")),
        ],
    );
}

/// A commit that changes only the mode of the file is listed, with the
/// same text.
pub async fn a_change_of_mode_only_is_listed<H: Harness + Sync>(harness: &H) {
    let graph = edge_graph();
    let adapter = harness.adapter(&graph);
    assert_versions(
        MODE,
        adapter.versions(MODE).await,
        vec![
            named(&graph, "base", &[], Some("m0\n")),
            named(&graph, "exec", &["base"], Some("m0\n")),
        ],
    );
}

/// The text at a commit that the adapter does not hold is `None`, also for
/// a file that each commit it holds has.
pub async fn a_commit_that_the_adapter_does_not_hold_has_no_text<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(&edge_graph());
    let text = adapter.versions.text(UNHELD, UNKNOWN_COMMIT).await.unwrap();
    assert_eq!(
        text, None,
        "{UNHELD}: the text at a commit that the adapter does not hold is not None"
    );
}

/// A commit whose file text is not UTF-8 is listed, and its text is an
/// error.
pub async fn a_text_that_is_not_utf8_is_an_error<H: Harness + Sync>(harness: &H) {
    let adapter = harness.adapter(&edge_graph());
    let versions = adapter.versions.versions(UNREADABLE).await.unwrap();
    let commits = versions
        .iter()
        .map(|version| adapter.name(&version.commit))
        .collect::<Vec<_>>();
    assert_eq!(
        commits,
        ["base"],
        "{UNREADABLE}: the listed commits are not the expected ones"
    );
    let text = adapter
        .versions
        .text(UNREADABLE, &adapter.ids["base"])
        .await;
    assert!(
        text.is_err(),
        "{UNREADABLE}: a text that is not UTF-8 is not an error: {text:?}"
    );
}
