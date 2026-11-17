//! A read over a warm index after the history changed: an amended commit, a
//! rebase, and a new merge commit. Each read gives the history that a read
//! with an empty index gives, and reads the texts of the new commits only.

use super::{fixture, history, records, repository, statement_text, Counted, ONE_READER};
use camino::Utf8Path;
use provenance_macros::verifies;
use provenance_store::review::history::contract::{Commit, Entry};

/// One commit of a plan: its name, its parents, and the edit of the
/// statement of `req_a` that it holds.
type Planned = (&'static str, &'static [&'static str], usize);

/// The commits of `plan` over the saved records file below `root`.
fn commits(root: &Utf8Path, plan: &[Planned]) -> Vec<Commit> {
    let (path, _) = records(root);
    plan.iter()
        .map(|(name, parents, edit)| {
            Commit::new(name, parents, "ann", 1_700_000_000)
                .with_entry(&path, Entry::text(&statement_text(root, *edit)))
        })
        .collect()
}

/// Each entry of the history of `req_a`: its id, the version before it and
/// its changed fields.
type Shape = Vec<(String, Option<String>, Vec<String>)>;

/// The shape of the history of `req_a` through the commits of `plan`, and
/// the text calls of the read.
async fn read(root: &Utf8Path, plan: &[Planned]) -> (Shape, usize) {
    let versions = Counted::new_lists(&commits(root, plan));
    let opened = repository(root).with_file_versions(versions.clone());
    let page = history(&opened, "req_a").await.unwrap();
    let shape = page
        .entries
        .into_iter()
        .map(|entry| {
            (
                entry.id.as_str().to_owned(),
                entry.before.map(|before| before.as_str().to_owned()),
                entry.changed_fields,
            )
        })
        .collect();
    (shape, versions.calls())
}

/// Reads the history through `first`, then through `then`. Gives the shape
/// of the second read and its text calls, and the shape of a read through
/// `then` in a new repository with an empty index.
async fn warm_then(first: &[Planned], then: &[Planned]) -> (Shape, usize, Shape) {
    let _one = ONE_READER.lock().await;
    let (temp, _store) = fixture();
    let root = Utf8Path::from_path(temp.path()).unwrap();
    read(root, first).await;
    let (warm, calls) = read(root, then).await;
    let (cold_temp, _cold_store) = fixture();
    let (cold, _) = read(Utf8Path::from_path(cold_temp.path()).unwrap(), then).await;
    (warm, calls, cold)
}

const LINE: [Planned; 3] = [("c0", &[], 0), ("c1", &["c0"], 1), ("c2", &["c1"], 2)];

/// The ids of the entries of `shape`.
fn ids(shape: &Shape) -> Vec<&str> {
    shape.iter().map(|(id, _, _)| id.as_str()).collect()
}

/// An amended last commit is a new commit on the same parent: the read
/// indexes it alone, and the old commit is not in the history.
#[tokio::test]
#[verifies("rule_record_history_reads_index", examples)]
async fn a_read_after_an_amended_commit_indexes_the_new_commit() {
    let amended = [LINE[0], LINE[1], ("c2a", &["c1"], 3)];
    let (warm, calls, cold) = warm_then(&LINE, &amended).await;
    assert_eq!(warm, cold);
    assert_eq!(ids(&warm), ["c0", "c1", "c2a", "working"]);
    assert_eq!(calls, 2, "one record text and one Discussion text");
}

/// A rebase onto a new commit gives new ids to the rebased commits: the
/// read indexes the new commit and each rebased commit.
#[tokio::test]
#[verifies("rule_record_history_reads_index", examples)]
async fn a_read_after_a_rebase_indexes_the_rebased_commits() {
    let rebased = [
        LINE[0],
        ("n1", &["c0"], 7),
        ("r1", &["n1"], 1),
        ("r2", &["r1"], 2),
    ];
    let (warm, calls, cold) = warm_then(&LINE, &rebased).await;
    assert_eq!(warm, cold);
    assert_eq!(ids(&warm), ["c0", "n1", "r1", "r2", "working"]);
    assert_eq!(calls, 6, "three new commits");
}

/// A new merge commit that resolves to new content follows its first
/// parent: the read indexes the merged branch and the merge.
#[tokio::test]
#[verifies("rule_record_history_reads_index", examples)]
async fn a_read_after_a_new_merge_commit_indexes_the_merge() {
    let merged = [
        LINE[0],
        LINE[1],
        LINE[2],
        ("b2", &["c1"], 5),
        ("m3", &["c2", "b2"], 6),
    ];
    let (warm, calls, cold) = warm_then(&LINE, &merged).await;
    assert_eq!(warm, cold);
    assert_eq!(ids(&warm), ["c0", "c1", "c2", "m3", "working"]);
    assert_eq!(warm[3].1.as_deref(), Some("c2"));
    assert_eq!(calls, 4, "the branch commit and the merge");
}
