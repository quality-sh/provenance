//! The laws that every run of a case keeps.
//!
//! | Law | Statement |
//! | --- | --------- |
//! | L0 | Each operation finishes in the wait limit, with no panic. |
//! | L1 | An operation that fails, with no unknown outcome, lands no write. |
//! | L2 | After an engine stops, the state opens and reads again. |
//! | L3 | A write that succeeded stays unless a later write changed it. |
//! | L4 | Of two writes on one etag, at most one succeeds. |
//! | L5 | After a moved base, the next write of the section lands. |
//! | L8 | One case gives the same final bytes each time. |

use crate::generate::{Effect, Target};
use crate::storage::{Outcome, Publish};
use crate::{Done, Run};
use std::collections::BTreeMap;

/// A law that a run broke.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Violation {
    pub law: &'static str,
    pub detail: String,
}

fn broken(law: &'static str, detail: impl Into<String>) -> Result<(), Violation> {
    Err(Violation {
        law,
        detail: detail.into(),
    })
}

/// L0: each operation finishes, with no panic.
fn each_operation_finishes(run: &Run) -> Result<(), Violation> {
    if !run.hung.is_empty() {
        return broken(
            "L0",
            format!(
                "engines {:?} did not finish in {:?}",
                run.hung,
                crate::storage::WAIT_LIMIT
            ),
        );
    }
    if let Some(done) = run
        .done
        .iter()
        .find(|done| matches!(&done.result, Err(text) if text.starts_with("panic:")))
    {
        return broken(
            "L0",
            format!("operation {} panicked: {:?}", done.operation, done.result),
        );
    }
    Ok(())
}

/// Checks L0 to L5 on one run.
pub fn check(run: &Run) -> Result<(), Violation> {
    each_operation_finishes(run)?;
    failed_operations_land_nothing(run)?;
    if let Some(failure) = run.reopen_failures.first() {
        return broken(
            "L2",
            format!("the state did not open after a stop: {failure}"),
        );
    }
    successful_writes_stay(run)?;
    one_winner_per_etag(run)?;
    the_write_after_a_moved_base_lands(run)
}

/// Checks one of L0 to L5 on one run; another law checks them all.
pub fn check_only(run: &Run, law: &str) -> Result<(), Violation> {
    match law {
        "L0" => each_operation_finishes(run),
        "L1" => failed_operations_land_nothing(run),
        "L2" => run.reopen_failures.first().map_or(Ok(()), |failure| {
            broken("L2", format!("the state did not open after a stop: {failure}"))
        }),
        "L3" => successful_writes_stay(run),
        "L4" => one_winner_per_etag(run),
        "L5" => the_write_after_a_moved_base_lands(run),
        _ => check(run),
    }
}

/// Checks L8: two runs of one case end with the same bytes.
pub fn check_same(first: &Run, second: &Run) -> Result<(), Violation> {
    if first.entries != second.entries {
        let keys: Vec<_> = first
            .entries
            .keys()
            .chain(second.entries.keys())
            .filter(|key| first.entries.get(*key) != second.entries.get(*key))
            .map(|key| key.as_str().to_owned())
            .collect();
        return broken("L8", format!("the final state differs at {keys:?}"));
    }
    Ok(())
}

fn publishes_of<'a>(run: &'a Run, done: &Done) -> impl Iterator<Item = (usize, &'a Publish)> {
    let (engine, operation) = (done.engine, done.operation);
    run.publishes
        .iter()
        .enumerate()
        .filter(move |(_, publish)| publish.engine == engine && publish.operation == operation)
}

