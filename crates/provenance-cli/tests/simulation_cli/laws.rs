//! The laws that a run of a case through the CLI keeps.

use crate::process::stop::Stop;
use provenance_core::{RequirementStatus, ScopeId, StableId};
use provenance_simulation::expected::{Expected, Write};
use provenance_simulation::generate::{
    created_requirement_id, requirement_id, Case, Effect, Operation, Step, Task, SCOPE,
};
use provenance_simulation::laws::Violation;
use provenance_simulation::{final_records, run_case, Config, Done};
use provenance_store::repository::Repository;
use provenance_store::storage::StateKey;
use std::collections::BTreeMap;

/// How the writes of one group ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ran {
    Alone,
    AtOnce,
    Stopped,
    Outside,
}

/// Everything that one case did on disk.
pub struct CliRun {
    pub groups: Vec<(Ran, Vec<Done>)>,
    /// What each stop of the case did.
    pub stops: Vec<Stop>,
    /// The final state, or the error of its read.
    pub entries: Result<BTreeMap<StateKey, Vec<u8>>, String>,
    /// The first failure of the full-rebuild comparison of the reads.
    pub reads: Result<(), String>,
    /// The processes that did not end in the process limit or that
    /// panicked.
    pub l0_breaks: Vec<String>,
}

const fn broken(law: &'static str, detail: String) -> Result<(), Violation> {
    Err(Violation { law, detail })
}

/// Checks L0, L3, L4, L6 and L7.
pub fn check(case: &Case, run: &CliRun) -> Result<(), Violation> {
    if let Some(process) = run.l0_breaks.first() {
        return broken("L0", process.clone());
    }
    successful_writes_stay(run)?;
    one_winner_per_etag(run)?;
    if let Err(detail) = &run.reads {
        return broken("L6", detail.clone());
    }
    one_writer_answers_as_the_engine(case, run, apply_public)
}

/// L3: each record holds a value of the last group that changed it, from
/// the values of the initial state. A write that stopped, or a write merged
/// from a branch, is not certain. A failed write that did not stop changes
/// no record.
fn successful_writes_stay(run: &CliRun) -> Result<(), Violation> {
    let mut expected = Expected::initial();
    for (index, (way, done)) in run.groups.iter().enumerate() {
        let landed = done
            .iter()
            .filter(|done| done.stopped || done.result.is_ok());
        let group = landed.flat_map(|done| {
            let certain = !done.stopped && *way != Ran::Outside;
            let writes = done.op.writes(done.engine, done.operation);
            writes.into_iter().map(move |(target, value)| Write {
                target,
                value,
                certain,
            })
        });
        expected.apply(index + 1, group);
    }
    let entries = run.entries.as_ref().map_err(|error| Violation {
        law: "L3",
        detail: format!("the storage adapter did not read the final state: {error}"),
    })?;
    let state = final_records(entries).map_err(|error| Violation {
        law: "L3",
        detail: format!("the final state does not read: {error:#}"),
    })?;
    expected.check(&state, "write group")
}

/// L4: two writes of the engines on one etag do not both succeed.
fn one_winner_per_etag(run: &CliRun) -> Result<(), Violation> {
    let mut winners = BTreeMap::new();
    for done in run.groups.iter().flat_map(|(_, done)| done) {
        let Ok(Effect::Described { id, etag, .. }) = &done.result else {
            continue;
        };
        if done.engine.is_none() {
            continue;
        }
        if let Some(first) = winners.insert((id, etag), done.operation) {
            let second = done.operation;
            let detail = format!("operations {first} and {second} both updated {id} on {etag}");
            return broken("L4", detail);
        }
    }
    Ok(())
}

/// The engine runs a write as the public API does: a create and an update
/// open a review submission.
pub fn apply_public(repository: &Repository, task: &Task) -> anyhow::Result<Effect> {
    use provenance_store::review::{CreateReviewRequirement, SaveRequirement};
    use provenance_store::state_store::{CreateRequirementInput, UpdateRequirementInput};
    let (scope, store) = (ScopeId::new(SCOPE)?, repository.store());
    match &task.op {
        Operation::CreateRequirement { n } => {
            let id = created_requirement_id(*n);
            let create = CreateRequirementInput {
                scope_id: scope,
                id: StableId::new(&id)?,
                statement: "Share links have an owner".into(),
                description: None,
                status: RequirementStatus::Active,
                refines: Vec::new(),
                depends_on: Vec::new(),
                supersedes: Vec::new(),
                spawned_by: None,
                origin_thread: None,
                origin_message: None,
            };
            let actor = task.actor();
            let origin = None;
            store.create_review_requirement(CreateReviewRequirement {
                actor,
                create,
                origin,
            })?;
            Ok(Effect::Created { id })
        }
        Operation::UpdateRequirement { n, stale } => {
            let id = requirement_id(*n);
            let etag = if *stale {
                task.stale_etag.clone()
            } else {
                let stable = StableId::new(&id)?;
                store.requirement_edit_state(&scope, &stable)?.etag
            };
            let update = UpdateRequirementInput {
                scope_id: scope,
                id: StableId::new(&id)?,
                expected_etag: None,
                declared_by: None,
                statement: None,
                description: Some(task.description()),
                status: None,
                archived_in_commit: None,
                clear_fields: Vec::new(),
            };
            store.save_requirement(SaveRequirement {
                actor: task.actor(),
                expected_etag: etag.clone(),
                update,
                relationships: None,
            })?;
            let description = task.description();
            Ok(Effect::Described {
                id,
                description,
                etag,
            })
        }
        _ => provenance_simulation::generate::apply(repository, task),
    }
}

/// The answer kind of `done`: success, or the kind of its refusal.
fn answer(done: &Done) -> Result<(), Option<&str>> {
    done.result
        .as_ref()
        .map(|_| ())
        .map_err(|_| done.kind.as_deref())
}

/// L7: the writes of one writer, before the first write at once, stop or
/// merge, get the answer kind that the engine gives the same writes. A
/// write that the simulation did not send, because the read of its etag
/// refused, has no CLI answer to compare.
pub fn one_writer_answers_as_the_engine(
    case: &Case,
    run: &CliRun,
    apply: provenance_simulation::Apply,
) -> Result<(), Violation> {
    let alone = run.groups.iter().take_while(|(way, _)| *way == Ran::Alone);
    let alone: Vec<&Done> = alone.flat_map(|(_, done)| done).collect();
    let mut steps = Vec::new();
    for done in &alone {
        let engine = u8::try_from(done.engine.unwrap_or(0)).unwrap_or(0);
        let op = done.op.clone();
        steps.push(Step::Write { engine, op });
        steps.extend(std::iter::repeat_n(Step::Turn(engine), 400));
    }
    let config = Config {
        apply: Some(apply),
        ..Config::default()
    };
    let engines = case.engines;
    let engine = run_case(&Case { engines, steps }, config);
    for (index, cli) in alone.iter().enumerate() {
        let Some(engine) = engine.done.get(index) else {
            let detail = format!("the engine did not answer operation {}", cli.operation);
            return broken("L7", detail);
        };
        if cli.kind.as_deref() == Some(crate::process::NOT_SENT) {
            continue;
        }
        if answer(cli) != answer(engine) {
            let (operation, op) = (cli.operation, &cli.op);
            let kinds = (answer(cli), answer(engine));
            let (cli, engine) = (&cli.result, &engine.result);
            let detail = format!(
                "operation {operation} ({op:?}): answer kinds {kinds:?}; CLI {cli:?}, engine {engine:?}"
            );
            return broken("L7", detail);
        }
    }
    Ok(())
}
