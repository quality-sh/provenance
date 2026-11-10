//! Simulation of the write engine: several engines over one simulated storage.
//!
//! A [`generate::Case`] gives the steps. [`run_case`] runs them with real
//! engines over [`storage::SimulatedStorage`], one storage call at a time, and
//! returns a [`Run`]. [`laws`] checks a run; [`report`] prints a failure so
//! that a later run can replay it. The crate README tells what to do with a
//! failure.

pub mod generate;
mod identity;
pub mod laws;
pub mod plant;
pub mod report;
mod run;
mod state;
pub mod stats;
pub mod storage;

pub use identity::{bare_identity, scope, stable};
pub use plant::Plant;
pub use run::{answer_kind, run_case};
pub use state::{final_records, open_entries, FinalRecords};

use generate::{Case, Effect, Operation, Target, Task};
use provenance_store::repository::Repository;
use provenance_store::storage::StateKey;
use serde::Serialize;
use std::collections::BTreeMap;

/// Runs one operation of an engine; a layer above the engine gives its own.
pub type Apply = fn(&Repository, &Task) -> anyhow::Result<Effect>;

/// The laws that one layer checks in addition to [`laws::check`].
pub type LayerLaws = fn(&Run) -> Result<(), laws::Violation>;

/// How a case runs.
#[derive(Debug, Clone, Copy, Default)]
pub struct Config {
    /// A planted fault in the simulated storage or the identity.
    pub plant: Option<Plant>,
    /// Checks this one law only, as a test of a planted fault does.
    pub only_law: Option<&'static str>,
    /// How an engine runs an operation; `None` calls the store directly.
    pub apply: Option<Apply>,
    /// The laws of the layer, checked after the laws of every layer.
    pub layer_laws: Option<LayerLaws>,
}

impl Config {
    /// A run of a planted fault or of one law writes no failure file and
    /// no count, so that the nightly report opens no issue for it.
    pub const fn writes_failure_files(&self) -> bool {
        self.plant.is_none() && self.only_law.is_none()
    }
}

/// What one operation gave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Done {
    /// The engine slot, or `None` for a write outside every engine.
    pub engine: Option<usize>,
    pub operation: u64,
    pub op: Operation,
    pub target: Option<Target>,
    pub result: Result<Effect, String>,
    /// The kind of the public answer to a refusal, for example
    /// `requirement_edit_conflict`.
    pub kind: Option<String>,
    pub stopped: bool,
}

/// Everything that one case did.
#[derive(Debug, Clone)]
pub struct Run {
    pub done: Vec<Done>,
    pub publishes: Vec<storage::Publish>,
    /// Failures to open and read the state after an engine stopped.
    pub reopen_failures: Vec<String>,
    /// Engines that did not finish in the wait limit.
    pub hung: Vec<usize>,
    pub entries: BTreeMap<StateKey, Vec<u8>>,
}

/// Runs `case` twice and checks the laws of `config`. Returns the first run
/// and the first broken law.
fn verdict(case: &Case, config: Config) -> (Run, Result<(), laws::Violation>) {
    let first = run_case(case, config);
    let layer_laws = |run: &Run| config.layer_laws.map_or(Ok(()), |laws| laws(run));
    let verdict = match config.only_law {
        None => laws::check(&first)
            .and_then(|()| layer_laws(&first))
            .and_then(|()| laws::check_same(&first, &run_case(case, config))),
        Some("L8") => laws::check_same(&first, &run_case(case, config)),
        Some(law) if law.starts_with('O') => layer_laws(&first),
        Some(law) => laws::check_only(&first, law),
    };
    (first, verdict)
}

/// Runs a random case twice, checks every law, and returns the first run.
///
/// A broken law fails the test case with a report that names the law and
/// replays the case. A run of a planted fault or of one law fails with a
/// text that the nightly report does not read.
pub fn check_case(layer: &str, case: &Case, config: Config) -> Run {
    let (first, verdict) = verdict(case, config);
    stats::count_case(layer, &first);
    if let Err(violation) = verdict {
        if config.writes_failure_files() {
            report::fail(layer, case, &violation);
        }
        report::planted_fail(layer, &violation);
    }
    first
}

/// Runs the fixed case of probe `test` twice and returns the first run.
///
/// A broken law fails the probe with the law and the replay line of the
/// probe, and writes no failure file.
pub fn check_probe(layer: &str, test: &str, case: &Case, config: Config) -> Run {
    let (first, verdict) = verdict(case, config);
    report::probe(layer, test, verdict);
    first
}
