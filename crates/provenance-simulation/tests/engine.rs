//! Engines over one simulated storage keep the laws of `laws.rs`.
//!
//! The nightly workflow runs this target with `HEGEL_SEED` and
//! `HEGEL_TEST_CASES`. A broken law fails the case; Hegel shrinks it and
//! prints the `#[hegel::reproduce_failure]` line that replays it. CI does
//! not run these tests: the ci profile of nextest leaves out this crate.

#[allow(dead_code)]
mod support {
    pub mod cases;
    pub mod replay;
}

use hegel::TestCase;
use provenance_macros::verifies;
use provenance_simulation::generate::{Case, Effect, Fault, Operation, Step};
use provenance_simulation::storage::Outcome;
use provenance_simulation::{check_case, check_probe, laws, report, run_case, stats, Config, Run};
use support::cases::{alone, each_operation, moved_base_case, write};
use support::replay::{replay_of_a_broken_probe, selection};

#[hegel::test]
#[ignore = "random cases run nightly: cargo nextest run --profile simulation --run-ignored all"]
#[verifies("rule_write_sets_publish_through_the_storage_port", property)]
fn random_cases_keep_the_laws(tc: TestCase) {
    stats::record();
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
    let test = "the_write_after_a_moved_base_lands";
    report::probe("engine", test, |probe| {
        let run = run_case(&moved_base_case(), Config::default());
        assert!(met(&run, Outcome::BaseMoved), "the case met no moved base");
        assert!(
            matches!(run.done[1].result, Ok(Effect::Bound { .. })),
            "{:?}",
            run.done
        );
        probe.laws(laws::check(&run));
    });
}

/// A case where each fault of the generator meets an engine.
fn every_fault_case() -> Case {
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
    Case { engines: 3, steps }
}

// No Rule: a probe that each fault of the generator meets an engine.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_case_with_every_fault_keeps_the_laws() {
    let test = "a_case_with_every_fault_keeps_the_laws";
    report::probe("engine", test, |probe| {
        let run = check_probe(probe, &every_fault_case(), Config::default());
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
        assert!(
            run.done.iter().any(|done| done.stopped),
            "the stop met no engine during an operation: {:?}",
            run.done
        );
    });
}

// No Rule: a probe that an unknown outcome lands a prefix and the laws hold.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn an_unknown_outcome_lands_a_prefix_and_keeps_the_laws() {
    let test = "an_unknown_outcome_lands_a_prefix_and_keeps_the_laws";
    report::probe("engine", test, |probe| {
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
            probe.laws(laws::check(&run));
        }
    });
}

// No Rule: a probe that each operation of the generator succeeds alone.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn each_operation_runs_on_the_engine() {
    let test = "each_operation_runs_on_the_engine";
    report::probe("engine", test, |probe| {
        let ops = each_operation();
        let case = Case {
            engines: 2,
            steps: alone(&ops),
        };
        let run = check_probe(probe, &case, Config::default());
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
    });
}

// No Rule: a probe that L3 follows the claim through an archive. The archive
// of the Requirement of the Topic reads the state after the outside claim and
// lands after it, so the Topic is archived and holds no claim.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn an_archive_after_an_outside_claim_clears_the_claim_and_keeps_l3() {
    let test = "an_archive_after_an_outside_claim_clears_the_claim_and_keeps_l3";
    report::probe("engine", test, |probe| {
        let case = Case {
            engines: 2,
            steps: vec![
                write(0, Operation::ArchiveRequirement { n: 0 }),
                Step::OutsideWrite(Operation::ClaimTopic),
            ],
        };
        let run = run_case(&case, Config::default());
        let landed: Vec<_> = run
            .publishes
            .iter()
            .filter(|publish| publish.outcome == Outcome::Landed)
            .map(|publish| publish.engine)
            .collect();
        assert_eq!(landed, [None, Some(0)], "{:?}", run.publishes);
        assert!(
            run.done.iter().all(|done| done.result.is_ok()),
            "{:?}",
            run.done
        );
        let repository = provenance_simulation::open_entries(&run.entries).unwrap();
        let topics = repository
            .view()
            .unwrap()
            .list_topics(&provenance_simulation::scope())
            .unwrap();
        assert_eq!(topics[0].status, provenance_core::TopicStatus::Archived);
        assert_eq!(topics[0].claimed_by, None);
        probe.laws(laws::check(&run));
    });
}

// No Rule: a probe that the replay line of a probe in this test target runs
// this test target.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn the_replay_line_of_a_probe_runs_this_test_target() {
    let replay = replay_of_a_broken_probe("engine");
    let selection = selection(env!("CARGO_PKG_NAME"), env!("CARGO_CRATE_NAME"));
    assert!(replay.ends_with(&selection), "{replay}");
}

// No Rule: a probe that the replay line names each test target of this
// crate. Cargo names a test binary with `_` in place of each `-` of its
// target name, and the replay line takes the target name from the binary.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn the_replay_line_names_each_test_target_of_this_crate() {
    let tests = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut targets = Vec::new();
    for entry in std::fs::read_dir(&tests).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            targets.push(path.file_stem().unwrap().to_str().unwrap().to_owned());
        }
    }
    assert!(targets.contains(&"refused_post".to_owned()), "{targets:?}");
    for target in targets {
        let crate_name = target.replace('-', "_");
        for binary in [
            format!("{crate_name}-0123456789abcdef"),
            format!("{crate_name}-0123456789abcdef.exe"),
        ] {
            let exe = std::path::Path::new("target/debug/deps").join(binary);
            assert_eq!(report::test_target(&exe), Some(target.clone()), "{exe:?}");
        }
    }
}
