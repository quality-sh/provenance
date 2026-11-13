//! The cases of `engine.rs` through the public contract.
//!
//! Each engine sends its operations through `provenance_contract::invoke_with`
//! over the simulated storage. The run keeps the laws of every layer and the
//! laws O1 to O3 of this layer (`support/contract_laws.rs`). CI does not run
//! these tests: the ci profile of nextest leaves out this crate.

#[allow(dead_code)]
mod support {
    pub mod cases;
    pub mod contract;
    pub mod contract_laws;
    pub mod replay;
}

use hegel::TestCase;
use provenance_simulation::generate::{Case, Effect, Operation, Step, SCOPE};
use provenance_simulation::{check_case, check_probe, laws, report, run_case, stats};
use provenance_store::repository::Repository;
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use serde_json::json;
use std::sync::Arc;
use support::cases::{alone, each_operation};
use support::contract::{invoke, refusal_prefix, CONTRACT};
use support::contract_laws::contract_laws;
use support::replay::{replay_of_a_broken_probe, selection};

// No Rule: O1 to O3 have no accepted Rule; each is a check of the plan.
#[hegel::test]
#[ignore = "random cases run nightly: cargo nextest run --profile simulation --run-ignored all"]
fn random_cases_keep_the_laws_through_the_contract(tc: TestCase) {
    stats::record();
    check_case("contract", &Case::draw(&tc), CONTRACT);
}

/// A repository over the memory storage with one scope and nothing else.
fn empty_repository() -> Repository {
    use provenance_core::{Manifest, RepoPathPrefix};
    use provenance_store::storage::StateKey;
    let manifest =
        Manifest::default_with_scope(provenance_simulation::scope(), RepoPathPrefix::new("."));
    let entries = std::collections::BTreeMap::from([(
        StateKey::new("manifest.json").expect("a state key"),
        serde_json::to_vec(&manifest).expect("JSON"),
    )]);
    Repository::with_storage(
        provenance_simulation::bare_identity(),
        Arc::new(MemoryStateStorage::new(entries)),
        Arc::new(MemorySectionLock::default()),
        None,
    )
    .expect("the repository opens")
}

// No Rule: a probe of the plan. A catalog write runs through `invoke_with`
// on a repository over a storage port.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_create_source_runs_through_the_contract_on_a_storage_port() {
    let test = "a_create_source_runs_through_the_contract_on_a_storage_port";
    report::probe("contract", test, |_| {
        let repository = empty_repository();
        let source = invoke(
            &repository,
            "create-source",
            &json!({
                "scope_id": SCOPE, "id": "source_one", "name": "Policy", "source_type": "policy",
                "url": "https://example.com/policy", "reference": "section 1",
                "commit_pin": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "supersedes": [],
            }),
        )
        .unwrap_or_else(|failure| panic!("{}", serde_json::to_string(&failure).unwrap()));
        assert_eq!(source["id"], "source_one");
        let view = repository.view().unwrap();
        assert_eq!(
            view.list_sources(&provenance_simulation::scope())
                .unwrap()
                .len(),
            1
        );
    });
}

// No Rule: a probe of the plan. A stale `update-requirement` through the
// contract is a conflict (409), and the update before it stays.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn a_stale_update_through_the_contract_is_a_conflict() {
    let test = "a_stale_update_through_the_contract_is_a_conflict";
    report::probe("contract", test, |probe| {
        let update = |engine, stale| Step::Write {
            engine,
            op: Operation::UpdateRequirement { n: 0, stale },
        };
        let case = Case {
            engines: 2,
            steps: vec![update(0, false), update(1, true)],
        };
        let run = run_case(&case, CONTRACT);
        let results: Vec<_> = run.done.iter().map(|done| &done.result).collect();
        assert!(
            matches!(results[0], Ok(Effect::Described { .. })),
            "{results:?}"
        );
        assert!(
            matches!(results[1], Err(answer) if answer.starts_with(&refusal_prefix(409))),
            "{results:?}"
        );
        let verdict = laws::check(&run).and_then(|()| contract_laws(&run));
        probe.laws(verdict);
    });
}

// No Rule: a probe that each operation of the generator answers through the
// contract. Each write gets the turns it needs before the next one starts.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn each_operation_runs_through_the_contract() {
    let test = "each_operation_runs_through_the_contract";
    report::probe("contract", test, |probe| {
        let ops = each_operation();
        let case = Case {
            engines: 2,
            steps: alone(&ops),
        };
        let run = run_case(&case, CONTRACT);
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
        check_probe(probe, &case, CONTRACT);
    });
}

// No Rule: a probe that the replay line of a probe in this test target runs
// this test target.
#[test]
#[ignore = "a probe of the nightly simulation"]
fn the_replay_line_of_a_probe_runs_this_test_target() {
    let replay = replay_of_a_broken_probe("contract");
    let selection = selection(env!("CARGO_PKG_NAME"), env!("CARGO_CRATE_NAME"));
    assert!(replay.ends_with(&selection), "{replay}");
}
