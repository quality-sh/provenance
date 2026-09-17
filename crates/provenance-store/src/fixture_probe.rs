//! One-shot coordination points for router-level concurrency fixtures.

use crate::layout::ProvenanceLayout;
use fs2::FileExt as _;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

type Probe = Box<dyn FnOnce() + Send>;

fn probes() -> &'static Mutex<HashMap<&'static str, Probe>> {
    static PROBES: OnceLock<Mutex<HashMap<&'static str, Probe>>> = OnceLock::new();
    PROBES.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn arm(label: &'static str, probe: impl FnOnce() + Send + 'static) {
    probes().lock().unwrap().insert(label, Box::new(probe));
}

pub fn at(label: &'static str) {
    let probe = probes().lock().unwrap().remove(label);
    if let Some(probe) = probe {
        probe();
    }
}

pub fn disarm(label: &'static str) {
    probes().lock().unwrap().remove(label);
}

pub fn publication_lock_is_held(layout: &ProvenanceLayout) -> bool {
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
        let _ = file.unlock();
        false
    } else {
        true
    }
}
