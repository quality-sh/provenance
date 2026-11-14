//! The life of the `git cat-file` process of the git adapter.
//!
//! Implementation aid with no Rule binding: no Rule states how long the git
//! adapter keeps a Git process. A `git` first on PATH runs the real Git, and
//! for `cat-file` writes the process id to `.git/cat-file-pids` of the
//! repository. A word in `.git/cat-file-mode` changes what it does:
//! `noisy` writes more than a pipe holds to standard error first, and
//! `short` gives an answer that ends before its object. The script is a
//! shell script, so the tests are Unix-only.
#![cfg(unix)]

#[path = "history_git/repository.rs"]
mod repository;

use provenance_store::review::history::{FileVersions, GitFileVersions};
use repository::repository;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;
use std::time::{Duration, Instant};

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
                "#!/bin/sh\ncase \" $* \" in *' cat-file '*)\n\
                 mode=$(cat .git/cat-file-mode 2>/dev/null)\n\
                 echo $$ >> .git/cat-file-pids\n\
                 if [ \"$mode\" = short ]; then read -r name; printf 'x blob 10\\nabc'; exit 0; fi\n\
                 if [ \"$mode\" = noisy ]; then head -c 200000 /dev/zero | tr '\\000' e >&2; fi;;\n\
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

/// The last `cat-file` process that started in the repository at `root`.
fn last_pid(root: &Path) -> String {
    let pids = std::fs::read_to_string(root.join(".git/cat-file-pids")).unwrap();
    pids.lines().last().unwrap().to_owned()
}

/// A process that ended and that its parent waited for is gone; a process
/// that ended with no wait stays as a zombie, which `kill -0` still finds.
fn running(pid: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("kill -0 {pid} 2>/dev/null")])
        .status()
        .unwrap()
        .success()
}

/// The text of `good.jsonl` at `commit`, read on its own thread, or `None`
/// when the read does not end in 30 seconds.
fn text_or_hang(versions: std::sync::Arc<GitFileVersions>, commit: String) -> Option<String> {
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let text = runtime.block_on(versions.text("good.jsonl", &commit));
        let _ = sender.send(text.unwrap());
    });
    receiver
        .recv_timeout(Duration::from_secs(30))
        .ok()
        .flatten()
}

#[tokio::test]
async fn a_dropped_adapter_stops_its_process_at_once() {
    logging_git();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    let versions = GitFileVersions::new(root);
    let text = versions.text("good.jsonl", &commit).await.unwrap();
    assert_eq!(text.as_deref(), Some("good\n"));
    let pid = last_pid(temp.path());
    assert!(running(&pid), "the process stopped before the drop");
    drop(versions);
    assert!(!running(&pid), "the process {pid} stays after the drop");
}

#[tokio::test]
async fn an_idle_process_stops_while_its_adapter_is_held() {
    logging_git();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    let versions = GitFileVersions::new(root);
    versions.text("good.jsonl", &commit).await.unwrap();
    let pid = last_pid(temp.path());
    let deadline = Instant::now() + Duration::from_secs(30);
    while running(&pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!running(&pid), "the idle process {pid} stays");
    let text = versions.text("good.jsonl", &commit).await.unwrap();
    assert_eq!(text.as_deref(), Some("good\n"));
    assert_ne!(last_pid(temp.path()), pid, "no new process started");
}

/// The adapter reads the standard error of the process while the process
/// runs, so the process does not wait for it to be read.
#[tokio::test]
async fn a_process_that_writes_much_to_standard_error_still_answers() {
    logging_git();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    std::fs::write(temp.path().join(".git/cat-file-mode"), "noisy").unwrap();
    let versions = std::sync::Arc::new(GitFileVersions::new(root));
    assert_eq!(
        text_or_hang(versions, commit).as_deref(),
        Some("good\n"),
        "the read did not end"
    );
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
