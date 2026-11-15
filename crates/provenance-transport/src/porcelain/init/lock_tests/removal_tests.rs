//! The removal of a legacy skill file by a second init: a compare of the
//! file with the plan, then one remove and a sync of its folder, with no
//! backup name. A stop is real: a child process runs `InitPort::init`, the
//! port that the CLI calls, and exits when init comes to a point.
use super::{init_input, init_through_the_port, tree};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;

const ROOT: &str = "PROVENANCE_REMOVAL_STOP_ROOT";
const POINT: &str = "PROVENANCE_REMOVAL_STOP_POINT";
const STOPPED: i32 = 86;
const FINISHED: i32 = 87;
const CHILD: &str = "porcelain::init::lock_tests::removal_tests::stopped_removal_child";
/// The points of the removal: the compare is done, and the file is removed.
const POINTS: [&str; 2] = ["project_file_removal_compared", "project_file_removed"];
/// The folder of the legacy skill, one name for each part, so that a path
/// has the separators of the system, as the paths of init have.
const LEGACY_FOLDER: [&str; 3] = [".agents", "skills", "shaping"];
const EDIT: &str = "An edit of the person.\n";

/// The child process: the second init, which stops at once when it comes to
/// `POINT`.
#[test]
#[ignore = "the child process of the stop tests of this module"]
fn stopped_removal_child() {
    let (Ok(root), Ok(point)) = (std::env::var(ROOT), std::env::var(POINT)) else {
        return;
    };
    let point = POINTS.into_iter().find(|known| *known == point).unwrap();
    let input = init_input(Utf8Path::new(&root), &["reviewer"]);
    let result = init_through_the_port(input, move || {
        crate::test_probes::arm(point, || std::process::exit(STOPPED));
    });
    match result {
        Ok(_) => std::process::exit(FINISHED),
        Err(error) => panic!("the stopped init failed: {error:#}"),
    }
}

/// The folder of the legacy skill in `repo`.
fn legacy_folder_path(repo: &Utf8Path) -> Utf8PathBuf {
    LEGACY_FOLDER
        .iter()
        .fold(repo.to_owned(), |path, part| path.join(part))
}

/// The legacy skill file in `repo`.
fn legacy_path(repo: &Utf8Path) -> Utf8PathBuf {
    legacy_folder_path(repo).join("SKILL.md")
}

/// The text of a skill file that an older release installed with its stamp.
fn legacy_skill() -> String {
    let frontmatter = "---\nname: shaping\ndescription: An old skill.\n---\n";
    let payload = "Shape the work.\n";
    let header = crate::skills::stamp::header(&format!("{frontmatter}{payload}"));
    format!("{frontmatter}{header}\n{payload}")
}

/// An initialized repository with a legacy skill file that the next init
/// removes, and a file of the person next to it.
fn repository_with_legacy_skill() -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    init_through_the_port(init_input(&repo, &["reviewer"]), || {}).unwrap();
    std::fs::create_dir_all(legacy_folder_path(&repo)).unwrap();
    std::fs::write(legacy_path(&repo), legacy_skill()).unwrap();
    // A file of the person in the folder of the legacy skill, so that the
    // folder stays after the removal.
    std::fs::write(
        legacy_folder_path(&repo).join("notes.md"),
        "notes of the person\n",
    )
    .unwrap();
    (directory, repo)
}

/// Runs the second init in a child process that stops at `point`.
fn stop_second_init(repo: &Utf8Path, point: &str) {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([CHILD, "--exact", "--ignored", "--nocapture"])
        .env(ROOT, repo.as_str())
        .env(POINT, point)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(STOPPED),
        "the second init did not stop at {point}\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
}

/// The files in the folder of the legacy skill.
fn legacy_folder(repo: &Utf8Path) -> Vec<String> {
    let mut names: Vec<String> = legacy_folder_path(repo)
        .read_dir_utf8()
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_owned())
        .collect();
    names.sort();
    names
}

/// The paths of `repo` with `.provenance-` in the name.
fn init_names(repo: &Utf8Path) -> Vec<String> {
    tree(repo)
        .into_keys()
        .filter(|name| name.contains(".provenance-"))
        .collect()
}

