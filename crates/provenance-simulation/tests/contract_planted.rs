//! Planted faults of the contract laws. Each test plants the smallest fault
//! that one law must see, runs real steps through the contract, and asserts
//! that the law reports the fault by its id. A planted run writes no failure
//! file. CI does not run these tests: the ci profile of nextest leaves out
//! this crate.

#[allow(dead_code)]
mod support {
    pub mod cases;
    pub mod contract;
    pub mod contract_laws;
}

use provenance_simulation::generate::{Case, Effect, Operation, Task};
use provenance_simulation::laws::Violation;
use provenance_simulation::{report, run_case, Config, Plant};
use provenance_store::repository::Repository;
use support::cases::alone;
use support::contract::{apply_contract, CONTRACT};
use support::contract_laws::{
    message_times_increase, no_write_failure_without_a_fault, stages_land_whole,
};

/// Fails with the marker of a broken check unless `verdict` names `law`.
fn expect_law(verdict: Result<(), Violation>, law: &str) {
    match verdict {
        Err(violation) if violation.law == law => {}
        Err(violation) => report::check_broken(law, &format!("{violation:?}")),
        Ok(()) => report::check_broken(law, "every law held"),
    }
}

const fn planted(plant: Option<Plant>, law: &'static str) -> Config {
    Config {
        plant,
        only_law: Some(law),
        ..CONTRACT
    }
}

// No Rule: proof that O1 can fail; O1 has no accepted Rule. The first read
// of the engine fails, and the contract answers a write failure for a step
// that met no planted publish fault.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_failed_read_breaks_o1() {
    let case = Case {
        engines: 2,
        steps: alone(&[Operation::PostMessage]),
    };
    let run = run_case(&case, planted(Some(Plant::FailRead), "O1"));
    expect_law(no_write_failure_without_a_fault(&run), "O1");
}

// No Rule: proof that O2 can fail. The storage sets the time of each
// message that it lands to zero, so the second message has no later time.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn messages_without_later_times_break_o2() {
    let case = Case {
        engines: 2,
        steps: alone(&[Operation::PostMessage, Operation::PostMessage]),
    };
    let run = run_case(&case, planted(Some(Plant::BackdateMessage), "O2"));
    expect_law(message_times_increase(&run), "O2");
}

/// The contract, with a second empty stage after each decision.
fn decide_in_two_stages(repository: &Repository, task: &Task) -> anyhow::Result<Effect> {
    let effect = apply_contract(repository, task)?;
    if matches!(task.op, Operation::DecideReview { .. }) {
        repository.stage(false, None, |_| Ok(()))?;
    }
    Ok(effect)
}

// No Rule: proof that O3 can fail. A stand-in lands a decision in two
// whole sets.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_decision_in_two_stages_breaks_o3() {
    let case = Case {
        engines: 2,
        steps: alone(&[Operation::SubmitReview, Operation::DecideReview { n: 0 }]),
    };
    let config = Config {
        apply: Some(decide_in_two_stages),
        ..planted(None, "O3")
    };
    let run = run_case(&case, config);
    expect_law(stages_land_whole(&run), "O3");
}
