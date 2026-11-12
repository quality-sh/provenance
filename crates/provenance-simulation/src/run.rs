//! Runs the steps of a case with real engines over the simulated storage.

use crate::generate::{self, Case, Context, Fault, Step, Task};
use crate::identity::identity;
use crate::state::{initial_state, read_all};
use crate::storage::{self, Armed, SimulatedSectionLock, SimulatedStorage, Stopped, World};
use crate::{report, Apply, Config, Done, Run};
use provenance_store::repository::{Repository, RepositoryIdentity};
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use provenance_store::storage::StateStorage;
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Instant;

/// Opens a repository over `world`.
fn open(identity: &RepositoryIdentity, world: &Arc<World>) -> anyhow::Result<Repository> {
    Ok(Repository::with_storage(
        identity.clone(),
        Arc::new(SimulatedStorage(world.clone())),
        Arc::new(SimulatedSectionLock(world.clone())),
        None,
    )?)
}

/// The kind of the public answer to a failed write, as the contract names
/// it.
pub fn answer_kind(error: anyhow::Error) -> (String, String) {
    let error = provenance_store::write_error::WriteError(error);
    let kind = serde_json::to_value(error.safe())
        .ok()
        .and_then(|value| value["kind"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "no_kind".to_owned());
    (kind, format!("{:#}", error.0))
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
            let mut kind = None;
            let result = match result {
                Ok(Ok(effect)) => Ok(effect),
                Ok(Err(error)) => {
                    let (answer, text) = answer_kind(error);
                    kind = Some(answer);
                    Err(text)
                }
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
                    kind,
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

/// The engines of one run and what they did.
struct Runner {
    world: Arc<World>,
    identity: RepositoryIdentity,
    done: Arc<Mutex<Vec<Done>>>,
    apply: Apply,
    engines: Vec<Option<Engine>>,
    reopen_failures: Vec<String>,
    hung: Vec<usize>,
    serial: u64,
    context: Context,
}

impl Runner {
    fn new(case: &Case, config: Config) -> Self {
        let setup = initial_state();
        let mut runner = Self {
            world: World::new(setup.entries, config.plant),
            identity: identity(config.plant),
            done: Arc::new(Mutex::new(Vec::new())),
            apply: config.apply.unwrap_or(generate::apply),
            engines: Vec::new(),
            reopen_failures: Vec::new(),
            hung: Vec::new(),
            serial: 0,
            context: Context::new(setup.first_etag),
        };
        for slot in 0..usize::from(case.engines.max(1)) {
            let repository =
                open(&runner.identity, &runner.world).expect("the initial state opens");
            let engine = runner.spawn(slot, repository);
            runner.engines.push(Some(engine));
        }
        runner
    }

    fn spawn(&self, slot: usize, repository: Repository) -> Engine {
        spawn_engine(
            slot,
            repository,
            self.world.clone(),
            self.done.clone(),
            self.apply,
        )
    }

    /// The task of the next operation.
    fn task(&mut self, engine: Option<usize>, op: generate::Operation) -> Task {
        self.serial += 1;
        self.context.learn(effects(&self.done));
        self.context.task(self.serial, engine, op)
    }

    fn step(&mut self, step: Step) {
        let count = self.engines.len();
        match step {
            Step::Write { engine, op } => {
                let slot = usize::from(engine) % count;
                if self.world.is_busy(slot) {
                    self.world.grant(slot);
                } else if self.engines[slot].is_some() {
                    let task = self.task(Some(slot), op);
                    self.world.start(slot);
                    let engine = self.engines[slot].as_ref().expect("an engine");
                    engine.tasks.send(task).expect("the engine thread runs");
                    self.world.settle(slot);
                }
            }
            Step::WriteOnMovedBase {
                engine,
                op,
                outside,
            } => {
                let slot = usize::from(engine) % count;
                self.step(Step::Write { engine, op });
                for _ in 0..MAX_DRAIN_TURNS {
                    if !self.world.is_busy(slot) || self.world.waits_to_publish(slot) {
                        break;
                    }
                    self.world.grant(slot);
                }
                self.step(Step::OutsideWrite(outside));
            }
            Step::Turn(engine) => self.world.grant(usize::from(engine) % count),
            Step::Fault(Fault::Refuse) => self.world.arm(Armed::Refuse),
            Step::Fault(Fault::UnknownOutcome { landed }) => self.world.arm(Armed::Unknown {
                landed: usize::from(landed % 4),
            }),
            Step::Fault(Fault::Stop { engine, .. }) => self.stop(usize::from(engine) % count),
            Step::OutsideWrite(op) => {
                let task = self.task(None, op);
                let outcome = write_outside(&self.identity, &self.world, &task);
                self.done
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .push(outcome);
            }
        }
    }

    /// Stops engine `slot` and opens a new engine in its place.
    fn stop(&mut self, slot: usize) {
        self.world.note_stop();
        if self.world.is_busy(slot) {
            self.world.stop(slot);
            self.world.grant(slot);
        }
        if let Some(old) = self.engines[slot].take() {
            if self.world.is_busy(slot) {
                // The engine waits outside a storage call; keep it.
                self.engines[slot] = Some(old);
                return;
            }
            drop(old.tasks);
            if !join(old.thread) {
                self.hung.push(slot);
            }
        }
        match open(&self.identity, &self.world).and_then(|repository| {
            read_all(&repository)?;
            Ok(repository)
        }) {
            Ok(repository) => self.engines[slot] = Some(self.spawn(slot, repository)),
            Err(error) => self.reopen_failures.push(format!("{error:#}")),
        }
    }

    /// Lets each busy engine end, then ends each engine thread.
    fn finish(mut self) -> Run {
        let busy = drain(&self.world);
        for slot in &busy {
            self.world.stop(*slot);
            self.world.grant(*slot);
        }
        self.hung.extend(busy);
        for (slot, engine) in std::mem::take(&mut self.engines).into_iter().enumerate() {
            if let Some(engine) = engine {
                drop(engine.tasks);
                if !join(engine.thread) {
                    self.hung.push(slot);
                }
            }
        }
        self.hung.extend(self.world.hung());
        self.hung.sort_unstable();
        self.hung.dedup();
        let done = self
            .done
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        Run {
            done,
            publishes: self.world.publishes(),
            reopen_failures: self.reopen_failures,
            hung: self.hung,
            entries: self.world.entries(),
        }
    }
}

/// Runs `case` and returns what it did.
pub fn run_case(case: &Case, config: Config) -> Run {
    let mut runner = Runner::new(case, config);
    for step in &case.steps {
        runner.step(step.clone());
    }
    runner.finish()
}

/// Gives the busy engines turns until each one is idle; returns those that
/// are still busy.
fn drain(world: &World) -> Vec<usize> {
    for _ in 0..MAX_DRAIN_TURNS {
        let busy: Vec<_> = world
            .busy()
            .into_iter()
            .filter(|slot| !world.hung().contains(slot))
            .collect();
        if busy.is_empty() {
            return world.busy();
        }
        for slot in busy {
            world.grant(slot);
        }
    }
    world.busy()
}

/// Waits for `thread` to end, for the wait limit at most. Returns false when
/// the thread did not end; it then runs on, detached.
fn join(thread: JoinHandle<()>) -> bool {
    let deadline = Instant::now() + storage::WAIT_LIMIT;
    while !thread.is_finished() {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    if let Err(payload) = thread.join() {
        std::panic::resume_unwind(payload);
    }
    true
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
        None,
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
    let (result, kind) = match result {
        Ok(effect) => (Ok(effect), None),
        Err(error) => {
            let (kind, text) = answer_kind(error);
            (Err(text), Some(kind))
        }
    };
    Done {
        engine: None,
        operation: task.operation,
        target: task.target(),
        op: task.op.clone(),
        result,
        kind,
        stopped: false,
    }
}

/// The effects of the operations that succeeded so far.
fn effects(done: &Mutex<Vec<Done>>) -> Vec<generate::Effect> {
    done.lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .filter_map(|done| done.result.clone().ok())
        .collect()
}
