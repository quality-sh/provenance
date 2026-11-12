//! Bead provenance-60t3, wip branch only. These tests show a fault: a process
//! that stops during a second init, between the two moves of a project file,
//! leaves that file away. The test asserts the state that exists, so that it
//! passes and records the fact. No Rule binding: the tests approve nothing.
//!
//! The stop seam is the probe `file_displaced` (`atomic_file.rs`), as in
//! `stop_tests.rs`. A child process runs `InitPort::init` and exits at the
//! n-th move of a file to its backup name.
use super::{init_input, init_through_the_port, tree};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_store::repository::Repository;

const ROOT: &str = "PROVENANCE_60T3_ROOT";
const STOP_AT: &str = "PROVENANCE_60T3_MOVE";
const STOPPED: i32 = 86;
const FINISHED: i32 = 87;
const MOVE_LIMIT: usize = 20;
const CHILD: &str =
    "porcelain::init::lock_tests::project_file_stop_60t3::stopped_init_child_60t3";
const SKILL: &str = ".agents/skills/provenance-shaping/SKILL.md";
const PERSON_AGENTS: &str = "# Team notes\n\nKeep this text of the person.\n";
const PERSON_GITIGNORE: &str = "target/\n";
/// The order in which a second init moves the files of this repository.
const MOVED: [&str; 3] = [SKILL, "AGENTS.md", ".gitignore"];

#[test]
#[ignore = "the child process of the 60t3 stop test"]
fn stopped_init_child_60t3() {
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

/// An initialized repository. The person wrote AGENTS.md and .gitignore, and
/// one skill file has the stamp of an older release, so a second init
/// replaces the three files.
fn repository_with_person_files() -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    init_through_the_port(init_input(&repo, &["reviewer"]), || {}).unwrap();
    std::fs::write(repo.join("AGENTS.md"), PERSON_AGENTS).unwrap();
    std::fs::write(repo.join(".gitignore"), PERSON_GITIGNORE).unwrap();
    let skill = std::fs::read_to_string(repo.join(SKILL)).unwrap();
    let stamp = format!("Installed by provenance {};", env!("CARGO_PKG_VERSION"));
    assert!(skill.contains(&stamp), "{skill}");
    let older = skill.replace(&stamp, "Installed by provenance 0.2.1;");
    std::fs::write(repo.join(SKILL), older).unwrap();
    (directory, repo)
}

fn stop_second_init(repo: &Utf8Path, stop_at: usize) -> bool {
    let status = std::process::Command::new(std::env::current_exe().unwrap())
        .args([CHILD, "--exact", "--ignored", "--nocapture"])
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

/// The names in the folder of `target` that start with `.<file name>.provenance-`
/// and end with `.<kind>`.
fn leftovers(repo: &Utf8Path, target: &str, kind: &str) -> Vec<String> {
    let path = repo.join(target);
    let name = path.file_name().unwrap();
    let prefix = format!(".{name}.provenance-");
    let suffix = format!(".{kind}");
    let mut found: Vec<String> = path
        .parent()
        .unwrap()
        .read_dir_utf8()
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_owned())
        .filter(|item| item.starts_with(&prefix) && item.ends_with(&suffix))
        .collect();
    found.sort();
    found
}

fn sibling(repo: &Utf8Path, target: &str, name: &str) -> Utf8PathBuf {
    repo.join(target).parent().unwrap().join(name)
}

fn reviewers(repo: &Utf8Path) -> anyhow::Result<Vec<String>> {
    Ok(Repository::open(repo, "default")?
        .manifest()?
        .disposition_actor_ids)
}

/// A stop at move n of a second init leaves file n away, its old bytes only
/// in `.<name>.provenance-<pid>-0.backup` and its new bytes in
/// `.<name>.provenance-<pid>-0.tmp`. A read works. The next init exits with
/// success, does not read the backup, and writes the file from nothing: for
/// AGENTS.md and .gitignore, the text of the person is lost from the file.
/// The backup and the temporary file stay.
#[test]
fn fault_60t3_a_stop_between_the_two_moves_leaves_the_project_file_away() {
    let mut stops = 0;
    for stop_at in 1..=MOVE_LIMIT {
        let (_directory, repo) = repository_with_person_files();
        let before: Vec<Vec<u8>> = MOVED
            .iter()
            .map(|target| std::fs::read(repo.join(target)).unwrap())
            .collect();
        if !stop_second_init(&repo, stop_at) {
            break;
        }
        stops += 1;
        let index = stop_at - 1;
        let target = MOVED[index];
        let after_stop = tree(&repo);
        eprintln!("=== 60t3 stop at move {stop_at} ({target})");
        for (name, bytes) in &after_stop {
            if name.contains(".provenance-") || MOVED.contains(&name.as_str()) {
                let size = bytes.as_ref().map(Vec::len);
                eprintln!("  {name} ({size:?} bytes)");
            }
        }

        for other in MOVED.iter().filter(|other| **other != target) {
            assert!(repo.join(other).exists(), "{other} is away at stop {stop_at}");
        }
        assert!(
            !repo.join(target).exists(),
            "the target {target} is present at stop {stop_at}"
        );
        let backups = leftovers(&repo, target, "backup");
        assert_eq!(backups.len(), 1, "{backups:?}");
        assert!(backups[0].ends_with("-0.backup"), "{backups:?}");
        let backup = sibling(&repo, target, &backups[0]);
        assert_eq!(std::fs::read(&backup).unwrap(), before[index]);
        let temporaries = leftovers(&repo, target, "tmp");
        assert_eq!(temporaries.len(), 1, "{temporaries:?}");
        let planned = std::fs::read(sibling(&repo, target, &temporaries[0])).unwrap();
        eprintln!("  backup {}: {:?}", backups[0], String::from_utf8_lossy(&before[index]));
        if target == "AGENTS.md" {
            let text = String::from_utf8(planned.clone()).unwrap();
            assert!(text.starts_with(PERSON_AGENTS), "{text}");
            assert!(text.contains("## Provenance"), "{text}");
        }

        assert_eq!(
            reviewers(&repo).expect("a read after the stop"),
            ["reviewer"],
            "the manifest moved at stop {stop_at}"
        );

        let mut bare = init_input(&repo, &[]);
        bare.scope = None;
        bare.path_prefix = None;
        let report = init_through_the_port(bare, || {}).expect("the next init");
        eprintln!("  next init notices: {:?}", report.notices);
        eprintln!("  next init summary:\n{}", report.summary);
        let rewritten = std::fs::read(repo.join(target)).expect("init wrote the file again");
        eprintln!(
            "  {target} after the next init: {:?}",
            String::from_utf8_lossy(&rewritten)
        );
        match target {
            "AGENTS.md" => {
                let text = String::from_utf8(rewritten).unwrap();
                assert!(!text.contains("Keep this text"), "{text}");
                assert!(text.contains("## Provenance"), "{text}");
                assert!(report.summary.contains("AGENTS.md"), "{}", report.summary);
            }
            ".gitignore" => assert_eq!(rewritten, b".provenance/cache/\n"),
            _ => assert_eq!(rewritten, planned, "the skill file is the planned file"),
        }
        assert_eq!(std::fs::read(&backup).unwrap(), before[index], "backup kept");
        assert_eq!(leftovers(&repo, target, "backup"), backups);
        assert_eq!(leftovers(&repo, target, "tmp"), temporaries);
        assert_eq!(reviewers(&repo).unwrap(), ["reviewer"]);
    }
    assert_eq!(stops, MOVED.len(), "the number of file moves of the second init");
}
