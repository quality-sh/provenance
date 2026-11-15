//! A process stops during a second init at each point of the replacement of
//! each project file. Each project file then holds its bytes from before
//! init or the bytes that init planned, and the next init gives the same
//! project as an init that did not stop.
//!
//! The stop is real: a child process runs `InitPort::init`, the port that
//! the CLI calls, and exits at the n-th time that init comes to a point.
use super::{init_input, init_through_the_port, targets};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::verifies;
use std::collections::BTreeMap;

const ROOT: &str = "PROVENANCE_REPLACEMENT_STOP_ROOT";
const PDF: &str = "PROVENANCE_REPLACEMENT_STOP_PDF";
const POINT: &str = "PROVENANCE_REPLACEMENT_STOP_POINT";
const STOP_AT: &str = "PROVENANCE_REPLACEMENT_STOP_AT";
const STOPPED: i32 = 86;
const FINISHED: i32 = 87;
/// More replacements than a second init of this repository makes.
const STOP_LIMIT: usize = 20;
const CHILD: &str =
    "porcelain::init::lock_tests::replacement_stop_tests::stopped_replacement_child";
/// The points of one replacement: the temporary file has the planned bytes,
/// the compare with the target is done, and the target has the planned bytes.
const POINTS: [&str; 3] = [
    "project_file_prepared",
    "project_file_compared",
    "project_file_replaced",
];
const SKILL: &str = ".agents/skills/provenance-shaping/SKILL.md";
const DICTIONARY: &str = ".provenance/state/dictionary.json";
/// The project files that the second init replaces, in its write order.
const REPLACED: [&str; 4] = [SKILL, "AGENTS.md", ".gitignore", DICTIONARY];
const PERSON_AGENTS: &str = "# Team notes\n\nKeep this text of the person.\n";
const PERSON_GITIGNORE: &str = "target/\n";
const PERSON_TERM: &str = "pump";
/// Files of the person with names near the names of the temporary files of
/// init.
const NEAR_NAMES: [&str; 3] = [
    ".AGENTS.md.provenance-notes.tmp",
    ".AGENTS.md.provenance-AbC123.tmp.orig",
    "AGENTS.md.provenance-AbC123.tmp",
];

/// The child process: the second init, which stops at once when it comes to
/// `POINT` for the `STOP_AT`-th time.
#[test]
#[ignore = "the child process of the stop tests of this module"]
fn stopped_replacement_child() {
    let (Ok(root), Ok(pdf), Ok(point), Ok(stop_at)) = (
        std::env::var(ROOT),
        std::env::var(PDF),
        std::env::var(POINT),
        std::env::var(STOP_AT),
    ) else {
        return;
    };
    let point = POINTS.into_iter().find(|known| *known == point).unwrap();
    let stop_at: usize = stop_at.parse().unwrap();
    let input = second_init_input(Utf8Path::new(&root), Utf8Path::new(&pdf));
    let result = init_through_the_port(input, move || {
        let mut count = 0;
        crate::test_probes::arm(point, move || {
            count += 1;
            if count == stop_at {
                std::process::exit(STOPPED);
            }
        });
    });
    match result {
        Ok(_) => std::process::exit(FINISHED),
        Err(error) => panic!("the stopped init failed: {error:#}"),
    }
}

/// The second init: it changes the reviewer list and imports the dictionary
/// PDF at `pdf`.
fn second_init_input(repo: &Utf8Path, pdf: &Utf8Path) -> provenance_porcelain::init::InitInput {
    let mut input = init_input(repo, &["new-reviewer"]);
    input.ste_pdf = Some(pdf.to_string());
    input
}

/// The synthetic dictionary PDF, in a file of `directory`.
fn dictionary_pdf(directory: &tempfile::TempDir) -> Utf8PathBuf {
    let path = Utf8PathBuf::from_path_buf(directory.path().join("issue-9.pdf")).unwrap();
    std::fs::write(&path, crate::fixture_pdf::dictionary_pdf()).unwrap();
    path
}

