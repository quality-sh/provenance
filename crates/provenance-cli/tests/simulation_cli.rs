//! The cases of the simulation through the real CLI on disk.
//!
//! Each write is one `provenance api` process on a fixture repository. A
//! `Turn` starts the next two writes at once. A `Stop` kills a write process
//! during its measured write time. An outside write runs on a git branch,
//! and git merges it back through `provenance merge-jsonl`. The run keeps
//! these laws:
//!
//! | Law | Statement |
//! | --- | --------- |
//! | L0 | Each process ends in the process limit, with no panic. |
//! | L3 | A write that succeeded stays unless a later write changed it. |
//! | L4 | Of two writes on one etag, at most one succeeds. |
//! | L6 | The reads after each write equal the reads after a full rebuild. |
//! | L7 | Each answer of one writer has the answer kind of the engine. |
//!
//! The nightly workflow runs this target on Linux, macOS and Windows. Two
//! processes at once make a case nondeterministic, so Hegel only warns. CI
//! does not run these tests: the ci profile of nextest leaves out this
//! binary.

#[allow(dead_code)]
mod support {
    // rustfmt reads this path from a directory with the name of the test
    // file, and the compiler from `tests/support/`; both name one file.
    #[path = "../support/api_process.rs"]
    pub mod api_process;
    #[path = "../../../provenance-transport/tests/support/catch_up_steps.rs"]
    pub mod catch_up_steps;
    #[path = "../../../provenance-transport/tests/support/transcript.rs"]
    pub mod transcript;
}

#[path = "simulation_cli/fixture.rs"]
mod fixture;
#[path = "simulation_cli/laws.rs"]
mod laws;
#[path = "simulation_cli/process.rs"]
mod process;
#[path = "simulation_cli/reads.rs"]
mod reads;

use fixture::run_cli;
use hegel::{Hegel, NondeterminismStrictness, Settings, TestCase};
use laws::{check, Ran};
use process::layer;
use provenance_simulation::generate::{Case, Fault, Operation, Step};
use provenance_simulation::report;

// No Rule: L3, L4, L6 and L7 have their basis tests; this is the random
// check of the plan on each system.
#[test]
#[ignore = "random cases run nightly: cargo nextest run --profile simulation --run-ignored all"]
fn random_cases_keep_the_laws_through_the_cli() {
    provenance_simulation::stats::record();
    let settings = Settings::new().nondeterminism_strictness(NondeterminismStrictness::Warn);
    Hegel::new(|tc: TestCase| {
        let case = Case::draw(&tc);
        if let Err(violation) = check(&case, &run_cli(&case)) {
            report::fail(&layer(), &case, &violation);
        }
    })
    .settings(settings)
    .run();
}

const fn write(engine: u8, op: Operation) -> Step {
    Step::Write { engine, op }
}

const fn update(n: u8) -> Operation {
    Operation::UpdateRequirement { n, stale: false }
}

// No Rule: a probe that every operation with a public route writes alone
// through the CLI.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn each_operation_writes_through_the_cli() {
    let steps = vec![
        write(0, Operation::CreateRequirement { n: 0 }),
        write(1, update(1)),
        write(1, Operation::SubmitReview),
        write(1, Operation::WithdrawReview { n: 0 }),
        write(0, update(0)),
        write(0, Operation::ClaimTopic),
        write(0, Operation::ReleaseTopic),
    ];
    let case = Case { engines: 2, steps };
    let run = run_cli(&case);
    let done: Vec<_> = run.groups.iter().flat_map(|(_, done)| done).collect();
    assert_eq!(done.len(), 7, "{done:?}");
    assert!(done.iter().all(|done| done.result.is_ok()), "{done:?}");
    let test = "each_operation_writes_through_the_cli";
    report::probe(&layer(), test, check(&case, &run));
}

// No Rule: a probe that two updates at once on one etag have one winner.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn two_updates_at_once_have_one_winner() {
    let steps = vec![write(0, update(0)), write(1, update(0)), Step::Turn(0)];
    let case = Case { engines: 2, steps };
    let run = run_cli(&case);
    let (way, done) = &run.groups[0];
    assert_eq!(*way, Ran::AtOnce);
    let won = done.iter().filter(|done| done.result.is_ok()).count();
    assert_eq!(won, 1, "{done:?}");
    let test = "two_updates_at_once_have_one_winner";
    report::probe(&layer(), test, check(&case, &run));
}

/// The most runs of the stop probe. A stop at `at: 0` kills the write at
/// half of its mean time, so a write that ends before the stop is rare.
const STOP_ATTEMPTS: usize = 5;

// No Rule: a probe that a write on a branch merges through `provenance
// merge-jsonl`, that a stop kills a write while it runs, and that the
// stopped write leaves a state that reads.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_merged_branch_and_a_stopped_write_keep_the_laws() {
    let steps = vec![
        write(0, update(0)),
        Step::OutsideWrite(Operation::CreateRequirement { n: 0 }),
        write(1, Operation::ClaimTopic),
        Step::Fault(Fault::Stop { engine: 1, at: 0 }),
    ];
    let case = Case { engines: 2, steps };
    for _ in 0..STOP_ATTEMPTS {
        let run = run_cli(&case);
        let ways: Vec<_> = run.groups.iter().map(|(way, _)| *way).collect();
        assert_eq!(ways, [Ran::Alone, Ran::Outside, Ran::Stopped]);
        let (_, outside) = &run.groups[1];
        assert!(outside[0].result.is_ok(), "{outside:?}");
        let test = "a_merged_branch_and_a_stopped_write_keep_the_laws";
        report::probe(&layer(), test, check(&case, &run));
        let (_, stopped) = &run.groups[2];
        if stopped.iter().any(|done| done.stopped) {
            return;
        }
    }
    panic!("in {STOP_ATTEMPTS} runs, each write ended or printed its answer before its stop");
}

// No Rule: a probe that the replay line of a probe in this test target runs
// this test target.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn the_replay_line_of_a_probe_runs_this_test_target() {
    let broken = Err(provenance_simulation::laws::Violation {
        law: "L3",
        detail: "a planted law break".to_owned(),
    });
    let payload = std::panic::catch_unwind(|| report::probe(&layer(), "a_planted_probe", broken))
        .expect_err("a broken probe fails");
    let text = report::panic_text(&*payload);
    let selection = format!(
        "-p {} --test {} -E 'test(=a_planted_probe)'",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_CRATE_NAME")
    );
    assert!(text.ends_with(&selection), "{text}");
}
