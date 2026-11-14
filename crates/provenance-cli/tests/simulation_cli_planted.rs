//! Planted faults of the CLI laws. Each test plants the smallest fault that
//! one law must see, and asserts that the law reports it by its id. A
//! planted run writes no failure file, and a test whose law does not see its
//! fault fails with "simulation check broken", so the nightly report opens no
//! law issue for it. CI does not run these tests: the ci profile of nextest
//! leaves out this binary.

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

#[allow(dead_code)]
#[path = "simulation_cli_support/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "simulation_cli_support/laws.rs"]
mod laws;
#[allow(dead_code)]
#[path = "simulation_cli_support/process.rs"]
mod process;
#[path = "simulation_cli_support/reads.rs"]
mod reads;

use fixture::{run_cli, run_steps};
use laws::check;
use process::git;
use provenance_simulation::generate::{Case, Effect, Operation, Step, Task};
use provenance_simulation::laws::Violation;
use provenance_simulation::report;
use provenance_store::repository::Repository;
use std::time::Instant;

/// Fails with the marker of a broken check unless `verdict` names `law`.
fn expect_law(verdict: Result<(), Violation>, law: &str) {
    match verdict {
        Err(violation) if violation.law == law => {}
        Err(violation) => report::check_broken(law, &format!("{violation:?}")),
        Ok(()) => report::check_broken(law, "every law held"),
    }
}

const fn write(engine: u8, op: Operation) -> Step {
    Step::Write { engine, op }
}

const fn update(n: u8) -> Operation {
    Operation::UpdateRequirement { n, stale: false }
}

// No Rule: proof that L3 can fail. After the last write, the state file of
// the Requirements is the copy of the initial commit again.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_restored_state_file_breaks_l3() {
    let case = Case {
        engines: 2,
        steps: vec![write(0, update(0))],
    };
    let fixture = run_steps(&case);
    let file = ".provenance/state/scopes/default/requirements/req.jsonl";
    let restored = git(&fixture.root(), &["checkout", &fixture.base, "--", file]);
    restored.expect("the initial copy of the file");
    let run = fixture.finish(Instant::now());
    expect_law(check(&case, &run), "L3");
}

// No Rule: proof that L4 can fail. The second update was sent with the
// fresh etag, and the run records the etag of the first one for it.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_stale_etag_recorded_for_a_fresh_write_breaks_l4() {
    let case = Case {
        engines: 2,
        steps: vec![write(0, update(0)), write(1, update(0))],
    };
    let mut run = run_cli(&case);
    let mut described = run
        .groups
        .iter_mut()
        .flat_map(|(_, done)| done.iter_mut())
        .filter_map(|done| match &mut done.result {
            Ok(Effect::Described { etag, .. }) => Some(etag),
            _ => None,
        });
    let (Some(first), Some(second)) = (described.next(), described.next()) else {
        report::check_broken("L4", "the two updates did not both succeed");
    };
    second.clone_from(first);
    expect_law(check(&case, &run), "L4");
}

// No Rule: proof that L6 can fail. After the last write and a read, one
// description in the projection files has other bytes of the same length.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_changed_projection_row_breaks_l6() {
    let case = Case {
        engines: 2,
        steps: vec![write(0, update(0))],
    };
    let mut fixture = run_steps(&case);
    // A read catches up the projection, so that it holds the description.
    fixture.l6("the reads before the planted change of the projection");
    let (from, to) = (
        b"Description from operation 1",
        b"Xescription from operation 1",
    );
    let mut changed = 0;
    let cache = fixture.repo.layout.cache_dir();
    for entry in std::fs::read_dir(cache.as_std_path()).expect("a projection directory") {
        let path = entry.expect("an entry").path();
        let Ok(mut bytes) = std::fs::read(&path) else {
            continue;
        };
        let mut at = 0;
        while let Some(found) = bytes[at..]
            .windows(from.len())
            .position(|part| part == from)
        {
            let start = at + found;
            bytes[start..start + to.len()].copy_from_slice(to);
            at = start + to.len();
            changed += 1;
        }
        std::fs::write(&path, bytes).expect("a projection file");
    }
    if changed == 0 {
        report::check_broken("L6", "the projection files hold no description");
    }
    fixture.l6("the reads after a planted change of the projection");
    let run = fixture.finish(Instant::now());
    expect_law(check(&case, &run), "L6");
}

/// An engine that refuses each claim and runs each other write as the
/// public API does.
fn refusing_claims(repository: &Repository, task: &Task) -> anyhow::Result<Effect> {
    if task.op == Operation::ClaimTopic {
        anyhow::bail!("the stand-in engine refuses the claim");
    }
    laws::apply_public(repository, task)
}

// No Rule: proof that L7 can fail. The engine stand-in refuses a claim that
// the CLI accepts.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn an_engine_that_refuses_a_claim_breaks_l7() {
    let case = Case {
        engines: 2,
        steps: vec![write(0, Operation::ClaimTopic)],
    };
    let run = run_cli(&case);
    let verdict = laws::one_writer_answers_as_the_engine(&case, &run, refusing_claims);
    expect_law(verdict, "L7");
}