/// An initialized repository in which a second init replaces four project
/// files. The person wrote AGENTS.md and .gitignore, one skill file has the
/// stamp of an older release, and the dictionary reference names another
/// dictionary and has a technical name of the person.
fn repository_with_person_files(pdf: &Utf8Path) -> (tempfile::TempDir, Utf8PathBuf) {
    let directory = tempfile::tempdir().unwrap();
    let repo = Utf8PathBuf::from_path_buf(directory.path().join("repo")).unwrap();
    std::fs::create_dir_all(&repo).unwrap();
    let mut first = init_input(&repo, &["reviewer"]);
    first.ste_pdf = Some(pdf.to_string());
    init_through_the_port(first, || {}).unwrap();

    std::fs::write(repo.join("AGENTS.md"), PERSON_AGENTS).unwrap();
    std::fs::write(repo.join(".gitignore"), PERSON_GITIGNORE).unwrap();
    let skill = std::fs::read_to_string(repo.join(SKILL)).unwrap();
    let stamp = format!("Installed by provenance {};", env!("CARGO_PKG_VERSION"));
    assert!(skill.contains(&stamp), "{skill}");
    std::fs::write(
        repo.join(SKILL),
        skill.replace(&stamp, "Installed by provenance 0.2.1;"),
    )
    .unwrap();
    let mut reference: serde_json::Value =
        serde_json::from_slice(&std::fs::read(repo.join(DICTIONARY)).unwrap()).unwrap();
    reference["source_sha256"] = "0".repeat(64).into();
    reference["data_sha256"] = "0".repeat(64).into();
    reference["technical_names"] = serde_json::json!([PERSON_TERM]);
    let mut bytes = serde_json::to_vec_pretty(&reference).unwrap();
    bytes.push(b'\n');
    std::fs::write(repo.join(DICTIONARY), bytes).unwrap();
    for name in NEAR_NAMES {
        std::fs::write(repo.join(name), "a file of the person\n").unwrap();
    }
    (directory, repo)
}

/// Runs the second init in a child process that stops at the `stop_at`-th
/// time at `point`. Returns `false` when init came to `point` fewer times and
/// ended.
fn stop_second_init(repo: &Utf8Path, pdf: &Utf8Path, point: &str, stop_at: usize) -> bool {
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([CHILD, "--exact", "--ignored", "--nocapture"])
        .env(ROOT, repo.as_str())
        .env(PDF, pdf.as_str())
        .env(POINT, point)
        .env(STOP_AT, stop_at.to_string())
        .output()
        .unwrap();
    match output.status.code() {
        Some(STOPPED) => true,
        Some(FINISHED) => false,
        other => panic!(
            "the second init failed before stop {stop_at} at {point}: {other:?}\n{}",
            String::from_utf8_lossy(&output.stdout)
        ),
    }
}

/// Whether `name` has the name form of a temporary file of init for one of
/// the replaced files: `.<file name>.provenance-`, six letters or digits,
/// and `.tmp`, in the folder of that file.
fn is_init_temporary(name: &str) -> bool {
    REPLACED.iter().any(|target| {
        let target = Utf8Path::new(target);
        let folder = target.parent().unwrap().as_str();
        let (folder_of_name, leaf) = name.rsplit_once('/').unwrap_or(("", name));
        let prefix = format!(".{}.provenance-", target.file_name().unwrap());
        folder_of_name == folder
            && leaf.strip_prefix(&prefix).is_some_and(|rest| {
                rest.strip_suffix(".tmp").is_some_and(|random| {
                    random.len() == 6 && random.bytes().all(|byte| byte.is_ascii_alphanumeric())
                })
            })
    })
}

