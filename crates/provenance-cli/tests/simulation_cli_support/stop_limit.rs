//! The watch of a stopped write gives no point when the deadline of the
//! write is past and the write still runs, so that the wait reports the
//! limit and does not hide a write that hangs.

use super::{Point, Running, Watch};
use provenance_store::layout::ProvenanceLayout;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// A process that runs for longer than the test.
fn hung() -> Child {
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("ping");
        command.args(["-n", "120", "127.0.0.1"]);
        command
    };
    #[cfg(not(windows))]
    let mut command = {
        let mut command = Command::new("sleep");
        command.arg("120");
        command
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the test starts a process that hangs")
}

#[test]
fn a_write_that_hangs_after_its_temporary_file_reaches_its_limit() {
    let root = tempfile::tempdir().expect("a temporary directory");
    let layout = ProvenanceLayout::new(root.path().to_str().expect("a UTF-8 path"));
    let watch = Watch::new(&layout);
    // A one-key write whose temporary file no look saw: only the events
    // name it.
    let cache = layout.cache_dir();
    let temporary = cache.as_std_path().join(".state-key.provenance-abc123.tmp");
    std::fs::write(&temporary, b"new").expect("the test writes the temporary file");
    std::fs::rename(&temporary, cache.as_std_path().join("key"))
        .expect("the test renames the temporary file");
    let mut running = Running::with_limit(hung(), Duration::ZERO);
    assert!(
        watch.reach(&mut running, Point::First).is_none(),
        "the watch gave a point of a write that did not end in its limit"
    );
    let waited = running.wait("hung write");
    assert!(waited.is_err(), "the wait did not report the limit");
    super::super::take_l0_breaks();
}
