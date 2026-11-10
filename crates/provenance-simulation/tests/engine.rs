//! Engines over one simulated storage keep the laws of `laws.rs`.
//!
//! The nightly workflow runs this target with `HEGEL_SEED` and
//! `HEGEL_TEST_CASES`. A broken law fails the case; Hegel shrinks it and
//! prints the `#[hegel::reproduce_failure]` line that replays it.

use hegel::{Hegel, Settings, TestCase, Verbosity};
use provenance_macros::verifies;
use provenance_simulation::generate::{Case, Fault, Operation, Step};
use provenance_simulation::{check_case, laws, report, run_case, storage, Config};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

#[hegel::test]
#[ignore = "random cases run nightly: cargo nextest run --profile simulation --run-ignored all"]
#[verifies("rule_write_sets_publish_through_the_storage_port", property)]
fn random_cases_keep_the_laws(tc: TestCase) {
    check_case("engine", &Case::draw(&tc), Config::default());
}

const PLANTED: Config = Config {
    plant_stale_read: true,
    only_law: None,
    apply: None,
    layer_laws: None,
};

/// The planted property checks only the law the planted fault breaks, so a
/// real failure of another law cannot stand in for it.
const PLANTED_L5: Config = Config {
    plant_stale_read: true,
    only_law: Some("L5"),
    apply: None,
    layer_laws: None,
};

/// A section that meets a moved base: the engine enters the section and
/// reads, then a write outside lands before its first publish.
fn moved_base_case() -> Case {
    let bind = Operation::BindTwice {
        first: 0,
        second: 1,
    };
    Case {
        engines: 2,
        steps: vec![
            Step::Write {
                engine: 0,
                op: bind,
            },
            Step::Turn(0),
            Step::Turn(0),
            Step::OutsideWrite(Operation::PostMessage),
        ],
    }
}

#[test]
fn the_write_after_a_moved_base_lands() {
    let run = run_case(&moved_base_case(), Config::default());
    laws::check(&run).unwrap();
}

#[test]
fn a_planted_stale_read_breaks_l5() {
    let run = run_case(&moved_base_case(), PLANTED);
    assert_eq!(laws::check(&run).unwrap_err().law, "L5");
}

#[test]
fn a_case_with_every_fault_keeps_the_laws() {
    let case = Case {
        engines: 3,
        steps: vec![
            Step::Write {
                engine: 0,
                op: Operation::UpdateRequirement { n: 0, stale: false },
            },
            Step::Write {
                engine: 1,
                op: Operation::UpdateRequirement { n: 0, stale: false },
            },
            Step::Turn(0),
            Step::Turn(1),
            Step::Fault(Fault::UnknownOutcome { landed: 1 }),
            Step::Write {
                engine: 2,
                op: Operation::ClaimTopic,
            },
            Step::Fault(Fault::Stop { engine: 1 }),
            Step::Fault(Fault::Refuse),
            Step::Write {
                engine: 1,
                op: Operation::SubmitReview,
            },
            Step::OutsideWrite(Operation::CreateRequirement { n: 1 }),
            Step::Write {
                engine: 0,
                op: Operation::PostMessage,
            },
            Step::Write {
                engine: 2,
                op: Operation::ReleaseTopic,
            },
        ],
    };
    check_case("engine", &case, Config::default());
}

/// Runs the planted property with `settings`, or with the replay `blob`, and
/// returns the lines that Hegel printed and the text of the final panic.
fn planted_run(settings: Settings, blob: Option<&str>) -> (Vec<String>, String) {
    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink = lines.clone();
    let sink: Arc<dyn Fn(&str) + Send + Sync> =
        Arc::new(move |line: &str| sink.lock().unwrap().push(line.to_owned()));
    let payload = hegel::with_output_override(sink, || {
        catch_unwind(AssertUnwindSafe(|| {
            let mut hegel = Hegel::new(|tc: TestCase| {
                check_case("engine", &Case::draw(&tc), PLANTED_L5);
            })
            .settings(settings);
            if let Some(blob) = blob {
                hegel = hegel.reproduce_failure(blob);
            }
            hegel.run();
        }))
        .expect_err("the planted fault breaks a law")
    });
    let lines = lines.lock().unwrap().clone();
    (lines, report::panic_text(&*payload))
}

fn settings() -> Settings {
    Settings::new()
        .seed(Some(41))
        .database(None)
        .test_cases(1000)
        .verbosity(Verbosity::Normal)
}

/// Probe of the plan: one seed gives one shrunk case, the case breaks L5,
/// and its blob replays it.
#[test]
#[ignore = "random cases run nightly: cargo nextest run --profile simulation --run-ignored all"]
fn random_planted_cases_repeat_by_seed_and_replay_by_blob() {
    let (first, message) = planted_run(settings(), None);
    let second = planted_run(settings(), None);
    assert_eq!(
        (&first, &message),
        (&second.0, &second.1),
        "one seed gives one report"
    );
    assert!(message.contains("simulation law L5"), "{message}");
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

#[test]
fn an_unknown_outcome_lands_a_prefix_and_keeps_the_laws() {
    for landed in 0..3 {
        let case = Case {
            engines: 2,
            steps: vec![
                Step::Fault(Fault::UnknownOutcome { landed }),
                Step::Write {
                    engine: 0,
                    op: Operation::PostMessage,
                },
                Step::Write {
                    engine: 1,
                    op: Operation::UpdateRequirement { n: 0, stale: false },
                },
            ],
        };
        let run = run_case(&case, Config::default());
        assert!(
            run.publishes
                .iter()
                .any(|publish| publish.outcome == storage::Outcome::Unknown),
            "the fault met a publish"
        );
        laws::check(&run).unwrap();
    }
}

#[test]
fn one_law_is_checked_alone() {
    let run = run_case(&moved_base_case(), PLANTED);
    for law in ["L1", "L3", "L4"] {
        laws::check_only(&run, law).unwrap();
    }
    assert_eq!(laws::check_only(&run, "L5").unwrap_err().law, "L5");
    assert_eq!(laws::check_only(&run, "every").unwrap_err().law, "L5");
}
