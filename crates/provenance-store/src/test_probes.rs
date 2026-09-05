//! Labeled probe points for fault and observation tests.
//!
//! Production code calls [`at`] at a named point and [`record_read`] with
//! every canonical path it opens. Outside tests both are no-ops. A test arms
//! a closure for a label on its own thread, or records the set of paths one
//! derivation reads. A test can also test-set the working-tree scan the
//! reader hands out, so a timing row measures graph work and not the scan,
//! and can ask whether the publication lock is held.

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
    static SCAN: RefCell<Option<Vec<provenance_scanner::FileScan>>> = const { RefCell::new(None) };
}

/// The test-set scan, when a test set one on this thread.
#[cfg(test)]
pub fn test_scan() -> Option<Vec<provenance_scanner::FileScan>> {
    SCAN.with(|scan| scan.borrow().clone())
}

#[cfg(not(test))]
pub const fn test_scan() -> Option<Vec<provenance_scanner::FileScan>> {
    None
}

#[cfg(test)]
pub fn set_test_scan(scans: Option<Vec<provenance_scanner::FileScan>>) {
    SCAN.with(|scan| *scan.borrow_mut() = scans);
}

/// Whether another holder has the repository's publication lock right now.
#[cfg(test)]
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

#[cfg(test)]
pub fn start_recording_reads() {
    READS.with(|reads| *reads.borrow_mut() = Some(BTreeSet::new()));
}

#[cfg(test)]
pub fn take_recorded_reads() -> BTreeSet<String> {
    READS.with(|reads| reads.borrow_mut().take().unwrap_or_default())
}

#[cfg(test)]
fn leak_label(label: &str) -> &'static str {
    // Labels are literals at every call site. Re-arming needs a 'static copy.
    Box::leak(label.to_string().into_boxed_str())
}

#[cfg(test)]
pub fn arm(label: &'static str, probe: impl FnMut() -> anyhow::Result<()> + 'static) {
    PROBES.with(|probes| probes.borrow_mut().insert(label, Box::new(probe)));
}

#[cfg(test)]
pub fn disarm(label: &str) {
    PROBES.with(|probes| {
        probes.borrow_mut().remove(label);
    });
}

#[cfg(test)]
pub fn crash_at(label: &'static str) {
    arm(label, move || anyhow::bail!("injected crash at {label}"));
}