/// L1: a refusal leaves the state as it was.
fn failed_operations_land_nothing(run: &Run) -> Result<(), Violation> {
    for done in run
        .done
        .iter()
        .filter(|done| done.result.is_err() && !done.stopped)
    {
        let publishes: Vec<_> = publishes_of(run, done).collect();
        if publishes.iter().any(|(_, p)| p.outcome == Outcome::Unknown) {
            continue;
        }
        if let Some((index, _)) = publishes.iter().find(|(_, p)| p.outcome == Outcome::Landed) {
            return broken(
                "L1",
                format!(
                    "operation {} failed ({}) but publish {index} landed",
                    done.operation,
                    done.result.as_ref().err().map_or("", String::as_str)
                ),
            );
        }
    }
    Ok(())
}

/// What the final state must hold for one record, if the record's last
/// write is certain.
#[derive(Debug)]
struct Expected {
    at: usize,
    value: Option<Effect>,
}

/// L3: the last write that landed on a record decides it.
fn successful_writes_stay(run: &Run) -> Result<(), Violation> {
    let mut expected: BTreeMap<Target, Expected> = BTreeMap::new();
    for done in &run.done {
        let Some(target) = &done.target else { continue };
        let publishes: Vec<_> = publishes_of(run, done).collect();
        let Some(&(at, last)) = publishes
            .iter()
            .rev()
            .find(|(_, p)| matches!(p.outcome, Outcome::Landed | Outcome::Unknown))
        else {
            continue;
        };
        let certain = last.outcome == Outcome::Landed && !done.stopped && done.result.is_ok();
        let value = certain.then(|| done.result.clone().ok()).flatten();
        if expected.get(target).is_none_or(|old| old.at < at) {
            expected.insert(target.clone(), Expected { at, value });
        }
    }
    let state = crate::final_records(&run.entries).map_err(|error| Violation {
        law: "L3",
        detail: format!("the final state does not read: {error:#}"),
    })?;
    for (target, expected) in expected {
        let Some(value) = expected.value else {
            continue;
        };
        let holds = match (&target, &value) {
            (Target::Exists(id), _) => state.descriptions.contains_key(id),
            (Target::Description(id), Effect::Described { description, .. }) => {
                state.descriptions.get(id) == Some(&Some(description.clone()))
            }
            (Target::Claim, Effect::Claim { holder }) => &state.claim == holder,
            _ => true,
        };
        if !holds {
            return broken(
                "L3",
                format!(
                    "after publish {}, {target:?} should hold {value:?}",
                    expected.at
                ),
            );
        }
    }
    for done in &run.done {
        if let Ok(Effect::Message { id }) = &done.result {
            if !state.messages.contains(id) {
                return broken(
                    "L3",
                    format!("message {id} of operation {} is gone", done.operation),
                );
            }
        }
    }
    Ok(())
}

/// L4: two updates on one etag do not both succeed.
fn one_winner_per_etag(run: &Run) -> Result<(), Violation> {
    let mut winners: BTreeMap<(&str, &str), u64> = BTreeMap::new();
    for done in &run.done {
        if let Ok(Effect::Described { id, etag, .. }) = &done.result {
            if let Some(first) = winners.insert((id.as_str(), etag.as_str()), done.operation) {
                return broken(
                    "L4",
                    format!(
                        "operations {first} and {} both updated {id} on etag {etag}",
                        done.operation
                    ),
                );
            }
        }
    }
    Ok(())
}

/// L5: a section that met a moved base reads the state again, so its next
/// write lands unless the state moved again.
fn the_write_after_a_moved_base_lands(run: &Run) -> Result<(), Violation> {
    for done in run.done.iter().filter(|done| done.engine.is_some()) {
        let publishes: Vec<_> = publishes_of(run, done).collect();
        for pair in publishes.windows(2) {
            let ((_, refused), (index, next)) = (pair[0], pair[1]);
            if refused.outcome == Outcome::BaseMoved
                && next.outcome == Outcome::BaseMoved
                && next.outside_writes == refused.outside_writes
            {
                return broken(
                    "L5",
                    format!(
                        "operation {} met a moved base again at publish {index} with no change in between",
                        done.operation
                    ),
                );
            }
        }
    }
    Ok(())
}