/// A stop after the compare and before the remove keeps the legacy skill
/// file with its bytes, and the next init removes it.
#[test]
#[verifies("rule_init_stop_keeps_each_project_file_whole", examples)]
fn a_stop_before_the_remove_keeps_the_file() {
    let (_directory, repo) = repository_with_legacy_skill();

    stop_second_init(&repo, "project_file_removal_compared");

    assert_eq!(
        std::fs::read_to_string(legacy_path(&repo)).unwrap(),
        legacy_skill()
    );
    assert_eq!(legacy_folder(&repo), ["SKILL.md", "notes.md"]);
    assert_eq!(init_names(&repo), Vec::<String>::new());
    init_through_the_port(init_input(&repo, &["reviewer"]), || {}).unwrap();
    assert_eq!(legacy_folder(&repo), ["notes.md"]);
}

/// A stop after the remove leaves no legacy skill file and no other name for
/// it.
#[test]
#[verifies("rule_init_stop_keeps_each_project_file_whole", examples)]
fn a_stop_after_the_remove_leaves_no_file_and_no_other_name() {
    let (_directory, repo) = repository_with_legacy_skill();

    stop_second_init(&repo, "project_file_removed");

    assert_eq!(legacy_folder(&repo), ["notes.md"]);
    assert_eq!(init_names(&repo), Vec::<String>::new());
    init_through_the_port(init_input(&repo, &["reviewer"]), || {}).unwrap();
    assert_eq!(legacy_folder(&repo), ["notes.md"]);
}

/// An edit of the legacy skill file after the recheck and before the compare
/// of the removal: init refuses, and the file keeps the edit.
#[test]
#[verifies("rule_init_refuses_a_project_file_change_before_its_compare", examples)]
fn an_edit_before_the_compare_of_the_removal_is_refused_and_kept() {
    let (_directory, repo) = repository_with_legacy_skill();
    let legacy = legacy_path(&repo);

    let edited = legacy.clone();
    let error = init_through_the_port(init_input(&repo, &["reviewer"]), move || {
        let edited = edited.clone();
        crate::test_probes::arm("project_file_removal_checked", move || {
            std::fs::write(&edited, EDIT).unwrap();
        });
    })
    .unwrap_err();

    assert_eq!(
        format!("{error:#}"),
        format!("{} changed during removal", legacy.as_std_path().display())
    );
    assert_eq!(std::fs::read_to_string(&legacy).unwrap(), EDIT);
    assert_eq!(legacy_folder(&repo), ["SKILL.md", "notes.md"]);
    assert_eq!(init_names(&repo), Vec::<String>::new());
}

/// An edit of the legacy skill file after the compare and before the remove:
/// init removes the edited file and ends with success.
/// This test has no Rule binding: the description of the Rule
/// `rule_init_refuses_a_project_file_change_before_its_compare` states this
/// limit, and the test pins the limit, not an obligation.
#[test]
fn an_edit_after_the_compare_of_the_removal_is_removed() {
    let (_directory, repo) = repository_with_legacy_skill();

    let edited = legacy_path(&repo);
    init_through_the_port(init_input(&repo, &["reviewer"]), move || {
        let edited = edited.clone();
        crate::test_probes::arm("project_file_removal_compared", move || {
            std::fs::write(&edited, EDIT).unwrap();
        });
    })
    .unwrap();

    assert_eq!(legacy_folder(&repo), ["notes.md"]);
    assert_eq!(init_names(&repo), Vec::<String>::new());
}

/// On Windows, init removes a read-only legacy skill file.
/// This test has no Rule binding: it checks a platform behaviour of the
/// removal.
#[cfg(windows)]
#[test]
fn a_read_only_legacy_skill_file_is_removed() {
    let (_directory, repo) = repository_with_legacy_skill();
    let legacy = legacy_path(&repo);
    let mut permissions = std::fs::metadata(&legacy).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&legacy, permissions).unwrap();

    init_through_the_port(init_input(&repo, &["reviewer"]), || {}).unwrap();

    assert_eq!(legacy_folder(&repo), ["notes.md"]);
    assert_eq!(init_names(&repo), Vec::<String>::new());
}
