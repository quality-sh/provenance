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
//! | L0 | Each process ends in the process limit. |
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
    #[path = "api_process.rs"]
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
use provenance_simulation::laws::Violation;
use provenance_simulation::report;
use std::path::Path;

/// Fails the case with the report of `violation`.
fn fail(case: &Case, violation: &Violation) -> ! {
    let layer = layer();
    let seed = report::seed();
    let failure = report::Failure {
        layer: &layer,
        law: violation.law,
        detail: &violation.detail,
        replay: report::replay_line(&layer, seed.as_deref()),
        seed,
        case,
    };
    let json = serde_json::to_string_pretty(&failure).expect("a failure is JSON");
    if let Some(directory) = std::env::var_os("SIMULATION_FAILURE_DIR") {
        let path = Path::new(&directory).join(format!("{layer}-{}.json", violation.law));
        let written = std::fs::create_dir_all(&directory);
        if let Err(error) = written.and_then(|()| std::fs::write(path, &json)) {
            eprintln!("could not write the failure: {error}");
        }
    }
    let (law, detail, replay) = (violation.law, &violation.detail, &failure.replay);
    let text = format!("{detail}\nreplay: {replay}\nsteps: {json}");
    panic!("simulation law {law} broken in layer {layer}: {text}");
}

// No Rule: L3, L4, L6 and L7 have their basis tests; this is the random
// check of the plan on each system.
#[test]
#[ignore = "random cases run nightly: cargo nextest run --profile simulation --run-ignored all"]
fn random_cases_keep_the_laws_through_the_cli() {
    let settings = Settings::new().nondeterminism_strictness(NondeterminismStrictness::Warn);
    Hegel::new(|tc: TestCase| {
        let case = Case::draw(&tc);
        if let Err(violation) = check(&case, &run_cli(&case)) {
            fail(&case, &violation);
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
    check(&case, &run).unwrap();
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
    check(&case, &run).unwrap();
}

// No Rule: a probe that a write on a branch merges through `provenance
// merge-jsonl`, and that a stopped write leaves a state that reads.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_merged_branch_and_a_stopped_write_keep_the_laws() {
    let steps = vec![
        write(0, update(0)),
        Step::OutsideWrite(Operation::CreateRequirement { n: 0 }),
        write(1, Operation::ClaimTopic),
        Step::Fault(Fault::Stop { engine: 1, at: 128 }),
    ];
    let case = Case { engines: 2, steps };
    let run = run_cli(&case);
    let ways: Vec<_> = run.groups.iter().map(|(way, _)| *way).collect();
    assert_eq!(ways, [Ran::Alone, Ran::Outside, Ran::Stopped]);
    let (_, outside) = &run.groups[1];
    assert!(outside[0].result.is_ok(), "{outside:?}");
    check(&case, &run).unwrap();
}
