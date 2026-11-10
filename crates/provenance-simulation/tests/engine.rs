//! Engines over one simulated storage keep the laws of `laws.rs`.
//!
//! The nightly workflow runs this target with `HEGEL_SEED` and
//! `HEGEL_TEST_CASES`. A broken law fails the case; Hegel shrinks it and
//! prints the `#[hegel::reproduce_failure]` line that replays it. CI does
//! not run these tests: the ci profile of nextest leaves out this crate.

#[allow(dead_code)]
mod support {
    pub mod cases;
}

use hegel::TestCase;
use provenance_macros::verifies;
use provenance_simulation::generate::{Case, Effect, Fault, Operation, Step};
use provenance_simulation::storage::Outcome;
use provenance_simulation::{check_case, check_probe, laws, report, run_case, Config, Run};
use support::cases::{alone, each_operation, moved_base_case, write};

#[hegel::test]
#[ignore = "random cases run nightly: cargo nextest run --profile simulation --run-ignored all"]
#[verifies("rule_write_sets_publish_through_the_storage_port", property)]
fn random_cases_keep_the_laws(tc: TestCase) {
    check_case("engine", &Case::draw(&tc), Config::default());
}

fn met(run: &Run, outcome: Outcome) -> bool {
    run.publishes
        .iter()
        .any(|publish| publish.outcome == outcome)
}

// No Rule: a probe that the harness can make a moved base; L5 is the law.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn the_write_after_a_moved_base_lands() {
    let run = run_case(&moved_base_case(), Config::default());
    assert!(met(&run, Outcome::BaseMoved), "the case met no moved base");
    assert!(
        matches!(run.done[1].result, Ok(Effect::Bound { .. })),
        "{:?}",
        run.done
    );
    let test = "the_write_after_a_moved_base_lands";
    report::probe("engine", test, laws::check(&run));
}

// No Rule: a probe that each fault of the generator meets an engine.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_case_with_every_fault_keeps_the_laws() {
    let update = Operation::UpdateRequirement { n: 0, stale: false };
    let mut steps = vec![
        Step::Fault(Fault::UnknownOutcome { landed: 1 }),
        write(2, Operation::ClaimTopic),
    ];
    // The claim publishes before another engine holds the section and before
    // the next fault, so it meets the unknown outcome.
    steps.extend(std::iter::repeat_n(Step::Turn(2), 50));
    steps.extend([
        write(0, update.clone()),
        write(1, update),
        Step::Turn(0),
        Step::Turn(1),
        Step::Fault(Fault::Stop { engine: 1, at: 0 }),
        Step::Fault(Fault::Refuse),
        write(1, Operation::SubmitReview),
        Step::OutsideWrite(Operation::CreateRequirement { n: 1 }),
        write(0, Operation::PostMessage),
        write(2, Operation::ReleaseTopic),
    ]);
    let case = Case { engines: 3, steps };
    let test = "a_case_with_every_fault_keeps_the_laws";
    let run = check_probe("engine", test, &case, Config::default());
    assert!(
        met(&run, Outcome::Unknown),
        "no unknown outcome: {:?}",
        run.publishes
    );
    assert!(
        met(&run, Outcome::Refused),
        "no refusal: {:?}",
        run.publishes
    );
    assert!(
        run.done
            .iter()
            .any(|done| done.engine.is_none() && done.result.is_ok()),
        "no outside write: {:?}",
        run.done
    );
}

// No Rule: a probe that an unknown outcome lands a prefix and the laws hold.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn an_unknown_outcome_lands_a_prefix_and_keeps_the_laws() {
    for landed in 0..3 {
        let case = Case {
            engines: 2,
            steps: vec![
                Step::Fault(Fault::UnknownOutcome { landed }),
                write(0, Operation::PostMessage),
                write(1, Operation::UpdateRequirement { n: 0, stale: false }),
            ],
        };
        let run = run_case(&case, Config::default());
        assert!(met(&run, Outcome::Unknown), "the fault met a publish");
        let test = "an_unknown_outcome_lands_a_prefix_and_keeps_the_laws";
        report::probe("engine", test, laws::check(&run));
    }
}

// No Rule: a probe that each operation of the generator succeeds alone.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn each_operation_runs_on_the_engine() {
    let ops = each_operation();
    let case = Case {
        engines: 2,
        steps: alone(&ops),
    };
    let test = "each_operation_runs_on_the_engine";
    let run = check_probe("engine", test, &case, Config::default());
    let ran: Vec<_> = run.done.iter().map(|done| &done.op).collect();
    assert_eq!(ran, ops.iter().collect::<Vec<_>>());
    for done in &run.done {
        assert!(
            matches!(&done.result, Ok(effect) if *effect != Effect::Skipped),
            "{:?} gave {:?}",
            done.op,
            done.result
        );
    }
    for done in &run.done {
        if matches!(
            done.op,
            Operation::DecideReview { .. } | Operation::ArchiveRequirement { .. }
        ) {
            let staged = run.publishes.iter().any(|publish| {
                publish.operation == done.operation
                    && publish.whole_set
                    && publish.outcome == Outcome::Landed
            });
            assert!(staged, "{:?} landed no stage: {:?}", done.op, run.publishes);
        }
    }
}
