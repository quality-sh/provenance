//! The case of a gitlink, the entry of a submodule, at the path of a file.

use super::{assert_versions, commit, named, Commit, Entry, Harness, UNKNOWN_COMMIT};

/// A path that holds a file, then a gitlink, then nothing, then a file.
pub const GITLINK: &str = ".provenance/state/gitlink.jsonl";

/// The commit graph of the case, each parent before its children. `HEAD`
/// is the last commit, `file`.
///
/// - `base` holds [`GITLINK`] as a file.
/// - `absent` puts a gitlink to a commit that the history does not hold at
///   the path, and `held` a gitlink to `base`, which the history holds.
/// - `removed` removes the gitlink and `file` adds a file again.
#[must_use]
pub fn gitlink_graph() -> Vec<Commit> {
    let absent = Entry::gitlink(UNKNOWN_COMMIT);
    let held = Entry::gitlink("base");
    vec![
        commit("base", &[], "ann", 100).with_file(GITLINK, "g0\n"),
        commit("absent", &["base"], "bea", 200).with_entry(GITLINK, absent),
        commit("held", &["absent"], "cal", 300).with_entry(GITLINK, held),
        commit("removed", &["held"], "dan", 400).without_file(GITLINK),
        commit("file", &["removed"], "ann", 500).with_file(GITLINK, "g4\n"),
    ]
}

/// A commit that puts a gitlink at the path is listed with no text, and so
/// is the commit that removes it. This is so when the history holds the
/// commit of the gitlink and when it does not.
pub async fn a_gitlink_at_the_path_is_listed_with_no_text<H: Harness + Sync>(harness: &H) {
    let graph = gitlink_graph();
    let adapter = harness.adapter(&graph);
    assert_versions(
        GITLINK,
        adapter.versions(GITLINK).await,
        vec![
            named(&graph, "base", &[], Some("g0\n")),
            named(&graph, "absent", &["base"], None),
            named(&graph, "held", &["absent"], None),
            named(&graph, "removed", &["held"], None),
            named(&graph, "file", &["removed"], Some("g4\n")),
        ],
    );
}
