//! Planted faults of L3. Each test runs real steps with a planted storage
//! fault or a stand-in write, and asserts that L3 reports the fault.
//!
//! A stand-in write lands a Topic value that its write cannot give: a claim
//! for a different actor, a release that closes the Topic, or an archive
//! that claims the Topic. The tests run each stand-in with each outcome of
//! the write: certain, unknown, failed after its publish landed, and
//! stopped after its publish landed. L3 accepts the value that the Topic
//! held before the write or the value of the write, so each one breaks L3.
//! One more test checks the model of L3 for a group of writes.

use crate::support::cases::alone;
use crate::{expect_law, planted};
use provenance_core::TopicStatus;
use provenance_simulation::expected::{Expected, Write};
use provenance_simulation::generate::{self, Case, Effect, Fault, Operation, Step, Target, Task};
use provenance_simulation::generate::{Value, TOPIC};
use provenance_simulation::laws;
use provenance_simulation::storage::Outcome;
use provenance_simulation::{run_case, scope, stable, Apply, Config, FinalRecords, Plant, Run};
use provenance_store::repository::Repository;
use std::collections::BTreeMap;

const ONLY_L3: Config = Config {
    plant: None,
    only_law: Some("L3"),
    apply: None,
    layer_laws: None,
};

const fn with(apply: Apply) -> Config {
    Config {
        apply: Some(apply),
        ..ONLY_L3
    }
}

// No Rule: proof that L3 can fail. The storage drops the first set of an
// engine and answers that it landed.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_lost_write_breaks_l3() {
    let case = Case {
        engines: 2,
        steps: alone(&[Operation::UpdateRequirement { n: 0, stale: false }]),
    };
    let run = run_case(&case, planted(Plant::LoseWrite, "L3"));
    expect_law(laws::check_only(&run, "L3"), "L3");
}

// No Rule: proof that L3 sees a claim that an archive leaves. The storage
// drops the archive and answers that it landed, so the Topic keeps the claim.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn an_archive_that_leaves_the_claim_breaks_l3() {
    let mut steps = vec![Step::OutsideWrite(Operation::ClaimTopic)];
    steps.extend(alone(&[Operation::ArchiveRequirement { n: 0 }]));
    let run = run_case(&Case { engines: 2, steps }, planted(Plant::LoseWrite, "L3"));
    expect_law(laws::check_only(&run, "L3"), "L3");
}

// No Rule: proof that L3 sees a claim that is lost with no archive. The
// storage drops the claim of engine 0 and answers that it landed.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_lost_claim_breaks_l3() {
    let case = Case {
        engines: 2,
        steps: alone(&[Operation::ClaimTopic]),
    };
    let run = run_case(&case, planted(Plant::LoseWrite, "L3"));
    expect_law(laws::check_only(&run, "L3"), "L3");
}

// No Rule: proof that L3 keeps the check of a claim through an archive of
// unknown outcome. The storage drops the claim of engine 0 and answers that
// it landed. The archive lands no key, so the Topic stays open and must
// hold the claim.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_claim_lost_before_an_archive_of_unknown_outcome_breaks_l3() {
    let mut steps = alone(&[Operation::ClaimTopic]);
    steps.push(Step::Fault(Fault::UnknownOutcome { landed: 0 }));
    steps.extend(alone(&[Operation::ArchiveRequirement { n: 0 }]));
    let run = run_case(&Case { engines: 2, steps }, planted(Plant::LoseWrite, "L3"));
    assert!(met(&run, 1, Outcome::Unknown), "{:?}", run.publishes);
    expect_law(laws::check_only(&run, "L3"), "L3");
}

/// A stand-in archive of the first Requirement: it clears the claim of the
/// Topic and leaves the Requirement and the Topic open.
fn archive_that_only_clears_the_claim(
    repository: &Repository,
    task: &Task,
) -> anyhow::Result<Effect> {
    let Operation::ArchiveRequirement { n: 0 } = task.op else {
        return generate::apply(repository, task);
    };
    let store = repository.store();
    store.release_topic(&scope(), &stable(TOPIC), &task.actor())?;
    Ok(Effect::Archived {
        id: generate::FIRST_REQUIREMENT.to_owned(),
    })
}

// No Rule: proof that L3 sees a claim that is cleared on a Topic that stays
// open. A stand-in archive answers success, clears the claim and archives
// nothing.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_claim_cleared_on_an_open_topic_breaks_l3() {
    let case = Case {
        engines: 2,
        steps: alone(&[
            Operation::ClaimTopic,
            Operation::ArchiveRequirement { n: 0 },
        ]),
    };
    let run = run_case(&case, with(archive_that_only_clears_the_claim));
    expect_law(laws::check_only(&run, "L3"), "L3");
}

/// A stand-in claim that lands an empty stage and answers success, as a
/// claim that does not refuse an archived Topic.
fn claim_that_lands_an_empty_stage(repository: &Repository, task: &Task) -> anyhow::Result<Effect> {
    if task.op != Operation::ClaimTopic {
        return generate::apply(repository, task);
    }
    repository.stage(false, None, |_| Ok(()))?;
    Ok(Effect::Claim {
        holder: Some(task.actor()),
    })
}

