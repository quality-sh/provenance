//! Simulation of the write engine: several engines over one simulated storage.
//!
//! A [`generate::Case`] gives the steps. [`run_case`] runs them with real
//! engines over [`storage::SimulatedStorage`], one storage call at a time, and
//! returns a [`Run`]. [`laws`] checks a run; [`report`] prints a failure so
//! that a later run can replay it.

pub mod generate;
pub mod laws;
pub mod report;
pub mod storage;

use generate::{Case, Context, Effect, Fault, Operation, Step, Target, Task};
use provenance_store::repository::{Clock, IdSource, Repository, RepositoryIdentity};
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::{StateKey, StateStorage};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use storage::{Armed, SimulatedSectionLock, SimulatedStorage, Stopped, World};

/// Runs one operation of an engine; a layer above the engine gives its own.
pub type Apply = fn(&Repository, &Task) -> anyhow::Result<Effect>;

/// The laws that one layer checks in addition to [`laws::check`].
pub type LayerLaws = fn(&Run) -> Result<(), laws::Violation>;

/// How a case runs.
#[derive(Debug, Clone, Copy, Default)]
pub struct Config {
    /// Planted fault: after a moved base, an engine reads the old state again.
    pub plant_stale_read: bool,
    /// Checks this one law only, as a test of a planted fault does.
    pub only_law: Option<&'static str>,
    /// How an engine runs an operation; `None` calls the store directly.
    pub apply: Option<Apply>,
    /// The laws of the layer, checked after the laws of every layer.
    pub layer_laws: Option<LayerLaws>,
}

/// What one operation gave.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Done {
    /// The engine slot, or `None` for a write outside every engine.
    pub engine: Option<usize>,
    pub operation: u64,
    pub op: Operation,
    pub target: Option<Target>,
    pub result: Result<Effect, String>,
    pub stopped: bool,
}

/// Everything that one case did.
#[derive(Debug, Clone)]
pub struct Run {
    pub done: Vec<Done>,
    pub publishes: Vec<storage::Publish>,
    /// Failures to open and read the state after an engine stopped.
    pub reopen_failures: Vec<String>,
    /// Engines that were still busy when the case gave up waiting.
    pub hung: Vec<usize>,
    pub entries: BTreeMap<StateKey, Vec<u8>>,
}

/// A clock that moves one second at each read.
#[derive(Debug)]
struct CountingClock(AtomicI64);

impl Clock for CountingClock {
    fn now(&self) -> time::OffsetDateTime {
        let seconds = self.0.fetch_add(1, Ordering::SeqCst);
        time::OffsetDateTime::from_unix_timestamp(seconds).expect("a valid time")
    }
}

/// IDs that count up.
#[derive(Debug)]
struct CountingIds(AtomicU64);

impl IdSource for CountingIds {
    fn new_uuid(&self) -> uuid::Uuid {
        uuid::Uuid::from_u128(u128::from(self.0.fetch_add(1, Ordering::SeqCst) + 1))
    }
}

/// The identity that every engine of one run shares: one clock, one ID source.
fn identity() -> RepositoryIdentity {
    identity_with_ids(0)
}

/// The first ID of the initial state. The IDs of a run start at 1, so they
/// never meet an ID that the initial state holds.
const INITIAL_STATE_IDS: u64 = 1 << 32;

/// An identity whose IDs count up after `start`.
fn identity_with_ids(start: u64) -> RepositoryIdentity {
    RepositoryIdentity::new(
        camino::Utf8PathBuf::from("/simulated"),
        provenance_core::ScopeId::new(generate::SCOPE).expect("a scope"),
        provenance_store::settings::Settings::default(),
    )
    .with_clock(Arc::new(CountingClock(AtomicI64::new(1_000_000_000))))
    .with_ids(Arc::new(CountingIds(AtomicU64::new(start))))
}

/// Opens a repository over `world`.
fn open(identity: &RepositoryIdentity, world: &Arc<World>) -> anyhow::Result<Repository> {
    Ok(Repository::with_storage(
        identity.clone(),
        Arc::new(SimulatedStorage(world.clone())),
        Arc::new(SimulatedSectionLock(world.clone())),
    )?)
}

struct Engine {
    tasks: Sender<Task>,
    thread: JoinHandle<()>,
}

fn spawn_engine(
    slot: usize,
    repository: Repository,
    world: Arc<World>,
    done: Arc<Mutex<Vec<Done>>>,
    apply: Apply,
) -> Engine {
    let (tasks, inbox) = channel::<Task>();
    let thread = std::thread::spawn(move || {
        for task in inbox {
            storage::set_engine(Some((slot, task.operation)));
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                apply(&repository, &task)
            }));
            storage::set_engine(None);
            let stopped = matches!(&result, Err(payload) if payload.is::<Stopped>());
            let result = match result {
                Ok(result) => result.map_err(|error| format!("{error:#}")),
                Err(payload) if stopped => {
                    drop(payload);
                    Err("stopped".to_owned())
                }
                Err(payload) => Err(format!("panic: {}", report::panic_text(&*payload))),
            };
            done.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(Done {
                    engine: Some(slot),
                    operation: task.operation,
                    target: task.target(),
                    op: task.op,
                    result,
                    stopped,
                });
            world.finish(slot);
            if stopped {
                return;
            }
        }
    });
    Engine { tasks, thread }
}

