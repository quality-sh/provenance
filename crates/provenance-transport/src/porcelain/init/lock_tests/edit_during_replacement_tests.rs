//! An edit of AGENTS.md, or an open handle on it, while a second init
//! replaces it. The init runs through `InitPort::init`, the port that the
//! CLI calls.
use super::{init_input, init_through_the_port, tree};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;
use provenance_store::repository::Repository;

const PERSON_AGENTS: &str = "# Team notes\n";
const EDIT: &str = "# Team notes\n\nAn edit while init runs.\n";

/// An initialized repository in which the person changed AGENTS.md, so that
/// a second init replaces it. The second init replaces no other project
/// file.
fn repository_with_changed_agents() -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    init_through_the_port(init_input(&repo, &["reviewer"]), || {}).unwrap();
    std::fs::write(repo.join("AGENTS.md"), PERSON_AGENTS).unwrap();
    (directory, repo)
}

/// A second init that writes `EDIT` to AGENTS.md the first time that it
/// comes to `point`.
fn init_with_an_edit_at(
    repo: &Utf8Path,
    point: &'static str,
) -> anyhow::Result<provenance_porcelain::init::InitReport> {
    let agents = repo.join("AGENTS.md");
    init_through_the_port(init_input(repo, &["new-reviewer"]), move || {
        let agents = agents.clone();
        let mut edited = false;
        crate::test_probes::arm(point, move || {
            if !edited {
                edited = true;
                std::fs::write(&agents, EDIT).unwrap();
            }
        });
    })
}

fn reviewers(repo: &Utf8Path) -> Vec<String> {
    Repository::open(repo, "default")
        .unwrap()
        .manifest()
        .unwrap()
        .disposition_actor_ids
}

/// The paths of `repo` with `.provenance-` in the name: the temporary files
/// of init.
fn temporary_files(repo: &Utf8Path) -> Vec<String> {
    tree(repo)
        .into_keys()
        .filter(|name| name.contains(".provenance-"))
        .collect()
}

/// An edit of AGENTS.md after init wrote its temporary file and before init
/// compares AGENTS.md with its plan: init refuses, AGENTS.md keeps the edit,
/// the manifest keeps its reviewer list, and no temporary file stays.
#[test]
#[verifies("rule_init_refuses_a_project_file_change_before_its_compare", examples)]
fn an_edit_before_the_compare_is_refused_and_kept() {
    let (_directory, repo) = repository_with_changed_agents();

    let error = init_with_an_edit_at(&repo, "project_file_prepared").unwrap_err();

    let agents = repo.join("AGENTS.md");
    assert_eq!(
        format!("{error:#}"),
        format!(
            "failed to replace {agents}: {agents} changed during replacement",
            agents = agents.as_std_path().display()
        )
    );
    assert_eq!(std::fs::read_to_string(&agents).unwrap(), EDIT);
    assert_eq!(reviewers(&repo), ["reviewer"]);
    assert_eq!(temporary_files(&repo), Vec::<String>::new());
}

/// An edit of AGENTS.md after init compared AGENTS.md with its plan and
/// before the rename: the rename puts the planned file in place of the edit,
/// and init ends with success. This is the limit of the guard against an
/// edit during init.
/// This test has no Rule binding: it shows a loss that no Rule promises.
#[test]
fn an_edit_after_the_compare_is_replaced() {
    let (_directory, repo) = repository_with_changed_agents();

    init_with_an_edit_at(&repo, "project_file_compared").unwrap();

    let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(agents.starts_with(PERSON_AGENTS), "{agents}");
    assert!(!agents.contains("An edit while init runs"), "{agents}");
    assert!(agents.contains("## Provenance"), "{agents}");
    assert_eq!(reviewers(&repo), ["new-reviewer"]);
    assert_eq!(temporary_files(&repo), Vec::<String>::new());
}

/// A reader that has AGENTS.md open, with the file sharing of `std::fs`,
/// does not stop the replacement.
/// This test has no Rule binding: it checks the platform behaviour that the
/// replacement depends on.
#[test]
fn a_reader_with_the_file_open_does_not_stop_the_replacement() {
    let (_directory, repo) = repository_with_changed_agents();
    let reader = std::fs::File::open(repo.join("AGENTS.md")).unwrap();

    init_through_the_port(init_input(&repo, &["new-reviewer"]), || {}).unwrap();
    drop(reader);

    let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(agents.starts_with(PERSON_AGENTS), "{agents}");
    assert!(agents.contains("## Provenance"), "{agents}");
    assert_eq!(temporary_files(&repo), Vec::<String>::new());
}

/// On Windows, a handle on AGENTS.md that does not let other handles delete
/// the file makes the rename fail. Init refuses with the path and the cause,
/// AGENTS.md keeps its old bytes, and no temporary file stays.
/// This test has no Rule binding: it checks a platform refusal.
#[cfg(windows)]
#[test]
fn a_handle_that_refuses_delete_sharing_refuses_the_replacement() {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_SHARE_READ: u32 = 1;
    let (_directory, repo) = repository_with_changed_agents();
    let agents = repo.join("AGENTS.md");
    let holder = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .open(&agents)
        .unwrap();

    let error = init_through_the_port(init_input(&repo, &["new-reviewer"]), || {}).unwrap_err();
    drop(holder);

    let report = format!("{error:#}");
    assert!(
        report.starts_with(&format!(
            "failed to replace {}",
            agents.as_std_path().display()
        )),
        "{report}"
    );
    assert!(report.contains("os error"), "{report}");
    assert_eq!(std::fs::read_to_string(&agents).unwrap(), PERSON_AGENTS);
    assert_eq!(reviewers(&repo), ["reviewer"]);
    assert_eq!(temporary_files(&repo), Vec::<String>::new());
}
