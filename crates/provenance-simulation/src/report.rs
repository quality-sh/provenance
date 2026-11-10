//! The report of a broken law.
//!
//! The report names the layer and the law, the seed of the run, and the
//! shrunk steps. Hegel prints the drawn values and a
//! `#[hegel::reproduce_failure("<blob>")]` line after it. The blob is a
//! choice sequence that replays the case on any system; the seed repeats a
//! run only on one build. With `SIMULATION_FAILURE_DIR` set, the report of a
//! random case is also written as JSON to `<dir>/<layer>-<law>.json`. A
//! planted run and a probe write no file. A probe that breaks a law prints
//! the law and the command that runs the probe.

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

/// The random test of each layer.
fn random_test(layer: &str) -> &'static str {
    if layer.starts_with("cli-") {
        "random_cases_keep_the_laws_through_the_cli"
    } else if layer == "contract" {
        "random_cases_keep_the_laws_through_the_contract"
    } else {
        "random_cases_keep_the_laws"
    }
}

/// The command that runs `test` of the test binary of `layer`.
///
/// The tests are ignored tests, so the command gives `--run-ignored all`. A
/// CLI run that checks L6 only at the end of each case gives
/// `SIMULATION_L6=end`.
fn test_command(layer: &str, test: &str) -> String {
    let (package, binary, l6) = if layer.starts_with("cli-") {
        let end = std::env::var("SIMULATION_L6").is_ok_and(|l6| l6 == "end");
        let l6 = if end { "SIMULATION_L6=end " } else { "" };
        ("provenance-cli", "simulation_cli", l6)
    } else {
        ("provenance-simulation", layer, "")
    };
    format!(
        "{l6}cargo nextest run --profile simulation --run-ignored all -p {package} --test {binary} -E 'test(={test})'"
    )
}

/// The command that runs the random test of `layer` again with the seed and
/// the case count of this run.
pub fn replay_line(layer: &str, seed: Option<&str>) -> String {
    let seed = seed.unwrap_or("<seed>");
    let cases = std::env::var("HEGEL_TEST_CASES")
        .ok()
        .filter(|cases| !cases.is_empty())
        .map_or_else(String::new, |cases| format!(" HEGEL_TEST_CASES={cases}"));
    let command = test_command(layer, random_test(layer));
    format!("HEGEL_SEED={seed}{cases} {command}")
}

/// Fails the random test case with the report of `violation`, and writes
/// the report to the failure directory.
pub fn fail(layer: &str, case: &Case, violation: &Violation) -> ! {
    crate::stats::note_broken_law();
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
        let path =
            std::path::Path::new(&directory).join(format!("{layer}-{}.json", violation.law));
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

/// Fails probe `test` of `layer` when `verdict` is a broken law.
///
/// The text names the law and gives the command that runs the probe. A
/// probe writes no failure file; the nightly report reads this text.
pub fn probe(layer: &str, test: &str, verdict: Result<(), Violation>) {
    if let Err(violation) = verdict {
        panic!(
            "simulation probe {test} broke law {law} in layer {layer}: {detail}\nreplay: {replay}",
            law = violation.law,
            detail = violation.detail.replace('\n', " "),
            replay = test_command(layer, test),
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
