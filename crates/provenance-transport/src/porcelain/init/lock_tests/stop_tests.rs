//! A process stops during a second init. The next command reads the
//! repository, whatever file move the stop comes at.
//!
//! The probes of the store are active only in the unit tests of the store, so
//! no stop comes in the manifest publish. The stops come at the moves of the
//! project files only.
use super::{init_input, init_through_the_port};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::repository::Repository;

const ROOT: &str = "PROVENANCE_STOPPED_INIT_ROOT";
const STOP_AT: &str = "PROVENANCE_STOPPED_INIT_MOVE";
const STOPPED: i32 = 86;
const FINISHED: i32 = 87;
/// More moves than a second init of this repository makes.
const MOVE_LIMIT: usize = 20;
/// The second init moves AGENTS.md and .gitignore away. The skill files do
/// not change, the repository has no dictionary reference, and the manifest
/// does not move away.
const MOVES: usize = 2;
const CHILD: &str = "porcelain::init::lock_tests::stop_tests::stopped_second_init_child";

/// The child process: a second init that stops at once when it moves file
/// number `STOP_AT` to its backup file, before it moves the new file in.
#[test]
fn stopped_second_init_child() {
    let (Ok(root), Ok(stop_at)) = (std::env::var(ROOT), std::env::var(STOP_AT)) else {
        return;
    };
    let stop_at: usize = stop_at.parse().unwrap();
    init_through_the_port(
        init_input(Utf8Path::new(&root), &["new-reviewer"]),
        move || {
            let mut moves = 0;
            crate::test_probes::arm("file_displaced", move || {
                moves += 1;
                if moves == stop_at {
                    std::process::exit(STOPPED);
                }
            });
        },
    )
    .unwrap();
    std::process::exit(FINISHED);
}

/// An initialized repository with the reviewer "reviewer". A person changed
/// AGENTS.md and .gitignore, so a second init writes them again.
fn repository_with_changed_files() -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    init_through_the_port(init_input(&repo, &["reviewer"]), || {}).unwrap();
    std::fs::write(repo.join("AGENTS.md"), "# Team notes\n").unwrap();
    std::fs::write(repo.join(".gitignore"), "target/\n").unwrap();
    (directory, repo)
}

/// Runs the second init in a child process that stops at move `stop_at`.
/// Returns `false` when init made fewer moves and ended.
fn stop_second_init(repo: &Utf8Path, stop_at: usize) -> bool {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([CHILD, "--exact", "--nocapture"])
        .env(ROOT, repo.as_str())
        .env(STOP_AT, stop_at.to_string())
        .status()
        .unwrap();
    match status.code() {
        Some(STOPPED) => true,
        Some(FINISHED) => false,
        other => panic!("the second init failed before move {stop_at}: {other:?}"),
    }
}

fn reviewers(repo: &Utf8Path) -> anyhow::Result<Vec<String>> {
    let manifest = Repository::open(repo, "default")?.manifest()?;
    Ok(manifest.disposition_actor_ids)
}

/// A second init changes the reviewer list from "reviewer" to
/// "new-reviewer". The process stops at each file move of that init in turn.
/// Each move comes before the manifest publish, so after each stop the
/// manifest has its bytes from before init, the repository opens with the old
/// reviewer list, and an init with no reviewer option keeps that list.
/// This test has no Rule binding, because the Rule for a stopped init waits
/// for the review of its text by the owner.
#[test]
fn a_stop_at_each_file_move_of_a_second_init_leaves_the_old_manifest() {
    let mut stops = 0;
    for stop_at in 1..=MOVE_LIMIT {
        let (_directory, repo) = repository_with_changed_files();
        let manifest_path = ProvenanceLayout::new(repo.clone()).manifest_path();
        let manifest = std::fs::read(&manifest_path).unwrap();
        if !stop_second_init(&repo, stop_at) {
            break;
        }
        stops += 1;

        assert_eq!(
            std::fs::read(&manifest_path).unwrap(),
            manifest,
            "after a stop at move {stop_at}, the manifest changed"
        );
        let after_stop = reviewers(&repo).unwrap_or_else(|error| {
            panic!("after a stop at move {stop_at}, the read got an error: {error:#}")
        });
        assert_eq!(after_stop, ["reviewer"], "after a stop at move {stop_at}");
        let mut bare = init_input(&repo, &[]);
        bare.scope = None;
        bare.path_prefix = None;
        init_through_the_port(bare, || {}).unwrap_or_else(|error| {
            panic!("after a stop at move {stop_at}, init failed: {error:#}")
        });
        assert_eq!(
            reviewers(&repo).unwrap(),
            ["reviewer"],
            "the init after a stop at move {stop_at} changed the reviewer list"
        );
    }
    assert_eq!(stops, MOVES, "the number of file moves of the second init");
}
