//! The case of an entry that is not a file, a submodule entry or a
//! directory, at the path of a file.

use super::{assert_versions, commit, named, Commit, Entry, Harness};

/// A path that holds a file, then an entry that is not a file, then nothing,
/// then a file.
pub const NOT_A_FILE: &str = ".provenance/state/not_a_file.jsonl";

/// The commit graph of the case with `entry` at the path, each parent before
/// its children. `HEAD` is the last commit, `file`.
///
/// - `base` holds [`NOT_A_FILE`] as a file.
/// - `other` puts `entry` at the path. A submodule entry can name `base`.
/// - `removed` removes `entry` and `file` adds a file again.
#[must_use]
pub fn not_a_file_graph(entry: Entry) -> Vec<Commit> {
    vec![
        commit("base", &[], "ann", 100).with_file(NOT_A_FILE, "n0\n"),
        commit("other", &["base"], "bea", 200).with_entry(NOT_A_FILE, entry),
        commit("removed", &["other"], "cal", 300).without_file(NOT_A_FILE),
        commit("file", &["removed"], "dan", 400).with_file(NOT_A_FILE, "n3\n"),
    ]
}

/// A commit that puts `entry`, which is not a file, at the path is listed
/// with no text, and so is the commit that removes it.
pub async fn an_entry_that_is_not_a_file_has_no_text<H: Harness + Sync>(harness: &H, entry: Entry) {
    let graph = not_a_file_graph(entry);
    let adapter = harness.adapter(&graph);
    assert_versions(
        NOT_A_FILE,
        adapter.versions(NOT_A_FILE).await,
        vec![
            named(&graph, "base", &[], Some("n0\n")),
            named(&graph, "other", &["base"], None),
            named(&graph, "removed", &["other"], None),
            named(&graph, "file", &["removed"], Some("n3\n")),
        ],
    );
}
