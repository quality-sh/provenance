//! Labeled probe points for deterministic fault and observation tests.
//!
//! Production code calls [`at`] at a named point. Outside tests the call is
//! a no-op. A test arms a closure for a label on its own thread; the closure
//! may observe state (a lock probe) or return an error (an injected crash).

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
type Probe = Box<dyn FnMut() -> anyhow::Result<()>>;

#[cfg(test)]
thread_local! {
    static PROBES: RefCell<HashMap<&'static str, Probe>> = RefCell::new(HashMap::new());
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
fn leak_label(label: &str) -> &'static str {
    // Labels are compile-time literals at every call site; re-arming after a
    // fire only needs a 'static copy of the same text.
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
