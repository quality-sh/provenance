//! The cases of the simulation through the real CLI on disk.
//!
//! Each write is one `provenance api` process on a fixture repository. A
//! `Turn` starts the next two writes at once. A `Stop` kills a write process
//! at a point of its publish that the test sees on disk (`stop.rs`). An
//! outside write runs on a git branch, and git merges it back through
//! `provenance merge-jsonl`. The run keeps these laws:
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
    #[path = "../../../provenance-transport/tests/support/names.rs"]
    pub mod names;
    #[path = "../../../provenance-transport/tests/support/transcript.rs"]
    pub mod transcript;
    #[path = "../../../provenance-transport/tests/support/views.rs"]
    pub mod views;
}

#[path = "simulation_cli/fixture.rs"]
mod fixture;
#[path = "simulation_cli/laws.rs"]
mod laws;
#[path = "simulation_cli/process.rs"]
mod process;
#[path = "simulation_cli/reads.rs"]
mod reads;

use fixture::{run_cli, run_steps, state_entries};
use hegel::{Hegel, NondeterminismStrictness, Settings, TestCase};
use laws::{check, Ran};
use process::stop::{Landing, Stop};
use process::{etag, layer};
use provenance_simulation::generate::{Case, Fault, Operation, Step, FIRST_REQUIREMENT};
use provenance_simulation::report;
use std::time::Instant;

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
    report::probe_laws(&layer(), check(&case, &run));
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
    report::probe_laws(&layer(), check(&case, &run));
}

/// Runs a case with a stop at `at` and checks it: an update, a create on a
/// branch that git merges back through `provenance merge-jsonl`, and a claim
/// that the stop kills. The stop must land at `landing`, which is before the
/// install of the new state. After the kill, one read runs through the CLI.
/// Then the marker is gone, the state is the state before the claim, and the
/// transaction of the claim stays only for a stop before the marker. The run
/// must keep the laws.
fn stop_probe(at: u8, landing: Landing) {
    let started = Instant::now();
    let steps = vec![
        write(0, update(0)),
        Step::OutsideWrite(Operation::CreateRequirement { n: 0 }),
        write(1, Operation::ClaimTopic),
        Step::Fault(Fault::Stop { engine: 1, at }),
    ];
    let case = Case { engines: 2, steps };
    let fixture = run_steps(&case);
    let ways: Vec<_> = fixture.groups.iter().map(|(way, _)| *way).collect();
    assert_eq!(ways, [Ran::Alone, Ran::Outside, Ran::Stopped]);
    let (_, outside) = &fixture.groups[1];
    assert!(outside[0].result.is_ok(), "{outside:?}");
    let [Stop::Landed(landed)] = fixture.stops.as_slice() else {
        let rows: Vec<_> = fixture.stops.iter().map(Stop::row).collect();
        panic!("the stop did not kill the claim before its answer: {rows:?}");
    };
    assert_eq!(landed.landing, landing, "the stop landed at another point");
    let root = fixture.root();
    etag(&root, FIRST_REQUIREMENT).expect("the read after the stop answers");
    let marker = fixture.repo.layout.publication_marker_path();
    assert!(!marker.exists(), "the read after the stop left the marker");
    assert_eq!(
        state_entries(&root),
        landed.before,
        "after the read, the state is not the state before the claim"
    );
    // Recovery removes only the transaction that a marker names.
    assert_eq!(
        landed.transaction.exists(),
        landing == Landing::BeforeMarker,
        "the transaction of the claim after the read"
    );
    report::probe_laws(&layer(), check(&case, &fixture.finish(started)));
}

// No Rule: a probe of the nightly simulation. It runs `stop_probe` with the
// stop at the transaction, which must land before the marker.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_merged_branch_and_a_stop_before_the_marker_keep_the_laws() {
    stop_probe(0, Landing::BeforeMarker);
}

// No Rule: a probe of the nightly simulation. It runs `stop_probe` with the
// stop at the marker, which must land after the marker and before the
// install, so that the read after the stop recovers the publish.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_merged_branch_and_a_stop_after_the_marker_keep_the_laws() {
    stop_probe(1, Landing::BeforeInstall);
}

// No Rule: a probe that each kind of write that the simulation sends
// publishes through a transaction: a stop at the transaction kills each
// kind before its answer.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_stop_at_the_transaction_kills_each_kind_of_write() {
    let stop = Step::Fault(Fault::Stop { engine: 1, at: 0 });
    let steps = vec![
        write(1, Operation::CreateRequirement { n: 0 }),
        stop.clone(),
        write(1, update(0)),
        stop.clone(),
        write(1, Operation::SubmitReview),
        stop.clone(),
        write(1, Operation::SubmitReview),
        Step::Turn(1),
        write(1, Operation::WithdrawReview { n: 0 }),
        stop.clone(),
        write(1, Operation::ClaimTopic),
        stop.clone(),
        write(1, Operation::ClaimTopic),
        Step::Turn(1),
        write(1, Operation::ReleaseTopic),
        stop,
    ];
    let case = Case { engines: 2, steps };
    let run = run_cli(&case);
    let stopped: Vec<_> = run
        .groups
        .iter()
        .filter(|(way, _)| *way == Ran::Stopped)
        .flat_map(|(_, done)| done)
        .map(|done| done.op.clone())
        .collect();
    let kinds = [
        Operation::CreateRequirement { n: 0 },
        update(0),
        Operation::SubmitReview,
        Operation::WithdrawReview { n: 0 },
        Operation::ClaimTopic,
        Operation::ReleaseTopic,
    ];
    assert_eq!(stopped, kinds);
    let rows: Vec<_> = run.stops.iter().map(Stop::row).collect();
    let landed = run.stops.iter().all(|stop| matches!(stop, Stop::Landed(_)));
    assert!(
        landed,
        "a stop did not kill its write before its answer: {rows:?}"
    );
    report::probe_laws(&layer(), check(&case, &run));
}
