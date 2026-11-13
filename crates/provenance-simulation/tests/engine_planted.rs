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
use provenance_simulation::generate::{self, Case, Effect, Fault, Operation, Step, Task, TOPIC};
use provenance_simulation::laws::{self, Violation};
use provenance_simulation::storage::{Outcome, Publish};
use provenance_simulation::{check_case, report, run_case, scope, stable, Config, Plant, Run};
use provenance_store::repository::Repository;
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

// No Rule: proof that L3 can fail. The storage drops the first set of an
// engine and answers that it landed.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_lost_write_breaks_l3() {
    let case = Case {
        engines: 2,
        steps: alone(&[Operation::UpdateRequirement { n: 0, stale: false }]),
    };
    let run = run_case(&case, planted(Plant::LoseWrite, "L3"));
    expect_law(laws::check_only(&run, "L3"), "L3");
}

/// The outside claim, then an archive of the Requirement of the Topic by
/// engine 0, each one alone.
fn claim_then_archive() -> Case {
    let mut steps = vec![Step::OutsideWrite(Operation::ClaimTopic)];
    steps.extend(alone(&[Operation::ArchiveRequirement { n: 0 }]));
    Case { engines: 2, steps }
}

// No Rule: proof that L3 sees a claim that an archive leaves. The storage
// drops the archive and answers that it landed, so the Topic keeps the claim.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn an_archive_that_leaves_the_claim_breaks_l3() {
    let run = run_case(&claim_then_archive(), planted(Plant::LoseWrite, "L3"));
    expect_law(laws::check_only(&run, "L3"), "L3");
}

// No Rule: proof that L3 sees a claim that is lost with no archive. The
// storage drops the claim of engine 0 and answers that it landed.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_lost_claim_breaks_l3() {
    let case = Case {
        engines: 2,
        steps: alone(&[Operation::ClaimTopic]),
    };
    let run = run_case(&case, planted(Plant::LoseWrite, "L3"));
    expect_law(laws::check_only(&run, "L3"), "L3");
}

const ONLY_L3: Config = Config {
    plant: None,
    only_law: Some("L3"),
    apply: None,
    layer_laws: None,
};

/// A stand-in archive of the first Requirement: it clears the claim of the
/// Topic and leaves the Requirement and the Topic open.
fn archive_that_only_clears_the_claim(
    repository: &Repository,
    task: &Task,
) -> anyhow::Result<Effect> {
    let Operation::ArchiveRequirement { n: 0 } = task.op else {
        return generate::apply(repository, task);
    };
    let store = repository.store();
    store.release_topic(&scope(), &stable(TOPIC), &task.actor())?;
    Ok(Effect::Archived {
        id: generate::FIRST_REQUIREMENT.to_owned(),
    })
}

// No Rule: proof that L3 sees a claim that is cleared on a Topic that stays
// open. A stand-in archive answers success, clears the claim and archives
// nothing.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_claim_cleared_on_an_open_topic_breaks_l3() {
    let case = Case {
        engines: 2,
        steps: alone(&[
            Operation::ClaimTopic,
            Operation::ArchiveRequirement { n: 0 },
        ]),
    };
    let config = Config {
        apply: Some(archive_that_only_clears_the_claim),
        ..ONLY_L3
    };
    let run = run_case(&case, config);
    expect_law(laws::check_only(&run, "L3"), "L3");
}

/// Writes the claim of `holder` in the row of the Topic in the final state.
fn claim_in_the_final_state(run: &mut Run, holder: &str) {
    let (_, bytes) = run
        .entries
        .iter_mut()
        .find(|(key, _)| key.as_str().contains("/topics/"))
        .expect("a Topic shard");
    let mut rows = Vec::new();
    for line in bytes
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let mut row: serde_json::Value = serde_json::from_slice(line).expect("a Topic row");
        if row["id"] == TOPIC {
            row["claimed_by"] = holder.into();
            row["claimed_at"] = 1.into();
        }
        rows.extend(serde_json::to_vec(&row).expect("JSON"));
        rows.push(b'\n');
    }
    *bytes = rows;
}

// No Rule: proof that L3 sees a claim that lands on an archived Topic. The
// product refuses the claim after the archive; the test gives the claim a
// fake answer of success with a landed publish, and writes the claim in the
// row of the archived Topic.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_claim_on_an_archived_topic_breaks_l3() {
    let case = Case {
        engines: 2,
        steps: alone(&[
            Operation::ArchiveRequirement { n: 0 },
            Operation::ClaimTopic,
        ]),
    };
    let mut run = run_case(&case, ONLY_L3);
    let claim = &mut run.done[1];
    assert!(claim.result.is_err(), "the product refuses the claim");
    claim.result = Ok(Effect::Claim {
        holder: Some("engine-0".to_owned()),
    });
    claim.kind = None;
    let operation = claim.operation;
    run.publishes.push(Publish {
        engine: Some(0),
        operation,
        outcome: Outcome::Landed,
        outside_writes: 0,
        whole_set: false,
        on_base: true,
    });
    claim_in_the_final_state(&mut run, "engine-0");
    expect_law(laws::check_only(&run, "L3"), "L3");
}

// No Rule: proof that L3 keeps the check of a claim through an archive of
// unknown outcome. The storage drops the claim of engine 0 and answers that
// it landed. The archive lands no key, so the Topic stays open and must
// hold the claim.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_claim_lost_before_an_archive_of_unknown_outcome_breaks_l3() {
    let mut steps = alone(&[Operation::ClaimTopic]);
    steps.push(Step::Fault(Fault::UnknownOutcome { landed: 0 }));
    steps.extend(alone(&[Operation::ArchiveRequirement { n: 0 }]));
    let case = Case { engines: 2, steps };
    let run = run_case(&case, planted(Plant::LoseWrite, "L3"));
    let archive = run.done[1].operation;
    let unknown = run
        .publishes
        .iter()
        .any(|publish| publish.operation == archive && publish.outcome == Outcome::Unknown);
    assert!(
        unknown,
        "the archive met no unknown outcome: {:?}",
        run.publishes
    );
    expect_law(laws::check_only(&run, "L3"), "L3");
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