/// A short account of `tree` for the test log: each path that is not in
/// `before` and `after`, and for each replaced file whether it is old, new,
/// or neither.
fn account(
    tree: &BTreeMap<String, Option<Vec<u8>>>,
    before: &BTreeMap<String, Option<Vec<u8>>>,
    after: &BTreeMap<String, Option<Vec<u8>>>,
) -> String {
    let mut lines = Vec::new();
    for target in REPLACED {
        let state = match tree.get(target) {
            None => "away",
            found if found == before.get(target) => "old bytes",
            found if found == after.get(target) => "planned bytes",
            Some(_) => "other bytes",
        };
        lines.push(format!("  {target}: {state}"));
    }
    for (name, bytes) in tree {
        if !before.contains_key(name) && !after.contains_key(name) {
            let size = bytes.as_ref().map(Vec::len);
            lines.push(format!("  more: {name} ({size:?} bytes)"));
        }
    }
    lines.join("\n")
}

/// Stops a second init at each time it comes to `point`, in a new copy of
/// the repository for each stop, and checks the state after the stop and
/// after the next init.
fn stop_at_each_replacement(point: &str) {
    let pdf_directory = tempfile::tempdir().unwrap();
    let pdf = dictionary_pdf(&pdf_directory);
    let (_reference_directory, reference) = repository_with_person_files(&pdf);
    let before = targets(&reference);
    init_through_the_port(second_init_input(&reference, &pdf), || {}).unwrap();
    let after = targets(&reference);
    for target in REPLACED {
        assert_ne!(before[target], after[target], "init did not replace {target}");
    }
    for name in NEAR_NAMES {
        assert_eq!(after.get(name), before.get(name), "init changed {name}");
    }

    let mut stops = 0;
    for stop_at in 1..=STOP_LIMIT {
        let (_directory, repo) = repository_with_person_files(&pdf);
        if !stop_second_init(&repo, &pdf, point, stop_at) {
            break;
        }
        stops += 1;
        let stopped = targets(&repo);
        eprintln!(
            "=== stop {stop_at} at {point}\n{}",
            account(&stopped, &before, &after)
        );

        for name in before.keys().chain(after.keys()) {
            let found = stopped.get(name);
            assert!(
                found == before.get(name) || found == after.get(name),
                "after stop {stop_at} at {point}, {name} has neither its old bytes nor \
                 its planned bytes"
            );
        }
        for name in stopped.keys() {
            assert!(
                before.contains_key(name) || after.contains_key(name) || is_init_temporary(name),
                "after stop {stop_at} at {point}, init left {name}"
            );
        }
        let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
        assert!(agents.starts_with(PERSON_AGENTS), "{agents}");
        let gitignore = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
        assert!(gitignore.starts_with(PERSON_GITIGNORE), "{gitignore}");
        let dictionary = std::fs::read_to_string(repo.join(DICTIONARY)).unwrap();
        assert!(dictionary.contains(PERSON_TERM), "{dictionary}");

        let next = init_through_the_port(second_init_input(&repo, &pdf), || {})
            .unwrap_or_else(|error| {
                panic!("the init after stop {stop_at} at {point} failed: {error:#}")
            });
        eprintln!("--- the next init\n{}", next.summary);
        let resumed = targets(&repo);
        assert!(
            resumed == after,
            "the init after stop {stop_at} at {point} gave another project:\n{}",
            account(&resumed, &before, &after)
        );
    }
    assert_eq!(stops, REPLACED.len(), "the replacements of the second init");
}

#[test]
#[verifies("rule_init_stop_keeps_each_project_file_whole", examples)]
fn a_stop_after_the_temporary_file_and_before_the_compare_keeps_each_file_whole() {
    stop_at_each_replacement("project_file_prepared");
}

#[test]
#[verifies("rule_init_stop_keeps_each_project_file_whole", examples)]
fn a_stop_after_the_compare_and_before_the_rename_keeps_each_file_whole() {
    stop_at_each_replacement("project_file_compared");
}

#[test]
#[verifies("rule_init_stop_keeps_each_project_file_whole", examples)]
fn a_stop_after_the_rename_keeps_each_file_whole() {
    stop_at_each_replacement("project_file_replaced");
}
