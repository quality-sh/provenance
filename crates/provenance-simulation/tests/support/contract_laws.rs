//! The laws of the contract layer:
//!
//! | Law | Statement |
//! | --- | --------- |
//! | O1 | No answer is a write failure (500) unless a planted fault met the operation. |
//! | O2 | The times of the messages that landed increase in the order that they landed. |
//! | O3 | A stage lands as one whole set on the revision where it started. |

use super::contract::refusal_prefix;
use provenance_simulation::generate::Effect;
use provenance_simulation::laws::Violation;
use provenance_simulation::storage::Outcome;
use provenance_simulation::{open_entries, scope, Done, Run};

const fn broken(law: &'static str, detail: String) -> Result<(), Violation> {
    Err(Violation { law, detail })
}

/// O1, O2 and O3.
pub fn contract_laws(run: &Run) -> Result<(), Violation> {
    no_write_failure_without_a_fault(run)?;
    message_times_increase(run)?;
    stages_land_whole(run)
}

fn publishes_of<'a>(
    run: &'a Run,
    done: &'a Done,
) -> impl Iterator<Item = &'a provenance_simulation::storage::Publish> {
    run.publishes
        .iter()
        .filter(move |publish| publish.operation == done.operation && publish.engine == done.engine)
}

/// O1: an answer of 500 comes only from an operation that a refused or
/// unknown publish met, or that stopped.
pub fn no_write_failure_without_a_fault(run: &Run) -> Result<(), Violation> {
    let prefix = refusal_prefix(500);
    for done in &run.done {
        let Err(text) = &done.result else { continue };
        if !text.starts_with(&prefix) || done.stopped {
            continue;
        }
        let faulted = publishes_of(run, done)
            .any(|publish| matches!(publish.outcome, Outcome::Refused | Outcome::Unknown));
        if !faulted {
            return broken(
                "O1",
                format!(
                    "operation {} ({:?}) answered a write failure without a fault: {text}",
                    done.operation, done.op
                ),
            );
        }
    }
    Ok(())
}

/// O2: the messages of the successful posts, in the order of their last
/// publish, have increasing times.
pub fn message_times_increase(run: &Run) -> Result<(), Violation> {
    let messages = open_entries(&run.entries)
        .and_then(|repository| repository.view()?.list_messages(&scope()))
        .map_err(|error| Violation {
            law: "O2",
            detail: format!("the messages of the final state did not read: {error:#}"),
        })?;
    let mut landed = Vec::new();
    for done in &run.done {
        let Ok(Effect::Message { id }) = &done.result else {
            continue;
        };
        let last_publish = run.publishes.iter().rposition(|publish| {
            publish.operation == done.operation
                && publish.engine == done.engine
                && publish.outcome == Outcome::Landed
        });
        let message = messages.iter().find(|message| message.id.as_str() == id);
        if let (Some(position), Some(message)) = (last_publish, message) {
            landed.push((position, done.operation, message.created_at));
        }
    }
    landed.sort_unstable();
    for pair in landed.windows(2) {
        let ((_, first, before), (_, second, after)) = (pair[0], pair[1]);
        if after <= before {
            return broken(
                "O2",
                format!(
                    "the message of operation {second} landed after the one of operation {first} with time {after}, not after {before}"
                ),
            );
        }
    }
    Ok(())
}

/// O3: each whole set names the revision where its stage started, and each
/// operation of an engine that always stages and succeeded landed one set on
/// that revision: a whole set, or a per-key set for a change of one key. A
/// write outside every engine lands as one change of the world, with no
/// base, so O3 does not count it.
pub fn stages_land_whole(run: &Run) -> Result<(), Violation> {
    if let Some(publish) = run
        .publishes
        .iter()
        .find(|publish| publish.whole_set && !publish.on_base)
    {
        return broken(
            "O3",
            format!(
                "the whole set of operation {} names no base revision",
                publish.operation
            ),
        );
    }
    for done in run
        .done
        .iter()
        .filter(|done| done.engine.is_some() && done.op.always_stages())
        .filter(|done| matches!(&done.result, Ok(effect) if *effect != Effect::Skipped))
    {
        let landed = publishes_of(run, done)
            .filter(|publish| publish.on_base && publish.outcome == Outcome::Landed)
            .count();
        if landed != 1 {
            return broken(
                "O3",
                format!(
                    "operation {} ({:?}) succeeded with {landed} sets on a base, not one",
                    done.operation, done.op
                ),
            );
        }
    }
    Ok(())
}
