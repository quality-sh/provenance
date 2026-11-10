//! Counts of what the random cases of a layer did, for the job summary.
//!
//! With `SIMULATION_STATS_DIR` set, each count goes to
//! `<dir>/<layer>.json` after each case. The summary shows them, so that a
//! night whose cases did not reach a fault is visible. Only the random test
//! of a layer counts: it calls [`record`] first, so that a probe or a
//! planted test in its own process writes no counts file. The counts
//! describe the cases up to the first broken law; after it, Hegel runs the
//! shrink, and each run of the shrink adds only to "shrink runs".

use crate::storage::Outcome;
use crate::Run;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

static COUNTS: Mutex<BTreeMap<String, BTreeMap<&'static str, u64>>> = Mutex::new(BTreeMap::new());

/// The random test of this process counts.
static RECORDING: AtomicBool = AtomicBool::new(false);

/// A random case of this process broke a law, so each later run is a run of
/// the shrink.
static SHRINKING: AtomicBool = AtomicBool::new(false);

/// The count of the runs of the shrink.
const SHRINK_RUNS: &str = "shrink runs";

/// Turns on the counts of this test process.
pub fn record() {
    RECORDING.store(true, Ordering::Relaxed);
}

/// Notes that a random case broke a law: the runs after it are the shrink.
pub fn note_broken_law() {
    SHRINKING.store(true, Ordering::Relaxed);
}

/// Adds `amount` to the count `name` of `layer` and writes the counts of the
/// layer. A run of the shrink adds nothing.
pub fn add(layer: &str, name: &'static str, amount: u64) {
    if !SHRINKING.load(Ordering::Relaxed) {
        write(layer, name, amount);
    }
}

/// Counts one run of a random case: a case before the first broken law, or
/// a run of the shrink after it.
pub fn count_run(layer: &str) {
    if SHRINKING.load(Ordering::Relaxed) {
        write(layer, SHRINK_RUNS, 1);
    } else {
        write(layer, "cases", 1);
        write(layer, SHRINK_RUNS, 0);
    }
}

fn write(layer: &str, name: &'static str, amount: u64) {
    if !RECORDING.load(Ordering::Relaxed) {
        return;
    }
    let mut counts = COUNTS.lock().unwrap_or_else(PoisonError::into_inner);
    let layer_counts = counts.entry(layer.to_owned()).or_default();
    *layer_counts.entry(name).or_default() += amount;
    let json = serde_json::to_string_pretty(layer_counts).expect("counts are JSON");
    drop(counts);
    let Some(directory) = std::env::var_os("SIMULATION_STATS_DIR") else {
        return;
    };
    let path = std::path::Path::new(&directory).join(format!("{layer}.json"));
    let written = std::fs::create_dir_all(&directory).and_then(|()| std::fs::write(&path, json));
    if let Err(error) = written {
        eprintln!("could not write {}: {error}", path.display());
    }
}

/// Counts one random case of the engine or the contract layer.
pub fn count_case(layer: &str, run: &Run) {
    let number = |count: usize| u64::try_from(count).unwrap_or(u64::MAX);
    let count = |outcome| {
        number(
            run.publishes
                .iter()
                .filter(|publish| publish.outcome == outcome)
                .count(),
        )
    };
    let moved = count(Outcome::BaseMoved);
    count_run(layer);
    add(layer, "cases with a moved base", u64::from(moved > 0));
    add(layer, "moved bases", moved);
    add(layer, "refused publishes", count(Outcome::Refused));
    add(
        layer,
        "publishes with an unknown outcome",
        count(Outcome::Unknown),
    );
    let stages = number(
        run.publishes
            .iter()
            .filter(|publish| publish.whole_set)
            .count(),
    );
    add(layer, "stages", stages);
    let stopped = number(run.done.iter().filter(|done| done.stopped).count());
    add(layer, "stopped operations", stopped);
}