/// The most turns that the end of a case gives before it gives up.
const MAX_DRAIN_TURNS: usize = 10_000;

/// Runs `case` and returns what it did.
pub fn run_case(case: &Case, config: Config) -> Run {
    let setup = initial_state();
    let world = World::new(setup.entries.clone(), config.plant_stale_read);
    let identity = identity();
    let done = Arc::new(Mutex::new(Vec::new()));
    let count = usize::from(case.engines.max(1));
    let apply = config.apply.unwrap_or(generate::apply);
    let mut reopen_failures = Vec::new();
    let mut engines: Vec<Option<Engine>> = (0..count)
        .map(|slot| {
            let repository = open(&identity, &world).expect("the initial state opens");
            Some(spawn_engine(
                slot,
                repository,
                world.clone(),
                done.clone(),
                apply,
            ))
        })
        .collect();
    let mut serial = 0_u64;
    let mut context = Context::new(setup.first_etag);
    for step in &case.steps {
        match step {
            Step::Write { engine, op } => {
                let slot = usize::from(*engine) % count;
                if world.is_busy(slot) {
                    world.grant(slot);
                } else if let Some(engine) = &engines[slot] {
                    serial += 1;
                    context.learn(effects(&done));
                    let task = context.task(serial, Some(slot), op.clone());
                    world.start(slot);
                    engine.tasks.send(task).expect("the engine thread runs");
                    world.settle(slot);
                }
            }
            Step::Turn(engine) => world.grant(usize::from(*engine) % count),
            Step::Fault(Fault::Refuse) => world.arm(Armed::Refuse),
            Step::Fault(Fault::UnknownOutcome { landed }) => world.arm(Armed::Unknown {
                landed: usize::from(*landed % 4),
            }),
            Step::Fault(Fault::Stop { engine }) => {
                let slot = usize::from(*engine) % count;
                if world.is_busy(slot) {
                    world.stop(slot);
                    world.grant(slot);
                }
                if let Some(old) = engines[slot].take() {
                    if world.is_busy(slot) {
                        // The engine waits outside a storage call; keep it.
                        engines[slot] = Some(old);
                        continue;
                    }
                    drop(old.tasks);
                    join(old.thread);
                }
                match open(&identity, &world).and_then(|repository| {
                    read_all(&repository)?;
                    Ok(repository)
                }) {
                    Ok(repository) => {
                        engines[slot] = Some(spawn_engine(
                            slot,
                            repository,
                            world.clone(),
                            done.clone(),
                            apply,
                        ));
                    }
                    Err(error) => reopen_failures.push(format!("{error:#}")),
                }
            }
            Step::OutsideWrite(op) => {
                serial += 1;
                context.learn(effects(&done));
                let task = context.task(serial, None, op.clone());
                let outcome = write_outside(&identity, &world, &task);
                done.lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(outcome);
            }
        }
    }
    let hung = drain(&world);
    for slot in &hung {
        world.stop(*slot);
        world.grant(*slot);
    }
    for engine in engines.into_iter().flatten() {
        drop(engine.tasks);
        join(engine.thread);
    }
    let done = done.lock().unwrap_or_else(PoisonError::into_inner).clone();
    Run {
        done,
        publishes: world.publishes(),
        reopen_failures,
        hung,
        entries: world.entries(),
    }
}

/// Gives the busy engines turns until each one is idle; returns those that
/// are still busy.
fn drain(world: &World) -> Vec<usize> {
    for _ in 0..MAX_DRAIN_TURNS {
        let busy = world.busy();
        if busy.is_empty() {
            return busy;
        }
        for slot in busy {
            world.grant(slot);
        }
    }
    world.busy()
}

fn join(thread: JoinHandle<()>) {
    if let Err(payload) = thread.join() {
        std::panic::resume_unwind(payload);
    }
}

/// Runs `task` on its own repository over a copy of the state and lands the
/// change in the world outside every engine.
fn write_outside(identity: &RepositoryIdentity, world: &Arc<World>, task: &Task) -> Done {
    let before = world.entries();
    let storage = Arc::new(MemoryStateStorage::new(before.clone()));
    let result = Repository::with_storage(
        identity.clone(),
        storage.clone(),
        Arc::new(MemorySectionLock::default()),
    )
    .map_err(anyhow::Error::from)
    .and_then(|repository| generate::apply(&repository, task));
    if result.is_ok() {
        let after = storage
            .read_view()
            .map(provenance_store::storage::StateView::into_entries)
            .unwrap_or_default();
        if after != before {
            world.write_outside(task.operation, &before, &after);
        }
    }
    Done {
        engine: None,
        operation: task.operation,
        target: task.target(),
        op: task.op.clone(),
        result: result.map_err(|error| format!("{error:#}")),
        stopped: false,
    }
}

