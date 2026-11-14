//! Labeled probe points for fault and observation tests.
//!
//! Production code calls [`at`] at a named point and [`record_read`] with
//! every canonical path it opens. Outside tests both are no-ops. A test arms
//! a closure for a label on its own thread, or records the set of paths one
//! derivation reads. A test can also set the working-tree scan the reader
//! hands out, so a timing row measures graph work and not the scan, and
//! can ask whether the publication lock is held. A test can also set a
//! smaller work bound for the pages that it reads on its own thread.

#[cfg(all(test, feature = "host"))]
use std::cell::Cell;
#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::collections::{BTreeSet, HashMap};

#[cfg(test)]
type Probe = Box<dyn FnMut() -> anyhow::Result<()>>;

#[cfg(test)]
thread_local! {
    static PROBES: RefCell<HashMap<&'static str, Probe>> = RefCell::new(HashMap::new());
    static READS: RefCell<Option<BTreeSet<String>>> = const { RefCell::new(None) };
    #[cfg(feature = "host")]
    static PAGE_WORK_STEPS: Cell<Option<u64>> = const { Cell::new(None) };
    #[cfg(feature = "host")]
    static SCAN: RefCell<Option<Vec<provenance_scanner::FileScan>>> = const { RefCell::new(None) };
}

/// The scan a test set on this thread, when there is one.
#[cfg(all(test, feature = "host"))]
pub fn test_scan() -> Option<Vec<provenance_scanner::FileScan>> {
    SCAN.with(|scan| scan.borrow().clone())
}

#[cfg(all(not(test), feature = "host"))]
pub const fn test_scan() -> Option<Vec<provenance_scanner::FileScan>> {
    None
}

#[cfg(all(test, feature = "host"))]
pub fn set_test_scan(scans: Option<Vec<provenance_scanner::FileScan>>) {
    SCAN.with(|scan| *scan.borrow_mut() = scans);
}

/// The work bound of one page: `steps`, or the bound that a test set on
/// this thread.
#[cfg(all(test, feature = "host"))]
pub fn page_work_steps(steps: u64) -> u64 {
    PAGE_WORK_STEPS.with(Cell::get).unwrap_or(steps)
}

#[cfg(any(not(test), not(feature = "host")))]
pub const fn page_work_steps(steps: u64) -> u64 {
    steps
}

/// Sets the work bound of the pages that this thread reads; `None` puts
/// back the bound of the engine.
#[cfg(all(test, feature = "host"))]
pub fn set_page_work_steps(steps: Option<u64>) {
    PAGE_WORK_STEPS.with(|bound| bound.set(steps));
}

/// Whether another holder has the repository's publication lock right now.
#[cfg(all(test, feature = "host"))]
pub fn publication_lock_is_held(layout: &crate::layout::ProvenanceLayout) -> bool {
    use fs2::FileExt;
    let path = layout.publication_lock_path();
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .unwrap();
    if file.try_lock_exclusive().is_ok() {
        let _ = fs2::FileExt::unlock(&file);
        false
    } else {
        true
    }
}

#[cfg(test)]
pub fn at(label: &str) -> anyhow::Result<()> {
    let probe = PROBES.with(|probes| probes.borrow_mut().remove(label));
    if let Some(mut probe) = probe {
        let result = probe();
        PROBES.with(|probes| probes.borrow_mut().insert(leak_label(label), probe));
        return result;
    }
    Ok(())
}

#[cfg(not(test))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "one signature with the test build"
)]
pub const fn at(_label: &str) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(test)]
pub fn record_read(path: &camino::Utf8Path) {
    READS.with(|reads| {
        if let Some(set) = reads.borrow_mut().as_mut() {
            set.insert(path.to_string());
        }
    });
}

#[cfg(not(test))]
pub const fn record_read(_path: &camino::Utf8Path) {}

#[cfg(all(test, feature = "host"))]
pub fn start_recording_reads() {
    READS.with(|reads| *reads.borrow_mut() = Some(BTreeSet::new()));
}

#[cfg(all(test, feature = "host"))]
pub fn take_recorded_reads() -> BTreeSet<String> {
    READS.with(|reads| reads.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
fn leak_label(label: &str) -> &'static str {
    // Labels are literals at every call site. Re-arming needs a 'static copy.
    Box::leak(label.to_string().into_boxed_str())
}

#[cfg(all(test, feature = "host"))]
pub fn arm(label: &'static str, probe: impl FnMut() -> anyhow::Result<()> + 'static) {
    PROBES.with(|probes| probes.borrow_mut().insert(label, Box::new(probe)));
}

#[cfg(all(test, feature = "host"))]
pub fn disarm(label: &str) {
    PROBES.with(|probes| {
        probes.borrow_mut().remove(label);
    });
}

#[cfg(all(test, feature = "host"))]
pub fn crash_at(label: &'static str) {
    arm(label, move || anyhow::bail!("injected crash at {label}"));
}

/// Arms `label` to end the process at its next write to a file, with no
/// unwind, as a kill does.
///
/// The probe sets the file size limit to zero, so the system sends `SIGXFSZ`
/// at the next byte that the process writes to a file. The probe also sets
/// the core file size limit to zero.
#[cfg(all(test, unix, feature = "host"))]
pub fn stop_at_next_file_write(label: &'static str) {
    arm(label, || {
        use rustix::process::{getrlimit, setrlimit, Resource};
        for resource in [Resource::Core, Resource::Fsize] {
            let mut limit = getrlimit(resource);
            limit.current = Some(0);
            setrlimit(resource, limit)?;
        }
        Ok(())
    });
}
