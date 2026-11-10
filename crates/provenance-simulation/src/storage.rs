//! A simulated storage adapter that several engines share, one turn at a time.
//!
//! Each engine runs on its own thread. Each call that an engine makes to the
//! storage port (a read, a publish, entry to the section) is a turn: the
//! engine waits until the case gives it the turn. So one engine runs at a
//! time, and the case alone decides the order. The section lock is part of
//! the simulation, so an engine that waits for it never blocks its thread.

use provenance_store::storage::{
    Atomicity, PublishFailure, Revision, SectionLock, StateKey, StateStorage, StateView,
    StorageError, WriteSet,
};
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};

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
    /// Engines that wait for a turn.
    waiting: BTreeSet<usize>,
    /// Engines that run an operation.
    busy: BTreeSet<usize>,
    stopping: BTreeSet<usize>,
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
    /// Planted fault: after a moved base, an engine reads the state from
    /// before the outside write again.
    plant_stale_read: bool,
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
    pub fn new(entries: BTreeMap<StateKey, Vec<u8>>, plant_stale_read: bool) -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(State {
                entries,
                ..State::default()
            }),
            changed: Condvar::new(),
            plant_stale_read,
        })
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
        });
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

    /// Waits until engine `slot` waits for a turn or is idle.
    pub fn settle(&self, slot: usize) {
        let mut state = self.lock();
        while state.busy.contains(&slot) && !state.waiting.contains(&slot) {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        drop(state);
    }

    /// Gives engine `slot` one turn and waits until it settles again.
    pub fn grant(&self, slot: usize) {
        let mut state = self.lock();
        if !state.waiting.contains(&slot) {
            return;
        }
        state.waiting.remove(&slot);
        state.turn = Some(slot);
        drop(state);
        self.changed.notify_all();
        self.settle(slot);
    }

    /// Called by an engine before each storage call: waits for the turn.
    fn wait_for_turn(&self) -> Option<MutexGuard<'_, State>> {
        let (slot, _) = current_engine()?;
        let mut state = self.lock();
        if state.turn == Some(slot) {
            state.turn = None;
        }
        state.waiting.insert(slot);
        self.changed.notify_all();
        while state.turn != Some(slot) {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
        if state.stopping.remove(&slot) {
            drop(state);
            std::panic::resume_unwind(Box::new(Stopped));
        }
        Some(state)
    }

    fn read(&self) -> StateView {
        let Some(mut state) = self.wait_for_turn() else {
            return StateView::new(self.entries());
        };
        let slot = current_engine().map_or(usize::MAX, |(slot, _)| slot);
        let entries = match state.stale.get(&slot) {
            Some(old) if self.plant_stale_read => old.clone(),
            _ => state.entries.clone(),
        };
        state.last_read.insert(slot, entries.clone());
        StateView::new(entries)
    }

    fn publish(&self, writes: &WriteSet) -> Result<Revision, PublishFailure> {
        writes.ensure_disjoint()?;
        let mut state = self.wait_for_turn().unwrap_or_else(|| self.lock());
        let (engine, operation) = current_engine().map_or((None, 0), |(s, o)| (Some(s), o));
        let current = StateView::new(state.entries.clone());
        let (result, outcome) = if writes.base().is_some_and(|base| base != current.revision()) {
            if let Some(slot) = engine {
                if let Some(read) = state.last_read.get(&slot).cloned() {
                    state.stale.insert(slot, read);
                }
            }
            let failure = PublishFailure::BaseMoved {
                base: writes.base().cloned().expect("a base"),
                current: current.revision().clone(),
            };
            (Err(failure), Outcome::BaseMoved)
        } else {
            if let Some(slot) = engine {
                state.stale.remove(&slot);
            }
            match state.armed.take() {
                Some(Armed::Refuse) => (
                    Err(PublishFailure::Refused(anyhow::anyhow!(
                        "the simulated storage refused the set"
                    ))),
                    Outcome::Refused,
                ),
                Some(Armed::Unknown { landed }) => {
                    land_prefix(&mut state.entries, writes, landed);
                    (
                        Err(PublishFailure::Started(anyhow::anyhow!(
                            "the simulated storage lost the outcome"
                        ))),
                        Outcome::Unknown,
                    )
                }
                None => {
                    writes.apply_to(&mut state.entries);
                    let revision = StateView::new(state.entries.clone()).revision().clone();
                    (Ok(revision), Outcome::Landed)
                }
            }
        };
        let outside_writes = state.outside_writes;
        state.publishes.push(Publish {
            engine,
            operation,
            outcome,
            outside_writes,
        });
        result
    }

    /// Enters the section for this engine, a turn at a time.
    fn acquire(self: &Arc<Self>) -> Box<dyn Send> {
        let Some((slot, _)) = current_engine() else {
            return Box::new(());
        };
        loop {
            let mut state = self.wait_for_turn().expect("an engine thread");
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

/// Lands the first `landed` keys of `writes` in key order. A whole set lands
/// every key or none.
fn land_prefix(entries: &mut BTreeMap<StateKey, Vec<u8>>, writes: &WriteSet, landed: usize) {
    let mut keys: Vec<(&StateKey, Option<&Vec<u8>>)> = writes
        .deletes()
        .iter()
        .map(|key| (key, None))
        .chain(writes.puts().iter().map(|(key, bytes)| (key, Some(bytes))))
        .collect();
    keys.sort_by(|a, b| a.0.cmp(b.0));
    let count = match writes.atomicity() {
        Atomicity::WholeSet if landed > 0 => keys.len(),
        Atomicity::WholeSet => 0,
        Atomicity::PerKey => landed.min(keys.len()),
    };
    for (key, bytes) in keys.into_iter().take(count) {
        match bytes {
            Some(bytes) => entries.insert(key.clone(), bytes.clone()),
            None => entries.remove(key),
        };
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
        Ok(self.0.read())
    }

    fn publish(&self, writes: WriteSet) -> Result<Revision, PublishFailure> {
        self.0.publish(&writes)
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
