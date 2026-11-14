//! A child process of the test binary that runs a write and stops at a
//! probe, and the reads of the state that the stopped write leaves.

use crate::test_probes;
use camino::Utf8Path;

/// The exit code of a child that a probe ends.
const STOPPED: i32 = 86;
const ROOT: &str = "PROVENANCE_STOPPED_CHILD_ROOT";
const PHASE: &str = "PROVENANCE_STOPPED_CHILD_PHASE";
const FILE_SIZE: &str = "PROVENANCE_STOPPED_CHILD_FILE_SIZE";

/// Runs `write` on the repository of the parent when this process is a
/// child of [`stop_in_child`], and stops it at the probe of the phase that
/// the parent gives. In the parent, this does nothing.
pub fn run_as_child(phases: &[&'static str], write: impl FnOnce(&Utf8Path)) {
    let Ok(root) = std::env::var(ROOT) else {
        return;
    };
    let phase = std::env::var(PHASE).unwrap();
    let phase = *phases
        .iter()
        .find(|label| **label == phase)
        .expect("known phase");
    match std::env::var(FILE_SIZE) {
        #[cfg(unix)]
        Ok(limit) => test_probes::stop_at_file_size(phase, limit.parse().unwrap()),
        _ => test_probes::arm(phase, || std::process::exit(STOPPED)),
    }
    write(Utf8Path::new(&root));
    panic!("the stop after the probe at {phase} did not occur");
}

/// The command that runs the test `test` of this binary as a child that
/// writes to the repository at `root` and stops at the probe at `phase`.
fn child(test: &str, root: &Utf8Path, phase: &str) -> std::process::Command {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", test, "--nocapture"])
        .env(ROOT, root.as_str())
        .env(PHASE, phase)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    command
}

/// Runs the test `test` as a child for the repository at `root`. The probe
/// at `phase` ends the child with no unwind, as a kill does.
pub fn stop_in_child(test: &str, root: &Utf8Path, phase: &str) {
    let status = child(test, root, phase).status().unwrap();
    assert_eq!(status.code(), Some(STOPPED), "{phase}");
}

/// Runs the test `test` as a child for the repository at `root`. After the
/// probe at `phase`, the system ends the child with `SIGXFSZ` at its first
/// write of a byte at or past `limit` in a file.
#[cfg(unix)]
pub fn stop_in_child_at_file_size(test: &str, root: &Utf8Path, phase: &str, limit: u64) {
    use std::os::unix::process::ExitStatusExt;
    let status = child(test, root, phase)
        .env(FILE_SIZE, limit.to_string())
        .status()
        .unwrap();
    assert_eq!(
        status.signal(),
        Some(rustix::process::Signal::XFSZ.as_raw()),
        "{phase}"
    );
}

/// The names of the entries of `directory`, in name order.
pub fn names(directory: &Utf8Path) -> Vec<String> {
    let mut names = std::fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

/// The names of the regular files in `directory` that start with `prefix`
/// and end with `suffix`, in name order.
pub fn files_named(directory: &Utf8Path, prefix: &str, suffix: &str) -> Vec<String> {
    names(directory)
        .into_iter()
        .filter(|name| {
            name.len() > prefix.len() + suffix.len()
                && name.starts_with(prefix)
                && name.ends_with(suffix)
                && directory.join(name).is_file()
        })
        .collect()
}