// No Rule: proof that L3 sees a claim that answers success on an archived
// Topic. The Topic stays archived with no claim.
#[test]
#[ignore = "a planted fault of the nightly simulation"]
fn a_claim_on_an_archived_topic_breaks_l3() {
    let case = Case {
        engines: 2,
        steps: alone(&[
            Operation::ArchiveRequirement { n: 0 },
            Operation::ClaimTopic,
        ]),
    };
    let run = run_case(&case, with(claim_that_lands_an_empty_stage));
    assert!(met(&run, 1, Outcome::Landed), "{:?}", run.publishes);
    expect_law(laws::check_only(&run, "L3"), "L3");
}

/// The final records with the initial Requirements and an open Topic that
/// `claim` holds.
fn open_topic_held_by(claim: Option<&str>) -> FinalRecords {
    FinalRecords {
        descriptions: BTreeMap::from([(generate::FIRST_REQUIREMENT.to_owned(), None)]),
        topic_status: Some(TopicStatus::Open),
        claim: claim.map(str::to_owned),
        ..FinalRecords::default()
    }
}

// No Rule: proof that L3 refuses the earlier value of a record after a group
// with one certain write and one write that is not certain on the record,
// and accepts it when no write of the group is certain.
#[test]
#[ignore = "a check of the nightly simulation"]
fn a_certain_write_in_a_group_removes_the_earlier_value() {
    for (certain, earlier_holds) in [(true, false), (false, true)] {
        let write = |actor: &str, certain| Write {
            target: Target::Topic,
            value: Value::Topic {
                status: TopicStatus::Open,
                claim: Some(actor.to_owned()),
            },
            certain,
        };
        let mut expected = Expected::initial();
        expected.apply(1, [write("engine-0", certain), write("engine-1", false)]);
        for actor in ["engine-0", "engine-1"] {
            let state = open_topic_held_by(Some(actor));
            expected.check(&state, "group").unwrap();
        }
        let earlier = expected.check(&open_topic_held_by(None), "group");
        assert_eq!(earlier.is_ok(), earlier_holds, "{certain}: {earlier:?}");
    }
}

/// Tells if a publish of the operation at `index` of the run met `outcome`.
fn met(run: &Run, index: usize, outcome: Outcome) -> bool {
    let operation = run.done[index].operation;
    run.publishes
        .iter()
        .any(|publish| publish.operation == operation && publish.outcome == outcome)
}

/// The end of a stand-in write: a read, where a stop can meet the engine
/// after the publish of the write, then success or, with `FAIL`, a failure.
fn end<const FAIL: bool>(repository: &Repository, effect: Effect) -> anyhow::Result<Effect> {
    repository.view()?.list_topics(&scope())?;
    anyhow::ensure!(!FAIL, "a planted failure after the publish");
    Ok(effect)
}

/// A stand-in claim that gives the claim to a different actor.
fn claim_for_a_different_actor<const FAIL: bool>(
    repository: &Repository,
    task: &Task,
) -> anyhow::Result<Effect> {
    if task.op != Operation::ClaimTopic {
        return generate::apply(repository, task);
    }
    let store = repository.store();
    store.claim_topic(&scope(), &stable(TOPIC), "a-different-actor")?;
    let holder = Some(task.actor());
    end::<FAIL>(repository, Effect::Claim { holder })
}

/// A stand-in release that closes the Topic.
fn release_that_closes<const FAIL: bool>(
    repository: &Repository,
    task: &Task,
) -> anyhow::Result<Effect> {
    if task.op != Operation::ReleaseTopic {
        return generate::apply(repository, task);
    }
    repository.store().close_topic(&scope(), &stable(TOPIC))?;
    end::<FAIL>(repository, Effect::Claim { holder: None })
}

/// A stand-in archive of the first Requirement that claims the Topic for
/// its actor and archives nothing.
fn archive_that_claims<const FAIL: bool>(
    repository: &Repository,
    task: &Task,
) -> anyhow::Result<Effect> {
    let Operation::ArchiveRequirement { n: 0 } = task.op else {
        return generate::apply(repository, task);
    };
    let store = repository.store();
    store.claim_topic(&scope(), &stable(TOPIC), &task.actor())?;
    let id = generate::FIRST_REQUIREMENT.to_owned();
    end::<FAIL>(repository, Effect::Archived { id })
}

/// The writes before the planted write, and the planted write.
fn writes(planted: &Operation) -> (Vec<Operation>, Operation) {
    let before = if *planted == Operation::ReleaseTopic {
        vec![Operation::ClaimTopic]
    } else {
        Vec::new()
    };
    (before, planted.clone())
}

