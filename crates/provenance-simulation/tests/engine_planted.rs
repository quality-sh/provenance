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

#[path = "engine_planted/l3.rs"]
mod l3;

use hegel::{Hegel, Settings, TestCase, Verbosity};
use provenance_simulation::generate::{Case, Fault, Operation, Step};
use provenance_simulation::laws::{self, Violation};
use provenance_simulation::{check_case, report, run_case, Config, Plant};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};
use support::cases::{alone, moved_base_case, write};

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
    if !message.contains("law L5 reported") {
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

/// Two engines that update the first Requirement on a fresh etag, one
/// storage call at a time each, in turn.
fn lockstep_updates() -> Case {
    let update = Operation::UpdateRequirement { n: 0, stale: false };
    let mut steps = vec![write(0, update.clone()), write(1, update)];
    for _ in 0..100 {
        steps.extend([Step::Turn(0), Step::Turn(1)]);
    }
    Case { engines: 2, steps }
}

// No Rule: proof that L1 can fail. The storage lands the set of an armed
// refusal and answers a refusal, so the update fails and its write landed.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_refusal_that_lands_the_set_breaks_l1() {
    let mut steps = vec![Step::Fault(Fault::Refuse)];
    steps.extend(alone(&[Operation::UpdateRequirement {
        n: 0,
        stale: false,
    }]));
    let case = Case { engines: 2, steps };
    let run = run_case(&case, planted(Plant::LandRefused, "L1"));
    expect_law(laws::check_only(&run, "L1"), "L1");
}

// No Rule: proof that L2 can fail. After a stop, the reads of the new
// engine fail.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_state_that_does_not_open_after_a_stop_breaks_l2() {
    let mut steps = alone(&[Operation::PostMessage]);
    steps.push(Step::Fault(Fault::Stop { engine: 0, at: 0 }));
    let case = Case { engines: 2, steps };
    let run = run_case(&case, planted(Plant::FailReopen, "L2"));
    expect_law(laws::check_only(&run, "L2"), "L2");
}

// No Rule: proof that L4 can fail. With no section lock and no base check,
// two updates on one etag both succeed.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn two_updates_without_concurrency_control_break_l4() {
    let config = planted(Plant::NoConcurrencyControl, "L4");
    let run = run_case(&lockstep_updates(), config);
    expect_law(laws::check_only(&run, "L4"), "L4");
}

// No Rule: proof that L8 can fail. The clock continues from the last run,
// so the second run of a case writes other times.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_clock_that_continues_between_runs_breaks_l8() {
    let case = Case {
        engines: 2,
        steps: alone(&[Operation::PostMessage]),
    };
    let config = planted(Plant::SharedClock, "L8");
    let (first, second) = (run_case(&case, config), run_case(&case, config));
    expect_law(laws::check_same(&first, &second), "L8");
}
