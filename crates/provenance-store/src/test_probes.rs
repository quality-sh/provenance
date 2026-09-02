//! Labeled probe points for fault and observation tests.
//!
//! Production code calls [`at`] at a named point and [`record_read`] with
//! every canonical path it opens. Outside tests both are no-ops. A test arms
//! a closure for a label on its own thread, or records the set of paths one
//! derivation reads.

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