/// Runs `planted` with `apply` after the writes that it needs; the planted
/// write is certain.
fn certain(planted: &Operation, apply: Apply) -> Run {
    let (mut ops, planted) = writes(planted);
    ops.push(planted);
    let case = Case {
        engines: 2,
        steps: alone(&ops),
    };
    let run = run_case(&case, with(apply));
    let done = run.done.last().expect("the planted write ran");
    assert!(done.result.is_ok() && !done.stopped, "{:?}", run.done);
    run
}

/// Runs `planted` with `apply`; the publish of the planted write lands
/// and its outcome is unknown.
fn unknown(planted: &Operation, apply: Apply) -> Run {
    let (before, planted) = writes(planted);
    let mut steps = alone(&before);
    steps.push(Step::Fault(Fault::UnknownOutcome { landed: 3 }));
    steps.extend(alone(&[planted]));
    let run = run_case(&Case { engines: 2, steps }, with(apply));
    assert!(
        met(&run, before.len(), Outcome::Unknown),
        "{:?}",
        run.publishes
    );
    run
}

/// Runs `planted` with `apply`, which fails after the publish lands.
fn failed(planted: &Operation, apply: Apply) -> Run {
    let (mut ops, planted) = writes(planted);
    ops.push(planted);
    let case = Case {
        engines: 2,
        steps: alone(&ops),
    };
    let run = run_case(&case, with(apply));
    let index = ops.len() - 1;
    assert!(run.done[index].result.is_err(), "{:?}", run.done);
    assert!(met(&run, index, Outcome::Landed), "{:?}", run.publishes);
    run
}

/// The most turns of the planted write before its stop.
const MOST_TURNS: usize = 40;

/// Runs `planted` with `apply` and stops its engine after a number of
/// turns; returns the first run where the stop met the write after its
/// publish landed.
fn stopped(planted: &Operation, apply: Apply) -> Run {
    let (before, planted) = writes(planted);
    let index = before.len();
    for turns in 0..MOST_TURNS {
        let mut steps = alone(&before);
        steps.push(Step::Write {
            engine: 0,
            op: planted.clone(),
        });
        steps.extend(std::iter::repeat_n(Step::Turn(0), turns));
        steps.push(Step::Fault(Fault::Stop { engine: 0, at: 0 }));
        let run = run_case(&Case { engines: 2, steps }, with(apply));
        let stopped = run.done.get(index).is_some_and(|done| done.stopped);
        if stopped && met(&run, index, Outcome::Landed) {
            return run;
        }
    }
    panic!("in {MOST_TURNS} runs, no stop met {planted:?} after its publish landed");
}

/// One planted test of a Topic write: an outcome, the write and its stand-in.
/// The stand-in of a `failed` row fails after its publish.
macro_rules! topic_write_breaks_l3 {
    ($($test:ident: $outcome:ident, $op:expr, $apply:expr;)*) => {$(
        // No Rule: proof that L3 sees a wrong Topic value from this write
        // with this outcome.
        #[test]
        #[ignore = "a planted fault of the nightly simulation"]
        fn $test() {
            let run = $outcome(&$op, $apply);
            expect_law(laws::check_only(&run, "L3"), "L3");
        }
    )*};
}

topic_write_breaks_l3! {
    a_certain_claim_for_a_different_actor_breaks_l3:
        certain, Operation::ClaimTopic, claim_for_a_different_actor::<false>;
    a_claim_of_unknown_outcome_for_a_different_actor_breaks_l3:
        unknown, Operation::ClaimTopic, claim_for_a_different_actor::<false>;
    a_failed_claim_for_a_different_actor_breaks_l3:
        failed, Operation::ClaimTopic, claim_for_a_different_actor::<true>;
    a_stopped_claim_for_a_different_actor_breaks_l3:
        stopped, Operation::ClaimTopic, claim_for_a_different_actor::<false>;
    a_certain_release_that_closes_the_topic_breaks_l3:
        certain, Operation::ReleaseTopic, release_that_closes::<false>;
    a_release_of_unknown_outcome_that_closes_the_topic_breaks_l3:
        unknown, Operation::ReleaseTopic, release_that_closes::<false>;
    a_failed_release_that_closes_the_topic_breaks_l3:
        failed, Operation::ReleaseTopic, release_that_closes::<true>;
    a_stopped_release_that_closes_the_topic_breaks_l3:
        stopped, Operation::ReleaseTopic, release_that_closes::<false>;
    a_certain_archive_that_claims_the_topic_breaks_l3:
        certain, Operation::ArchiveRequirement { n: 0 }, archive_that_claims::<false>;
    an_archive_of_unknown_outcome_that_claims_the_topic_breaks_l3:
        unknown, Operation::ArchiveRequirement { n: 0 }, archive_that_claims::<false>;
    a_failed_archive_that_claims_the_topic_breaks_l3:
        failed, Operation::ArchiveRequirement { n: 0 }, archive_that_claims::<true>;
    a_stopped_archive_that_claims_the_topic_breaks_l3:
        stopped, Operation::ArchiveRequirement { n: 0 }, archive_that_claims::<false>;
}
