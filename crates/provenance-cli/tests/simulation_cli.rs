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

#[path = "simulation_cli_support/fixture.rs"]
mod fixture;
#[path = "simulation_cli_support/laws.rs"]
mod laws;
#[path = "simulation_cli_support/process.rs"]
mod process;
#[path = "simulation_cli_support/reads.rs"]
mod reads;

use fixture::{run_cli, run_steps, state_entries};
use hegel::{Hegel, NondeterminismStrictness, Settings, TestCase};
use laws::{check, Ran};
use process::stop::{Landing, Stop};
use process::{etag, layer};
use provenance_simulation::generate::{Case, Fault, Operation, Step, FIRST_REQUIREMENT};
use provenance_simulation::report;
use provenance_store::storage::StateKey;
use std::collections::BTreeSet;
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
/// branch that git merges back through `provenance merge-jsonl`, and the
/// write `stopped` that the stop kills. The stop must land at one of
/// `landings`. A one-key write can also end before the kill: then it must
/// answer success. After the stop, one read runs through the CLI. Then no
/// marker and no transaction are there, and the state is the state before
/// `stopped`, with only the Topic key changed for a stop after a rename or a
/// write that ended. The temporary file of a one-key write stays only for a
/// stop before its rename, and no read sees it. The run must keep the laws.
fn stop_probe(stopped: Operation, at: u8, landings: &[Landing]) {
    let started = Instant::now();
    let steps = vec![
        write(0, update(0)),
        Step::OutsideWrite(Operation::CreateRequirement { n: 0 }),
        write(1, stopped),
        Step::Fault(Fault::Stop { engine: 1, at }),
    ];
    let case = Case { engines: 2, steps };
    let fixture = run_steps(&case);
    let ways: Vec<_> = fixture.groups.iter().map(|(way, _)| *way).collect();
    assert_eq!(ways, [Ran::Alone, Ran::Outside, Ran::Stopped]);
    let (_, outside) = &fixture.groups[1];
    assert!(outside[0].result.is_ok(), "{outside:?}");
    let one_key = landings.contains(&Landing::BeforeRename);
    let (landing, before) = match fixture.stops.as_slice() {
        [Stop::Landed(landed)] => {
            assert!(
                landings.contains(&landed.landing),
                "the stop landed at another point: {:?}",
                landed.landing
            );
            (Some(landed), &landed.before)
        }
        [Stop::Answered { before } | Stop::Ended { before }] if one_key => {
            let (_, stopped) = &fixture.groups[2];
            assert!(stopped[0].result.is_ok(), "{stopped:?}");
            (None, before)
        }
        other => {
            let rows: Vec<_> = other.iter().map(Stop::row).collect();
            panic!("the stop did not kill the write before its answer: {rows:?}");
        }
    };
    let row = fixture.stops[0].row();
    let root = fixture.root();
    etag(&root, FIRST_REQUIREMENT).expect("the read after the stop answers");
    let layout = &fixture.repo.layout;
    assert!(
        !layout.publication_marker_path().exists(),
        "{row}: the read after the stop left the marker"
    );
    let transactions =
        std::fs::read_dir(layout.import_transactions_dir()).map_or(0, std::iter::Iterator::count);
    assert_eq!(transactions, 0, "{row}: a transaction stays after the read");
    let after = state_entries(&root).expect("the state reads after the stop");
    let before = before.clone().expect("the state read before the write");
    let changed: BTreeSet<_> = before
        .keys()
        .chain(after.keys())
        .filter(|key| before.get(*key) != after.get(*key))
        .map(StateKey::as_str)
        .collect();
    let landing = landing.map(|landed| landed.landing);
    if one_key && landing != Some(Landing::BeforeRename) {
        assert!(
            changed.len() == 1 && changed.iter().all(|key| key.contains("/topics/")),
            "{row}: after the read, keys other than the Topic key changed: {changed:?}"
        );
    } else {
        assert!(
            changed.is_empty(),
            "{row}: after the read, the state is not the state before the write: {changed:?}"
        );
    }
    // The section entry removes a transaction with no marker, and recovery
    // the transaction that a marker names. Nothing removes a temporary file
    // that a stop left before the rename.
    if let [Stop::Landed(landed)] = fixture.stops.as_slice() {
        assert_eq!(
            landed.made.exists(),
            landed.landing == Landing::BeforeRename,
            "{row}: the transaction or the temporary file of the write after the read"
        );
    }
    report::probe_laws(&layer(), check(&case, &fixture.finish(started)));
}

