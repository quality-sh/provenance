//! A simulated storage adapter that several engines share, one turn at a time.
//!
//! Each engine runs on its own thread. Each call that an engine makes to the
//! storage port (a read, a publish, entry to the section) is a turn: the
//! engine waits until the case gives it the turn. So one engine runs at a
//! time, and the case alone decides the order. The section lock is part of
//! the simulation, so an engine that waits for it never blocks its thread.
//!
//! Each wait has a limit. When the case waits [`WAIT_LIMIT`] for an engine,
//! the engine is hung; when an engine waits that long for a turn, it panics.
//! So a deadlock fails its case in minutes, with the case, and does not hold
//! the job.

mod publish;

use crate::plant::Plant;
use provenance_store::storage::{
    PublishFailure, SectionLock, StateKey, StateStorage, StateView, StorageError, WriteSet,
};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// The longest wait for one engine or for one turn.
pub const WAIT_LIMIT: Duration = Duration::from_secs(60);

/// The unwind payload of an engine that a fault stops.
#[derive(Debug)]
pub struct Stopped;

/// What one publish gave.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Outcome {
    Landed,
    Refused,
    BaseMoved,
    Unknown,
}

/// One publish, in the order the storage took them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Publish {
    /// The engine slot, or `None` for a write outside every engine.
    pub engine: Option<usize>,
    /// The operation of the engine that made the publish.
    pub operation: u64,
    pub outcome: Outcome,
    /// How many outside writes landed before this publish.
    pub outside_writes: usize,
    /// The set lands as one whole set: only a stage gives one.
    pub whole_set: bool,
    /// The set names the revision that it was made on.
    pub on_base: bool,
}

/// The storage call that an engine waits to make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Read,
    Publish,
    Enter,
}

/// A fault that the next publish meets.
#[derive(Debug, Clone, Copy)]
pub enum Armed {
    Refuse,
    Unknown { landed: usize },
}

#[derive(Debug, Default)]
struct State {
    entries: BTreeMap<StateKey, Vec<u8>>,
    holder: Option<usize>,
    armed: Option<Armed>,
    /// The engine that has the turn.
    turn: Option<usize>,
    /// Engines that wait for a turn, with the call that each one waits to
    /// make.
    waiting: BTreeMap<usize, Call>,
    /// Engines that run an operation.
    busy: BTreeSet<usize>,
    stopping: BTreeSet<usize>,
    /// Engines that did not settle in the wait limit.
    hung: BTreeSet<usize>,
    /// An engine stopped in this case.
    stopped: bool,
    /// The planted fault met its first chance.
    planted: bool,
    /// A key whose landing fails, as the storage contract suite asks.
    broken: Option<StateKey>,
    publishes: Vec<Publish>,
    outside_writes: usize,
    /// The entries that an engine read before its last moved base (planted
    /// fault).
    stale: BTreeMap<usize, BTreeMap<StateKey, Vec<u8>>>,
    /// The entries that each engine read last.
    last_read: BTreeMap<usize, BTreeMap<StateKey, Vec<u8>>>,
}

/// The state that every engine of a case shares.
#[derive(Debug)]
pub struct World {
    state: Mutex<State>,
    changed: Condvar,
    plant: Option<Plant>,
}

thread_local! {
    static ENGINE: Cell<Option<(usize, u64)>> = const { Cell::new(None) };
}

/// Marks this thread as engine `slot` running `operation`, or as no engine.
pub fn set_engine(engine: Option<(usize, u64)>) {
    ENGINE.with(|cell| cell.set(engine));
}

fn current_engine() -> Option<(usize, u64)> {
    ENGINE.with(Cell::get)
}

