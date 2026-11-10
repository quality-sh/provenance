//! Labeled probe points for timing tests, as in `provenance-store`.
//!
//! Production code calls [`at`] at a named point. Outside tests it is a
//! no-op. A test arms a closure for a label on its own thread.

#[cfg(test)]
use std::cell::RefCell;
#[cfg(test)]
use std::collections::HashMap;

#[cfg(test)]
type Probe = Box<dyn FnMut()>;

#[cfg(test)]
thread_local! {
    static PROBES: RefCell<HashMap<&'static str, Probe>> = RefCell::new(HashMap::new());
}

#[cfg(test)]
pub fn at(label: &'static str) {
    let probe = PROBES.with(|probes| probes.borrow_mut().remove(label));
    if let Some(mut probe) = probe {
        probe();
        PROBES.with(|probes| probes.borrow_mut().insert(label, probe));
    }
}

#[cfg(not(test))]
pub const fn at(_label: &'static str) {}

#[cfg(test)]
pub fn arm(label: &'static str, probe: impl FnMut() + 'static) {
    PROBES.with(|probes| probes.borrow_mut().insert(label, Box::new(probe)));
}

#[cfg(test)]
pub fn disarm(label: &str) {
    PROBES.with(|probes| {
        probes.borrow_mut().remove(label);
    });
}
