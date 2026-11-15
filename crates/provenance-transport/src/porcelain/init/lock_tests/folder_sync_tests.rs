//! A sync of the folder of AGENTS.md that fails after the rename of a second
//! init. Init runs through `InitPort::init`, the port that the CLI calls. A
//! probe takes the read permission from the folder after the compare, so
//! that the rename works and the sync of the folder fails, and gives the
//! permission back after the rename. The CLI cannot make the sync fail at
//! that point.
use super::{
    init_input, init_through_the_port, repository_with_changed_agents, reviewers,
    temporary_files, PERSON_AGENTS,
};
use camino::Utf8Path;
use provenance_macros::verifies;
use std::os::unix::fs::PermissionsExt;

/// A second init in which the sync of the folder of AGENTS.md fails after
/// each of the first `failures` renames of AGENTS.md. Init prints the error
/// as the CLI does.
fn init_with_failed_folder_syncs(
    repo: &Utf8Path,
    failures: usize,
) -> anyhow::Result<provenance_porcelain::init::InitReport> {
    let folder = repo.to_owned();
    let ended = init_through_the_port(init_input(repo, &["new-reviewer"]), move || {
        let closed = folder.clone();
        let mut renames = 0;
        crate::test_probes::arm("project_file_compared", move || {
            renames += 1;
            if renames <= failures {
                std::fs::set_permissions(&closed, std::fs::Permissions::from_mode(0o300)).unwrap();
            }
        });
        let opened = folder.clone();
        crate::test_probes::arm("project_file_replaced", move || {
            std::fs::set_permissions(&opened, std::fs::Permissions::from_mode(0o755)).unwrap();
        });
    });
    if let Err(error) = &ended {
        eprintln!("Error: {error:?}");
    }
    ended
}

/// The text of the failed sync of `folder`.
fn failed_sync(folder: &Utf8Path) -> String {
    format!(
        "sync {}: {}",
        folder.as_std_path().display(),
        std::io::Error::from_raw_os_error(13)
    )
}

fn assert_restored(repo: &Utf8Path) {
    assert_eq!(
        std::fs::read_to_string(repo.join("AGENTS.md")).unwrap(),
        PERSON_AGENTS
    );
    assert_eq!(reviewers(repo), ["reviewer"]);
    assert_eq!(temporary_files(repo), Vec::<String>::new());
}

/// The folder sync fails after AGENTS.md was replaced: init says that the
/// file was replaced and that its folder was not synced, and the rollback
/// restores AGENTS.md and the manifest.
#[test]
#[verifies("rule_init_apply_rolls_back_owned_changes", examples)]
fn a_failed_folder_sync_after_the_rename_restores_the_file() {
    let (_directory, repo) = repository_with_changed_agents();

    let error = init_with_failed_folder_syncs(&repo, 1).unwrap_err();

    let agents = repo.join("AGENTS.md");
    assert_eq!(
        format!("{error:#}"),
        format!(
            "{} was replaced, but its folder was not synced: {}",
            agents.as_std_path().display(),
            failed_sync(&repo)
        )
    );
    assert_restored(&repo);
}

/// The folder sync also fails after the rollback restored AGENTS.md: init
/// says that the rollback failed for AGENTS.md because its folder was not
/// synced, and AGENTS.md has its old bytes.
#[test]
#[verifies("rule_init_apply_rolls_back_owned_changes", examples)]
fn a_failed_folder_sync_of_the_restore_is_a_rollback_failure() {
    let (_directory, repo) = repository_with_changed_agents();

    let error = init_with_failed_folder_syncs(&repo, 2).unwrap_err();

    let agents = repo.join("AGENTS.md").as_std_path().display().to_string();
    let sync = failed_sync(&repo);
    assert_eq!(
        format!("{error:#}"),
        format!(
            "repository initialization rollback failed: initialization rollback failure(s):\n\
             - {agents}: {agents} was restored, but its folder was not synced: {sync}: \
             {agents} was replaced, but its folder was not synced: {sync}"
        )
    );
    assert_restored(&repo);
}
