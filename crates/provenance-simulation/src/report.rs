//! The report of a broken law.
//!
//! The report names the layer and the law, the seed of the run, and the
//! shrunk steps. Hegel prints the drawn values and a
//! `#[hegel::reproduce_failure("<blob>")]` line after it. The blob is a
//! choice sequence that replays the case on any system; the seed repeats a
//! run only on one build. With `SIMULATION_FAILURE_DIR` set, the report of a
//! random case is also written as JSON to `<dir>/<layer>-<law>.json`. A
//! planted run and a probe write no file. The nightly report reads the
//! package, the test target and the test of each failure from the log of
//! nextest, so this text does not name them.

use crate::generate::Case;
use crate::laws::Violation;
use serde::Serialize;
use std::path::Path;

/// The failure that the nightly workflow reads.
#[derive(Debug, Serialize)]
pub struct Failure<'a> {
    pub layer: &'a str,
    pub law: &'static str,
    pub detail: &'a str,
    pub seed: Option<String>,
    pub case: &'a Case,
}

/// The seed of this run, from `HEGEL_SEED`.
pub fn seed() -> Option<String> {
    std::env::var("HEGEL_SEED")
        .ok()
        .filter(|seed| !seed.is_empty())
}

/// Fails the random test case with the report of `violation`, and writes
/// the report to the failure directory.
pub fn fail(layer: &str, case: &Case, violation: &Violation) -> ! {
    crate::stats::note_broken_law();
    let failure = Failure {
        layer,
        law: violation.law,
        detail: &violation.detail,
        seed: seed(),
        case,
    };
    let json = serde_json::to_string_pretty(&failure).expect("a failure is JSON");
    if let Some(directory) = std::env::var_os("SIMULATION_FAILURE_DIR") {
        let path = Path::new(&directory).join(format!("{layer}-{}.json", violation.law));
        if let Err(error) =
            std::fs::create_dir_all(&directory).and_then(|()| std::fs::write(&path, &json))
        {
            eprintln!("could not write {}: {error}", path.display());
        }
    }
    panic!(
        "simulation law {law} broken in layer {layer}: {detail}\nseed: {seed}\nto replay the case, paste the #[hegel::reproduce_failure] line below on the test\nsteps: {json}",
        law = violation.law,
        detail = violation.detail,
        seed = failure.seed.as_deref().unwrap_or("none (derandomized or random)"),
    );
}

/// Fails a run of a planted fault or of one law with the law that it saw.
///
/// The text does not match a broken-law line of the nightly report, and the
/// run writes no failure file.
pub fn planted_fail(layer: &str, violation: &Violation) -> ! {
    panic!(
        "planted run in layer {layer}: law {law} reported: {detail}",
        law = violation.law,
        detail = violation.detail,
    );
}

/// Fails a probe, a fixed case of `layer`, when `verdict` is a broken law.
/// The text is on one line, so that the nightly report reads it whole.
pub fn probe_laws(layer: &str, verdict: Result<(), Violation>) {
    if let Err(violation) = verdict {
        panic!(
            "simulation probe broke law {law} in layer {layer}: {detail}",
            law = violation.law,
            detail = violation.detail.replace('\n', " "),
        );
    }
}

/// Fails a planted test whose law did not see its planted fault. The
/// nightly report reads the marker and opens no law issue.
pub fn check_broken(law: &str, detail: &str) -> ! {
    panic!("simulation check broken: law {law} did not see its planted fault: {detail}");
}

/// The text of a panic payload.
pub fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|text| (*text).to_owned())
        })
        .unwrap_or_else(|| "a panic without text".to_owned())
}
