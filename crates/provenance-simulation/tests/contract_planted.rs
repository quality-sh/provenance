//! Planted faults of the contract laws. Each test plants the smallest fault
//! that one law must see and asserts that the law reports it by its id. A
//! planted run writes no failure file. CI does not run these tests: the ci
//! profile of nextest leaves out this crate.

#[allow(dead_code)]
mod support {
    pub mod cases;
    pub mod contract;
    pub mod contract_laws;
}

use provenance_simulation::generate::{Case, Operation};
use provenance_simulation::laws::Violation;
use provenance_simulation::{report, run_case, Done};
use support::contract::{refusal_prefix, CONTRACT};
use support::contract_laws::contract_laws;

/// Fails with the marker of a broken check unless `verdict` names `law`.
fn expect_law(verdict: Result<(), Violation>, law: &str) {
    match verdict {
        Err(violation) if violation.law == law => {}
        Err(violation) => report::check_broken(law, &format!("{violation:?}")),
        Ok(()) => report::check_broken(law, "every law held"),
    }
}

// No Rule: proof that O1 can fail; O1 has no accepted Rule.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_write_failure_without_a_fault_breaks_o1() {
    let mut run = run_case(
        &Case {
            engines: 2,
            steps: Vec::new(),
        },
        CONTRACT,
    );
    run.done.push(Done {
        engine: Some(0),
        operation: 1,
        op: Operation::PostMessage,
        target: None,
        result: Err(format!("{} {{}}", refusal_prefix(500))),
        kind: Some("write_failed".into()),
        stopped: false,
    });
    expect_law(contract_laws(&run), "O1");
}
