//! Counts of what the random cases of a layer did, for the job summary.
//!
//! With `SIMULATION_STATS_DIR` set, each count goes to
//! `<dir>/<layer>.json` after each case. The summary shows them, so that a
//! night whose cases did not reach a fault is visible.

use crate::storage::Outcome;
use crate::Run;
use std::collections::BTreeMap;
use std::sync::{Mutex, PoisonError};

static COUNTS: Mutex<BTreeMap<String, BTreeMap<&'static str, u64>>> =
    Mutex::new(BTreeMap::new());

/// Adds `amount` to the count `name` of `layer` and writes the counts of the
/// layer.
pub fn add(layer: &str, name: &'static str, amount: u64) {
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
    add(layer, "cases", 1);
    add(layer, "cases with a moved base", u64::from(moved > 0));
    add(layer, "moved bases", moved);
    add(layer, "refused publishes", count(Outcome::Refused));
    add(layer, "publishes with an unknown outcome", count(Outcome::Unknown));
    let stages = number(run.publishes.iter().filter(|publish| publish.whole_set).count());
    add(layer, "stages", stages);
    let stopped = number(run.done.iter().filter(|done| done.stopped).count());
    add(layer, "stopped operations", stopped);
}