impl World {
    pub fn new(entries: BTreeMap<StateKey, Vec<u8>>, plant: Option<Plant>) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                entries,
                ..State::default()
            }),
            changed: Condvar::new(),
            plant,
        })
    }

    /// The planted faults that turn off the section lock and the base check.
    const fn no_concurrency_control(&self) -> bool {
        matches!(self.plant, Some(Plant::NoConcurrencyControl))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The current entries.
    pub fn entries(&self) -> BTreeMap<StateKey, Vec<u8>> {
        self.lock().entries.clone()
    }

    /// Every publish so far.
    pub fn publishes(&self) -> Vec<Publish> {
        self.lock().publishes.clone()
    }

    pub fn arm(&self, fault: Armed) {
        self.lock().armed = Some(fault);
    }

    /// Lands the change from `before` to `after` outside every engine.
    pub fn write_outside(
        &self,
        operation: u64,
        before: &BTreeMap<StateKey, Vec<u8>>,
        after: &BTreeMap<StateKey, Vec<u8>>,
    ) {
        let mut state = self.lock();
        for key in before.keys().filter(|key| !after.contains_key(*key)) {
            state.entries.remove(key);
        }
        for (key, bytes) in after {
            if before.get(key) != Some(bytes) {
                state.entries.insert(key.clone(), bytes.clone());
            }
        }
        state.outside_writes += 1;
        let outside_writes = state.outside_writes;
        state.publishes.push(Publish {
            engine: None,
            operation,
            outcome: Outcome::Landed,
            outside_writes,
            whole_set: false,
            on_base: false,
        });
    }

    /// Makes each later landing of `key` fail: a per-key set lands the keys
    /// before it, a whole set lands no key, and the outcome is unknown.
    pub fn break_key(&self, key: StateKey) {
        self.lock().broken = Some(key);
    }

    /// Notes that an engine of the case stopped.
    pub fn note_stop(&self) {
        self.lock().stopped = true;
    }

    /// Tells if engine `slot` waits to publish.
    pub fn waits_to_publish(&self, slot: usize) -> bool {
        self.lock().waiting.get(&slot) == Some(&Call::Publish)
    }

    /// The engines that did not settle in the wait limit.
    pub fn hung(&self) -> Vec<usize> {
        self.lock().hung.iter().copied().collect()
    }

    /// Marks engine `slot` busy before it gets an operation.
    pub fn start(&self, slot: usize) {
        self.lock().busy.insert(slot);
    }

    /// Marks engine `slot` idle after its operation.
    pub fn finish(&self, slot: usize) {
        let mut state = self.lock();
        state.busy.remove(&slot);
        if state.turn == Some(slot) {
            state.turn = None;
        }
        drop(state);
        self.changed.notify_all();
    }

    pub fn is_busy(&self, slot: usize) -> bool {
        self.lock().busy.contains(&slot)
    }

    /// The busy engines.
    pub fn busy(&self) -> Vec<usize> {
        self.lock().busy.iter().copied().collect()
    }

    /// Makes engine `slot` unwind at its next turn.
    pub fn stop(&self, slot: usize) {
        self.lock().stopping.insert(slot);
    }

    /// Waits until engine `slot` waits for a turn or is idle. An engine that
    /// does not settle in the wait limit is hung, and the case gives it no
    /// more turns.
    pub fn settle(&self, slot: usize) {
        let deadline = Instant::now() + WAIT_LIMIT;
        let mut state = self.lock();
        while state.busy.contains(&slot) && !state.waiting.contains_key(&slot) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                state.hung.insert(slot);
                break;
            }
            state = self
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        drop(state);
    }

    /// Gives engine `slot` one turn and waits until it settles again.
    pub fn grant(&self, slot: usize) {
        let mut state = self.lock();
        if !state.waiting.contains_key(&slot) || state.hung.contains(&slot) {
            return;
        }
        state.waiting.remove(&slot);
        state.turn = Some(slot);
        drop(state);
        self.changed.notify_all();
        self.settle(slot);
    }

    /// Called by an engine before each storage call: waits for the turn.
    fn wait_for_turn(&self, call: Call) -> Option<MutexGuard<'_, State>> {
        let (slot, _) = current_engine()?;
        let mut state = self.lock();
        if state.turn == Some(slot) {
            state.turn = None;
        }
        state.waiting.insert(slot, call);
        self.changed.notify_all();
        let deadline = Instant::now() + WAIT_LIMIT;
        while state.turn != Some(slot) {
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                state.waiting.remove(&slot);
                drop(state);
                panic!("engine {slot} waited {WAIT_LIMIT:?} for a turn");
            }
            state = self
                .changed
                .wait_timeout(state, left)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
        if state.stopping.remove(&slot) {
            drop(state);
            std::panic::resume_unwind(Box::new(Stopped));
        }
        Some(state)
    }

    /// Enters the section for this engine, a turn at a time.
    fn acquire(self: &Arc<Self>) -> Box<dyn Send> {
        let Some((slot, _)) = current_engine() else {
            return Box::new(());
        };
        loop {
            let mut state = self.wait_for_turn(Call::Enter).expect("an engine thread");
            if self.no_concurrency_control() {
                return Box::new(());
            }
            if state.holder.is_none() {
                state.holder = Some(slot);
                return Box::new(Held {
                    world: self.clone(),
                    slot,
                });
            }
            drop(state);
        }
    }
}

/// The section, held until it drops.
struct Held {
    world: Arc<World>,
    slot: usize,
}

impl Drop for Held {
    fn drop(&mut self) {
        let mut state = self.world.lock();
        if state.holder == Some(self.slot) {
            state.holder = None;
        }
    }
}

/// The storage port of one case.
#[derive(Debug, Clone)]
pub struct SimulatedStorage(pub Arc<World>);

impl StateStorage for SimulatedStorage {
    fn read_view(&self) -> Result<StateView, StorageError> {
        self.0.read()
    }

    fn publish(&self, writes: WriteSet) -> Result<(), PublishFailure> {
        self.0.publish(&writes)?;
        Ok(())
    }
}

/// The section lock of one case.
#[derive(Debug, Clone)]
pub struct SimulatedSectionLock(pub Arc<World>);

impl SectionLock for SimulatedSectionLock {
    fn acquire(&self) -> anyhow::Result<Box<dyn Send>> {
        Ok(self.0.acquire())
    }
}
