//! The report of a broken law.
//!
//! The report names the layer and the law, the seed of the run, and the
//! shrunk steps. Hegel prints the drawn values and a
//! `#[hegel::reproduce_failure("<blob>")]` line after it. The blob is a
//! choice sequence that replays the case on any system; the seed repeats a
//! run only on one build. With `SIMULATION_FAILURE_DIR` set, the report is
//! also written as JSON to `<dir>/<layer>-<law>.json`.

use crate::generate::Case;
use crate::laws::Violation;
use serde::Serialize;

/// The failure that the nightly workflow reads.
#[derive(Debug, Serialize)]
pub struct Failure<'a> {
    pub layer: &'a str,
    pub law: &'static str,
    pub detail: &'a str,
    pub seed: Option<String>,
    pub case: &'a Case,
    pub replay: String,
}

/// The seed of this run, from `HEGEL_SEED`.
pub fn seed() -> Option<String> {
    std::env::var("HEGEL_SEED")
        .ok()
        .filter(|seed| !seed.is_empty())
}

/// The command that runs the case again with the seed of this run.
pub fn replay_line(layer: &str, seed: Option<&str>) -> String {
    let seed = seed.unwrap_or("<seed>");
    format!(
        "HEGEL_SEED={seed} cargo nextest run --profile simulation -p provenance-simulation --test {layer}"
    )
}

/// Fails the test case with the report of `violation`.
pub fn fail(layer: &str, case: &Case, violation: &Violation) -> ! {
    let seed = seed();
    let failure = Failure {
        layer,
        law: violation.law,
        detail: &violation.detail,
        replay: replay_line(layer, seed.as_deref()),
        seed,
        case,
    };
    let json = serde_json::to_string_pretty(&failure).expect("a failure is JSON");
    if let Some(directory) = std::env::var_os("SIMULATION_FAILURE_DIR") {
        let path = std::path::Path::new(&directory).join(format!("{layer}-{}.json", violation.law));
        if let Err(error) =
            std::fs::create_dir_all(&directory).and_then(|()| std::fs::write(&path, &json))
        {
            eprintln!("could not write {}: {error}", path.display());
        }
    }
    panic!(
        "simulation law {law} broken in layer {layer}: {detail}\nseed: {seed}\nreplay: {replay}\nor paste the #[hegel::reproduce_failure] line below on the test\nsteps: {json}",
        law = violation.law,
        detail = violation.detail,
        seed = failure.seed.as_deref().unwrap_or("none (derandomized or random)"),
        replay = failure.replay,
    );
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
