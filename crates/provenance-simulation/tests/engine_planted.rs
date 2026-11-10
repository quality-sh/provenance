//! Planted faults: each test plants the smallest fault that one law must
//! see, and asserts that the law reports it by its id. A planted run writes
//! no failure file, and a test whose law does not see its fault fails with
//! "simulation check broken", so the nightly report opens no law issue for
//! it. CI does not run these tests: the ci profile of nextest leaves out
//! this crate.

#[allow(dead_code)]
mod support {
    pub mod cases;
}

use hegel::{Hegel, Settings, TestCase, Verbosity};
use provenance_simulation::generate::Case;
use provenance_simulation::laws::{self, Violation};
use provenance_simulation::{check_case, report, run_case, Config, Plant};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use support::cases::moved_base_case;

const fn planted(plant: Plant, law: &'static str) -> Config {
    Config {
        plant: Some(plant),
        only_law: Some(law),
        apply: None,
        layer_laws: None,
    }
}

/// Fails with the marker of a broken check unless `verdict` names `law`.
fn expect_law(verdict: Result<(), Violation>, law: &str) {
    match verdict {
        Err(violation) if violation.law == law => {}
        Err(violation) => report::check_broken(law, &format!("{violation:?}")),
        Ok(()) => report::check_broken(law, "every law held"),
    }
}

// No Rule: proof that L5 can fail; the law is a test aid with no Rule.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_planted_stale_read_breaks_l5() {
    let run = run_case(&moved_base_case(), planted(Plant::StaleRead, "L5"));
    expect_law(laws::check(&run), "L5");
}

// No Rule: proof that one law can be checked alone.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn one_law_is_checked_alone() {
    let run = run_case(&moved_base_case(), planted(Plant::StaleRead, "L5"));
    for law in ["L0", "L1", "L2", "L3", "L4"] {
        laws::check_only(&run, law).unwrap();
    }
    expect_law(laws::check_only(&run, "L5"), "L5");
    expect_law(laws::check_only(&run, "every"), "L5");
}

/// Runs the planted property with `settings`, or with the replay `blob`, and
/// returns the lines that Hegel printed and the text of the final panic.
fn planted_run(settings: Settings, blob: Option<&str>) -> (Vec<String>, String) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    let sink: Arc<dyn Fn(&str) + Send + Sync> =
        Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_owned()));
    let outcome = hegel::with_output_override(sink, || {
        catch_unwind(AssertUnwindSafe(|| {
            let mut hegel = Hegel::new(|tc: TestCase| {
                check_case("engine", &Case::draw(&tc), planted(Plant::StaleRead, "L5"));
            })
            .settings(settings);
            if let Some(blob) = blob {
                hegel = hegel.reproduce_failure(blob);
            }
            hegel.run();
        }))
    });
    let lines = lines.lock().unwrap().clone();
    let text = match outcome {
        Err(payload) => report::panic_text(&*payload),
        Ok(()) => report::check_broken("L5", "no random case met the planted fault"),
    };
    (lines, text)
}

fn settings() -> Settings {
    Settings::new()
        .seed(Some(41))
        .database(None)
        .test_cases(1000)
        .verbosity(Verbosity::Normal)
}

// No Rule: a probe of the plan. One seed gives one shrunk case, the case
// breaks L5, and its blob replays it.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn random_planted_cases_repeat_by_seed_and_replay_by_blob() {
    let (first, message) = planted_run(settings(), None);
    let second = planted_run(settings(), None);
    assert_eq!(
        (&first, &message),
        (&second.0, &second.1),
        "one seed gives one report"
    );
    if !message.contains("simulation law L5") {
        report::check_broken("L5", &message);
    }
    let report = first.join("\n");
    let blob = first
        .iter()
        .find_map(|line| {
            let start = line.find("reproduce_failure(\"")? + "reproduce_failure(\"".len();
            let end = line[start..].find('"')? + start;
            Some(line[start..end].to_owned())
        })
        .unwrap_or_else(|| panic!("a reproduce_failure line in {report}"));
    let (_, replayed) = planted_run(Settings::new().database(None), Some(&blob));
    assert_eq!(replayed, message, "the blob replays the shrunk case");
}
