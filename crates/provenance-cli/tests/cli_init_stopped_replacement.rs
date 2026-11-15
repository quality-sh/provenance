//! What a person sees when an init stopped before the rename of AGENTS.md:
//! AGENTS.md has its old bytes, and the temporary file of init is next to
//! it. The CLI cannot stop init, so the test makes that state by hand, with
//! the name form of the temporary file of init.
use provenance_macros::verifies;
use std::path::Path;
use std::process::Command;

const PERSON_AGENTS: &str = "# Team notes\n\nKeep this text of the person.\n";
/// The temporary file of the stopped init: `.AGENTS.md.provenance-`, six
/// letters or digits, and `.tmp`. A stop in the write leaves a part of the
/// planned bytes.
const STOPPED_TEMPORARY: &str = ".AGENTS.md.provenance-Xy12Z9.tmp";
/// Files of the person with names near the name of the temporary file.
const NEAR_NAMES: [&str; 3] = [
    ".AGENTS.md.provenance-notes.tmp",
    ".AGENTS.md.provenance-AbC123.tmp.orig",
    "AGENTS.md.provenance-AbC123.tmp",
];

/// Runs `provenance init` in `repo`, and gives the exit code and the text.
fn init(repo: &Path, args: &[&str]) -> (i32, String) {
    let output = Command::new(assert_cmd::cargo::cargo_bin!("provenance"))
        .args(["init", "--path", repo.to_str().unwrap()])
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    let code = output.status.code().unwrap_or(-1);
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    eprintln!(
        "$ provenance init {}\n{text}[exit {code}]\n",
        args.join(" ")
    );
    (code, text)
}

/// The names in the folder `repo` that have `.provenance-` in them.
fn names_near_init_files(repo: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(repo)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| name.contains(".provenance-"))
        .collect();
    names.sort();
    names
}

/// The next init removes the temporary file that the stopped init left,
/// keeps the files of the person with near names, and replaces AGENTS.md as
/// an init that did not stop does.
#[test]
#[verifies("rule_init_stop_keeps_each_project_file_whole", examples)]
fn the_next_init_removes_the_temporary_file_of_a_stopped_init() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    let (code, text) = init(&repo, &["--disposition-actor-id", "reviewer"]);
    assert_eq!(code, 0, "{text}");
    std::fs::write(repo.join("AGENTS.md"), PERSON_AGENTS).unwrap();
    std::fs::write(
        repo.join(STOPPED_TEMPORARY),
        "# Team notes\n\nKeep this text of the person.\n\n## Prov",
    )
    .unwrap();
    for name in NEAR_NAMES {
        std::fs::write(repo.join(name), "a file of the person\n").unwrap();
    }

    let (code, text) = init(&repo, &[]);

    assert_eq!(code, 0, "{text}");
    assert!(
        text.contains("Changed\n  AGENTS.md (added the Provenance section)\n"),
        "{text}"
    );
    let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(agents.starts_with(PERSON_AGENTS), "{agents}");
    assert!(agents.contains("## Provenance"), "{agents}");
    let mut near = NEAR_NAMES.map(str::to_owned).to_vec();
    near.sort();
    assert_eq!(names_near_init_files(&repo), near);
    for name in NEAR_NAMES {
        assert_eq!(
            std::fs::read_to_string(repo.join(name)).unwrap(),
            "a file of the person\n"
        );
    }
}

/// Sets the read-only attribute of the file at `path`.
#[cfg(windows)]
fn set_read_only(path: &Path) {
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).unwrap();
}

/// On Windows, a stop after init set the permissions of a read-only
/// AGENTS.md on its temporary file leaves a read-only temporary file. The
/// next init removes it and replaces AGENTS.md.
#[cfg(windows)]
#[test]
#[verifies("rule_init_stop_keeps_each_project_file_whole", examples)]
fn the_next_init_removes_a_read_only_temporary_file_of_a_stopped_init() {
    let directory = tempfile::tempdir().unwrap();
    let repo = directory.path().join("repo");
    let (code, text) = init(&repo, &["--disposition-actor-id", "reviewer"]);
    assert_eq!(code, 0, "{text}");
    std::fs::write(repo.join("AGENTS.md"), PERSON_AGENTS).unwrap();
    set_read_only(&repo.join("AGENTS.md"));
    std::fs::write(
        repo.join(STOPPED_TEMPORARY),
        "# Team notes\n\nKeep this text of the person.\n\n## Prov",
    )
    .unwrap();
    set_read_only(&repo.join(STOPPED_TEMPORARY));

    let (code, text) = init(&repo, &[]);

    assert_eq!(code, 0, "{text}");
    assert_eq!(names_near_init_files(&repo), Vec::<String>::new());
    let agents = std::fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(agents.starts_with(PERSON_AGENTS), "{agents}");
    assert!(agents.contains("## Provenance"), "{agents}");
    let metadata = std::fs::metadata(repo.join("AGENTS.md")).unwrap();
    assert!(metadata.permissions().readonly(), "AGENTS.md is not read-only");
}