/// The effects of the operations that succeeded so far.
fn effects(done: &Mutex<Vec<Done>>) -> Vec<Effect> {
    done.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter_map(|done| done.result.clone().ok())
        .collect()
}

pub(crate) fn scope() -> provenance_core::ScopeId {
    provenance_core::ScopeId::new(generate::SCOPE).expect("a scope")
}

pub(crate) fn stable(id: &str) -> provenance_core::StableId {
    provenance_core::StableId::new(id).expect("a stable ID")
}

/// The state that every case starts from and the etag of its Requirement.
struct InitialState {
    entries: BTreeMap<StateKey, Vec<u8>>,
    first_etag: String,
}

/// The manifest, one Requirement, one Rule and one Topic.
fn initial_state() -> InitialState {
    use provenance_core::{Manifest, RepoPathPrefix};
    let manifest = Manifest::default_with_scope(scope(), RepoPathPrefix::new("."));
    let key = |value: &str| StateKey::new(value).expect("a state key");
    let shard = |row: serde_json::Value| {
        let mut bytes = serde_json::to_vec(&row).expect("JSON");
        bytes.push(b'\n');
        bytes
    };
    let requirement = serde_json::json!({
        "scope_id": generate::SCOPE, "id": generate::FIRST_REQUIREMENT,
        "statement": "Share links expire", "status": "active",
    });
    let rule = serde_json::json!({
        "scope_id": generate::SCOPE, "id": generate::FIRST_RULE,
        "statement": "Share links expire", "status": "active", "severity": "medium",
        "requirement_ids": [generate::FIRST_REQUIREMENT],
    });
    let entries = BTreeMap::from([
        (
            key("manifest.json"),
            serde_json::to_vec(&manifest).expect("JSON"),
        ),
        (
            key("scopes/default/requirements/req.jsonl"),
            shard(requirement),
        ),
        (key("scopes/default/rules/rule.jsonl"), shard(rule)),
    ]);
    let storage = Arc::new(MemoryStateStorage::new(entries));
    let repository = Repository::with_storage(
        identity_with_ids(INITIAL_STATE_IDS),
        storage.clone(),
        Arc::new(MemorySectionLock::default()),
    )
    .expect("the initial state opens");
    let store = repository.store();
    store
        .create_topic(provenance_store::state_store::CreateTopicInput {
            scope_id: scope(),
            id: stable(generate::TOPIC),
            requirement_id: stable(generate::FIRST_REQUIREMENT),
            title: "Who owns a share link".into(),
            status: provenance_core::TopicStatus::Open,
            links: Vec::new(),
        })
        .expect("the Topic is written");
    let first_etag = store
        .requirement_edit_state(&scope(), &stable(generate::FIRST_REQUIREMENT))
        .expect("the Requirement has an etag")
        .etag;
    InitialState {
        entries: storage.read_view().expect("a view").into_entries(),
        first_etag,
    }
}

/// Reads every record kind that the operations write.
fn read_all(repository: &Repository) -> anyhow::Result<()> {
    let view = repository.view()?;
    view.list_requirements(&scope())?;
    view.list_topics(&scope())?;
    view.list_messages(&scope())?;
    view.list_verification_bindings(&scope())?;
    Ok(())
}

/// The records of the final state that the laws follow.
#[derive(Debug, Default)]
pub struct FinalRecords {
    /// Each Requirement with its description.
    pub descriptions: BTreeMap<String, Option<String>>,
    /// The holder of the Topic claim.
    pub claim: Option<String>,
    pub messages: std::collections::BTreeSet<String>,
}

/// Reads the records that the laws follow from `entries`.
pub fn final_records(entries: &BTreeMap<StateKey, Vec<u8>>) -> anyhow::Result<FinalRecords> {
    let repository = Repository::with_storage(
        identity(),
        Arc::new(MemoryStateStorage::new(entries.clone())),
        Arc::new(MemorySectionLock::default()),
    )?;
    let view = repository.view()?;
    Ok(FinalRecords {
        descriptions: view
            .list_requirements(&scope())?
            .into_iter()
            .map(|requirement| (requirement.id.as_str().to_owned(), requirement.description))
            .collect(),
        claim: view
            .list_topics(&scope())?
            .into_iter()
            .find(|topic| topic.id.as_str() == generate::TOPIC)
            .and_then(|topic| topic.claimed_by),
        messages: view
            .list_messages(&scope())?
            .into_iter()
            .map(|message| message.id.as_str().to_owned())
            .collect(),
    })
}

/// Runs `case` twice and checks every law; a broken law fails the test case
/// with a report that names the law and replays the case.
pub fn check_case(layer: &str, case: &Case, config: Config) {
    let first = run_case(case, config);
    let verdict = config.only_law.map_or_else(
        || {
            laws::check(&first)
                .and_then(|()| {
                    config
                        .layer_laws
                        .map_or(Ok(()), |layer_laws| layer_laws(&first))
                })
                .and_then(|()| laws::check_same(&first, &run_case(case, config)))
        },
        |law| laws::check_only(&first, law),
    );
    if let Err(violation) = verdict {
        report::fail(layer, case, &violation);
    }
}
