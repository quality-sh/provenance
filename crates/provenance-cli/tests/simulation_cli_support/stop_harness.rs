//! Tests of the harness of a stopped write, with a process that is not the
//! CLI in place of the write.
//!
//! No Rule: these are implementation-aid tests of the test harness. The
//! product does not change, so no Rule of the product applies to them.

use crate::process::stop::{end, End, Made, Point, Watch};
use crate::process::{take_l0_breaks, Running};
use provenance_store::layout::ProvenanceLayout;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The name of a temporary file of a one-key write.
const TEMPORARY: &str = ".state-key.provenance-abc123.tmp";

/// Starts `program` with `args`, with its output in pipes.
fn spawn(program: &str, args: &[&str]) -> Child {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the test starts a process")
}

/// Starts a shell process that runs `unix` on Unix and `windows` on Windows.
fn shell(unix: &str, windows: &str) -> Child {
    if cfg!(windows) {
        spawn("cmd", &["/C", windows])
    } else {
        spawn("sh", &["-c", unix])
    }
}

/// A process that runs for longer than the test. It is not below a shell,
/// so that the kill ends it and the pipes close.
fn hung() -> Child {
    if cfg!(windows) {
        spawn("ping", &["-n", "120", "127.0.0.1"])
    } else {
        spawn("sleep", &["120"])
    }
}

/// Writes the temporary file of a one-key write in the cache of `layout`,
/// and renames it onto its key, so that only the events name it.
fn renamed_temporary(layout: &ProvenanceLayout) {
    let cache = layout.cache_dir();
    let temporary = cache.as_std_path().join(TEMPORARY);
    std::fs::write(&temporary, b"new").expect("the test writes the temporary file");
    std::fs::rename(&temporary, cache.as_std_path().join("key"))
        .expect("the test renames the temporary file");
}

/// Waits until the process of `running` exited.
fn wait_for_exit(running: &mut Running) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !running.ended() {
        assert!(Instant::now() < deadline, "the process did not exit");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_write_that_hangs_after_its_temporary_file_reaches_its_limit() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let layout = ProvenanceLayout::new(root.path().to_str().expect("a UTF-8 path"));
    let watch = Watch::new(&layout);
    renamed_temporary(&layout);
    let mut running = Running::with_limit(hung(), Duration::ZERO);
    assert!(
        watch.reach(&mut running, Point::First).is_none(),
        "the watch gave a point of a write that did not end in its limit"
    );
    let waited = running.wait("hung write");
    assert!(waited.is_err(), "the wait did not report the limit");
    take_l0_breaks();
}

#[test]
fn an_exited_write_whose_temporary_file_only_the_events_name_reaches_its_point() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let layout = ProvenanceLayout::new(root.path().to_str().expect("a UTF-8 path"));
    let watch = Watch::new(&layout);
    renamed_temporary(&layout);
    let mut running = Running::start(shell("exit 0", "exit 0"));
    wait_for_exit(&mut running);
    let reached = watch.reach(&mut running, Point::First);
    let expected = layout.cache_dir().as_std_path().join(TEMPORARY);
    assert!(
        matches!(&reached, Some(Made::Temporary(path)) if *path == expected),
        "the events did not give the temporary file of the write"
    );
    let _ = running.kill("exited write");
    take_l0_breaks();
}

#[test]
fn a_write_that_exits_with_no_answer_after_its_point_is_no_landing() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let layout = ProvenanceLayout::new(root.path().to_str().expect("a UTF-8 path"));
    let watch = Watch::new(&layout);
    // The temporary file stays, so the first look sees the point while the
    // write runs; the write then exits with no answer before the kill.
    let temporary = layout.cache_dir().as_std_path().join(TEMPORARY);
    std::fs::write(&temporary, b"new").expect("the test writes the temporary file");
    let mut running = Running::start(shell(
        "sleep 1; exit 3",
        "ping -n 2 127.0.0.1 >NUL & exit 3",
    ));
    let reached = watch.reach(&mut running, Point::First);
    assert!(
        matches!(reached, Some(Made::Temporary(_))),
        "the watch did not see the temporary file"
    );
    assert!(
        !running.ended(),
        "the write exited before the test saw its point"
    );
    wait_for_exit(&mut running);
    take_l0_breaks();
    let ended = end(running, "write with no answer");
    assert!(
        matches!(ended, End::Exited(_)),
        "a write that exited with no answer did not give Exited"
    );
    assert_eq!(
        take_l0_breaks().len(),
        1,
        "the exit with no answer broke no L0"
    );
}
