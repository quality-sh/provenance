//! The life of the `git cat-file` process of the git adapter, seen in the
//! process list.
//!
//! Implementation aid with no Rule binding: no Rule states how long the git
//! adapter keeps a Git process. The tests run the real Git on each platform,
//! and find the process of the adapter among the Git child processes of the
//! test process. Each test holds `SERIAL` while it runs, so no other test of
//! this binary has a Git child at that time.

#[path = "history_git_support/children.rs"]
mod children;
#[path = "history_git_support/repository.rs"]
mod repository;

use camino::Utf8Path;
use children::{git_children, hold};
use provenance_store::review::history::{FileVersions, GitFileVersions};
use repository::repository;
use std::fmt::Write as _;
use std::process::Stdio;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The idle stop ends a process no sooner than two seconds after the first
/// text of its adapter, so a process that is gone sooner did not stop idle.
const IDLE_FLOOR: Duration = Duration::from_secs(2);

/// How many times a test reads again when the idle stop can have ended the
/// process first, because the test runner stalled.
const ATTEMPTS: usize = 10;

fn text(versions: &GitFileVersions, path: &str, commit: &str) -> anyhow::Result<Option<String>> {
    tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(versions.text(path, commit))
}

/// A new adapter that read `good.jsonl`, the id of its process, and the
/// time before the read. `None` when the process was gone before the
/// process list was read.
fn adapter_with_process(root: &Utf8Path, commit: &str) -> Option<(GitFileVersions, u32, Instant)> {
    let start = Instant::now();
    let versions = GitFileVersions::new(root.to_owned());
    let good = text(&versions, "good.jsonl", commit).unwrap();
    assert_eq!(good.as_deref(), Some("good\n"));
    match git_children()[..] {
        [pid] => Some((versions, pid, start)),
        [] => None,
        ref more => panic!("more than one Git process: {more:?}"),
    }
}

/// Retries `attempt` while it gives `None`.
fn retried(mut attempt: impl FnMut() -> Option<()>) {
    assert!(
        (0..ATTEMPTS).any(|_| attempt().is_some()),
        "the idle stop came first in each of {ATTEMPTS} attempts"
    );
}

/// The process of an adapter is gone after the drop of the adapter.
///
/// On Unix a process that ended stays in the process list until its parent
/// waits for it, so the test fails when the drop does not stop the process
/// and wait for it. On Windows a process is gone from the list when it ends,
/// and Git ends soon after its standard input closes. So on Windows the test
/// shows that the process is gone soon after the drop, not that the drop
/// stopped it. A leaked adapter fails the test on each platform, except when
/// the runner stalls for two seconds or more and the idle stop comes first.
#[test]
fn a_dropped_adapter_stops_its_process() {
    let _serial = serial();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    retried(|| {
        let (versions, pid, _) = adapter_with_process(&root, &commit)?;
        drop(versions);
        // The drop stopped the process, or the idle stop did before it.
        assert!(
            !git_children().contains(&pid),
            "the process {pid} stays after the drop"
        );
        Some(())
    });
}

/// A Git child that ended and that its parent waited for is not a Git child
/// of the test, also while another process holds it. The adapter waits for
/// its process, but on Windows the host of the console of the process can
/// hold the process after the wait.
#[test]
fn an_ended_child_that_another_hold_keeps_is_not_listed() {
    let _serial = serial();
    let temp = tempfile::tempdir().unwrap();
    let (root, _) = repository(&temp, &[("good.jsonl", "good\n")]);
    let mut child = provenance_store::git_process::git()
        .args(["cat-file", "--batch"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let pid = child.id();
    assert_eq!(git_children(), [pid], "the running child is not listed");
    let _hold = hold(pid);
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(
        !git_children().contains(&pid),
        "the process {pid} ended, but it is listed"
    );
}

#[test]
fn an_idle_process_stops_while_its_adapter_is_held() {
    let _serial = serial();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    retried(|| {
        let (versions, pid, _) = adapter_with_process(&root, &commit)?;
        let deadline = Instant::now() + Duration::from_secs(30);
        while git_children().contains(&pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(
            !git_children().contains(&pid),
            "the idle process {pid} stays"
        );
        let good = text(&versions, "good.jsonl", &commit).unwrap();
        assert_eq!(good.as_deref(), Some("good\n"));
        assert!(!git_children().contains(&pid), "the old process answered");
        Some(())
    });
}

/// A name that the adapter refuses does not stop the process that the
/// adapter holds.
#[test]
fn a_refused_name_keeps_the_process() {
    let _serial = serial();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    retried(|| {
        let (versions, pid, start) = adapter_with_process(&root, &commit)?;
        assert!(text(&versions, "go\nod.jsonl", &commit).is_err());
        let good = text(&versions, "good.jsonl", &commit).unwrap();
        assert_eq!(good.as_deref(), Some("good\n"));
        let children = git_children();
        if start.elapsed() >= IDLE_FLOOR {
            return None;
        }
        assert_eq!(children, [pid], "the refusal stopped the process");
        Some(())
    });
}

/// The adapter reads the standard error of the process while the process
/// runs, so the process does not wait for it to be read. Git writes one
/// error to standard error for each alternate object directory that does
/// not exist, more than a pipe holds.
#[test]
fn a_process_that_writes_much_to_standard_error_still_answers() {
    let _serial = serial();
    let temp = tempfile::tempdir().unwrap();
    let (root, commit) = repository(&temp, &[("good.jsonl", "good\n")]);
    let long = "x".repeat(100);
    let mut missing = String::new();
    for index in 0..2000 {
        writeln!(missing, "missing-{long}-{index}").unwrap();
    }
    std::fs::write(temp.path().join(".git/objects/info/alternates"), missing).unwrap();
    let versions = GitFileVersions::new(root);
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(text(&versions, "good.jsonl", &commit).unwrap());
    });
    let good = receiver.recv_timeout(Duration::from_secs(30));
    assert_eq!(good, Ok(Some("good\n".to_owned())), "the read did not end");
}