// No Rule: a probe of the nightly simulation. It runs `stop_probe` on an
// update of two keys with the stop at the transaction, which must land
// before the marker.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_merged_branch_and_a_stop_before_the_marker_keep_the_laws() {
    stop_probe(update(0), 0, &[Landing::BeforeMarker]);
}

// No Rule: a probe of the nightly simulation. It runs `stop_probe` on an
// update of two keys with the stop at the marker, which must land after the
// marker and before the install, so that the read after the stop recovers
// the publish.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_merged_branch_and_a_stop_after_the_marker_keep_the_laws() {
    stop_probe(update(0), 1, &[Landing::BeforeInstall]);
}

// No Rule: a probe of the nightly simulation. It runs `stop_probe` on a
// claim, a write of one key, with the stop when its temporary file appears.
// The rename comes one file sync later, so the kill lands before or after
// it, or the write ends first. Each landing has its own asserts.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_merged_branch_and_a_stop_of_a_one_key_write_keep_the_laws() {
    stop_probe(
        Operation::ClaimTopic,
        0,
        &[Landing::BeforeRename, Landing::AfterRename],
    );
}

// No Rule: a probe that each kind of write that the simulation sends
// publishes through a transaction or a temporary file. A stop at the first
// point kills a write of two keys before the marker. A write of one key
// stops before or after its rename, or ends with a success first. A record
// has one pending submission at most. So a submit that no stop meets gives
// the stopped withdrawal its proposal, and a second withdrawal ends that
// submission if the stop landed before the rename. Then the stopped submit
// has no pending submission in its way.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_stop_at_the_first_point_of_each_kind_of_write_keeps_the_laws() {
    let stop = Step::Fault(Fault::Stop { engine: 1, at: 0 });
    let steps = vec![
        write(1, Operation::CreateRequirement { n: 0 }),
        stop.clone(),
        write(1, update(0)),
        stop.clone(),
        write(1, Operation::SubmitReview),
        Step::Turn(1),
        write(1, Operation::WithdrawReview { n: 0 }),
        stop.clone(),
        write(1, Operation::WithdrawReview { n: 0 }),
        Step::Turn(1),
        write(1, Operation::SubmitReview),
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
        .collect();
    let kinds = [
        Operation::CreateRequirement { n: 0 },
        update(0),
        Operation::WithdrawReview { n: 0 },
        Operation::SubmitReview,
        Operation::ClaimTopic,
        Operation::ReleaseTopic,
    ];
    let ops: Vec<_> = stopped.iter().map(|done| done.op.clone()).collect();
    assert_eq!(ops, kinds);
    assert_eq!(run.stops.len(), kinds.len());
    let one_key = [Landing::BeforeRename, Landing::AfterRename];
    let first_point =
        run.stops
            .iter()
            .zip(&stopped)
            .enumerate()
            .all(|(n, (stop, done))| match stop {
                Stop::Landed(landed) if n < 2 => landed.landing == Landing::BeforeMarker,
                Stop::Landed(landed) => one_key.contains(&landed.landing),
                Stop::Answered { .. } | Stop::Ended { .. } => n >= 2 && done.result.is_ok(),
                _ => false,
            });
    let rows: Vec<_> = run.stops.iter().map(Stop::row).collect();
    assert!(
        first_point,
        "a stop did not land at the first point of its write: {rows:?}"
    );
    report::probe_laws(&layer(), check(&case, &run));
}
