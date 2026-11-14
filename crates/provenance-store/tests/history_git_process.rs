//! The `git cat-file` process of the git adapter, seen through a `git` that
//! logs it.
//!
//! Implementation aid with no Rule binding: no Rule states when the git
//! adapter starts a Git process. A `git` first on PATH writes its arguments
//! to `.git/git-starts` of the repository, then runs the real Git. When
//! `.git/cat-file-mode` holds `short`, it gives a `cat-file` answer that
//! ends before its object. The real Git cannot give such an answer, and the
//! process list cannot show a process that started and stopped between two
//! looks. The script is a shell script, so the tests are Unix-only.
//! `history_git_children` tests the life of the process on each platform.
#![cfg(unix)]

#[path = "history_git/repository.rs"]
mod repository;

use provenance_store::review::history::{FileVersions, GitFileVersions};
use repository::repository;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

/// Puts the logging `git` first on PATH, once for the test binary, before
/// any test starts Git.
fn logging_git() {
    static TOOLS: std::sync::OnceLock<tempfile::TempDir> = std::sync::OnceLock::new();
    TOOLS.get_or_init(|| {
        let real = std::process::Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap();
        let real = String::from_utf8(real.stdout).unwrap();
        let real = real.trim();
        let tools = tempfile::tempdir().unwrap();
        let wrapper = tools.path().join("git");
        std::fs::write(
            &wrapper,
            format!(
                "#!/bin/sh\necho \"$*\" 2>/dev/null >> .git/git-starts\n\
                 case \" $* \" in *' cat-file '*)\n\
                 if [ \"$(cat .git/cat-file-mode 2>/dev/null)\" = short ]; then\n\
                 read -r name; printf 'x blob 10\\nabc'; exit 0; fi;;\n\
                 esac\nexec '{real}' \"$@\"\n"
            ),
        )
        .unwrap();
        std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        let path = std::env::join_paths(
            std::iter::once(tools.path().to_path_buf()).chain(std::env::split_paths(&path)),
        )
        .unwrap();
        std::env::set_var("PATH", path);
        tools
    });
}

/// The number of Git processes that started in the repository at `root`
/// and ran to their first command. A process that the adapter stops at once
/// can stop before that.
fn starts(root: &Path) -> usize {
    std::fs::read_to_string(root.join(".git/git-starts")).map_or(0, |starts| starts.lines().count())
}

#[tokio::test]
async fn a_refused_name_starts_no_process() {
    logging_git();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    let before = starts(temp.path());
    let versions = GitFileVersions::new(root);
    for path in ["go\nod.jsonl", "good.jsonl\r"] {
        assert!(versions.text(path, &commit).await.is_err(), "{path:?}");
    }
    assert_eq!(starts(temp.path()), before, "a refused name started Git");
    let text = versions.text("good.jsonl", &commit).await.unwrap();
    assert_eq!(text.as_deref(), Some("good\n"));
    assert!(starts(temp.path()) > before, "the log has no Git start");
}

#[tokio::test]
async fn an_answer_that_ends_before_its_object_is_refused() {
    logging_git();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    std::fs::write(temp.path().join(".git/cat-file-mode"), "short").unwrap();
    let versions = GitFileVersions::new(root);
    let error = versions.text("good.jsonl", &commit).await.unwrap_err();
    assert_eq!(format!("{error:#}"), "git cat-file output ended early");
}
