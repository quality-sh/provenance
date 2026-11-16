//! The read and the publish of the simulated storage, with the faults that
//! the case arms and the faults that a test plants.

use super::{current_engine, Armed, Call, Outcome, Publish, World};
use crate::plant::Plant;
use provenance_store::storage::{
    Atomicity, PublishFailure, Revision, StateKey, StateView, StorageError, WriteSet,
};
use std::collections::{BTreeMap, BTreeSet};

impl World {
    pub(super) fn read(&self) -> Result<StateView, StorageError> {
        let failed = || {
            Err(StorageError::Read(anyhow::anyhow!(
                "the simulated storage failed a read (planted)"
            )))
        };
        let Some(mut state) = self.wait_for_turn(Call::Read) else {
            let state = self.lock();
            if self.plant == Some(Plant::FailReopen) && state.stopped {
                return failed();
            }
            return Ok(StateView::new(state.entries.clone()));
        };
        if self.plant == Some(Plant::FailRead) && !state.planted {
            state.planted = true;
            return failed();
        }
        let slot = current_engine().map_or(usize::MAX, |(slot, _)| slot);
        let entries = match state.stale.get(&slot) {
            Some(old) if self.plant == Some(Plant::StaleRead) => old.clone(),
            _ => state.entries.clone(),
        };
        state.last_read.insert(slot, entries.clone());
        Ok(StateView::new(entries))
    }

    pub(super) fn publish(&self, writes: &WriteSet) -> Result<Revision, PublishFailure> {
        writes.ensure_disjoint()?;
        let mut state = self
            .wait_for_turn(Call::Publish)
            .unwrap_or_else(|| self.lock());
        let (engine, operation) = current_engine().map_or((None, 0), |(s, o)| (Some(s), o));
        let atomicity = if self.plant == Some(Plant::LandWholeSetPerKey) && engine.is_some() {
            Atomicity::PerKey
        } else {
            writes.atomicity()
        };
        let current = StateView::new(state.entries.clone());
        let checks_base = !self.no_concurrency_control();
        let moved = writes.base().is_some_and(|base| base != current.revision());
        let (result, outcome) = if checks_base && moved {
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
                Some(Armed::Refuse) => {
                    let refused = Err(PublishFailure::Refused(anyhow::anyhow!(
                        "the simulated storage refused the set"
                    )));
                    if self.plant == Some(Plant::LandRefused) {
                        // An armed fault gives its own outcome, so a broken
                        // key changes only the keys that land.
                        let state = &mut *state;
                        let broken = &state.broken;
                        let _ = land(&mut state.entries, broken, writes, atomicity, usize::MAX);
                        (refused, Outcome::Landed)
                    } else {
                        (refused, Outcome::Refused)
                    }
                }
                Some(Armed::Unknown { landed }) => {
                    let count = match atomicity {
                        Atomicity::WholeSet if landed > 0 => usize::MAX,
                        Atomicity::WholeSet => 0,
                        Atomicity::PerKey => landed,
                    };
                    // An armed fault gives its own outcome, so a broken key
                    // changes only the keys that land.
                    let state = &mut *state;
                    let _ = land(&mut state.entries, &state.broken, writes, atomicity, count);
                    (
                        Err(PublishFailure::Started(anyhow::anyhow!(
                            "the simulated storage lost the outcome"
                        ))),
                        Outcome::Unknown,
                    )
                }
                None => {
                    let mut entries = state.entries.clone();
                    match land(&mut entries, &state.broken, writes, atomicity, usize::MAX) {
                        Err(Broken { landed, key }) => {
                            let error = anyhow::anyhow!(
                                "the simulated storage failed to land {}",
                                key.as_str()
                            );
                            if atomicity == Atomicity::PerKey && landed == 0 {
                                // No key changed, so the set is refused.
                                (Err(PublishFailure::Refused(error)), Outcome::Refused)
                            } else {
                                state.entries = entries;
                                (Err(PublishFailure::Started(error)), Outcome::Unknown)
                            }
                        }
                        Ok(()) => {
                            let lose = self.plant == Some(Plant::LoseWrite)
                                && engine.is_some()
                                && !std::mem::replace(&mut state.planted, true);
                            if !lose {
                                state.entries = entries;
                            }
                            if self.plant == Some(Plant::BackdateMessage) {
                                backdate_newest_message(&mut state.entries, writes);
                            }
                            let revision = StateView::new(state.entries.clone()).revision().clone();
                            (Ok(revision), Outcome::Landed)
                        }
                    }
                }
            }
        };
        let outside_writes = state.outside_writes;
        state.publishes.push(Publish {
            engine,
            operation,
            outcome,
            outside_writes,
            whole_set: atomicity == Atomicity::WholeSet,
            keys: writes.key_count(),
            on_base: writes.base().is_some(),
        });
        result
    }
}

/// Planted fault: sets the time of the newest message in each message shard
/// that `writes` puts to zero.
fn backdate_newest_message(entries: &mut BTreeMap<StateKey, Vec<u8>>, writes: &WriteSet) {
    let shards = writes.puts().keys().filter(|key| {
        key.as_str().contains("/threads/") && !key.as_str().ends_with("/threads.jsonl")
    });
    for key in shards {
        let Some(bytes) = entries.get_mut(key) else {
            continue;
        };
        let mut rows: Vec<serde_json::Value> = String::from_utf8_lossy(bytes)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        let newest = rows
            .iter_mut()
            .filter(|row| row.get("body").is_some())
            .max_by_key(|row| row["created_at"].as_i64());
        if let Some(row) = newest {
            row["created_at"] = 0.into();
        }
        let mut backdated = Vec::new();
        for row in rows {
            backdated.extend(serde_json::to_vec(&row).expect("JSON"));
            backdated.push(b'\n');
        }
        *bytes = backdated;
    }
}

/// The broken key that stopped a landing, after `landed` keys of the set.
struct Broken {
    landed: usize,
    key: StateKey,
}

/// Writes the first `count` keys of `writes`, in key order, into `entries`,
/// with `atomicity`. Each write of a key in `broken` fails, whichever set
/// makes it. A per-key set keeps the keys before the broken key. A whole set
/// lands on a copy, so it keeps no key when one fails.
fn land(
    entries: &mut BTreeMap<StateKey, Vec<u8>>,
    broken: &BTreeSet<StateKey>,
    writes: &WriteSet,
    atomicity: Atomicity,
    count: usize,
) -> Result<(), Broken> {
    let mut keys: Vec<(&StateKey, Option<&Vec<u8>>)> = writes
        .deletes()
        .iter()
        .map(|key| (key, None))
        .chain(writes.puts().iter().map(|(key, bytes)| (key, Some(bytes))))
        .collect();
    keys.sort_by(|a, b| a.0.cmp(b.0));
    let mut landed = entries.clone();
    for (done, (key, bytes)) in keys.into_iter().take(count).enumerate() {
        if broken.contains(key) {
            if atomicity == Atomicity::PerKey {
                *entries = landed;
            }
            let key = key.clone();
            return Err(Broken { landed: done, key });
        }
        match bytes {
            Some(bytes) => landed.insert(key.clone(), bytes.clone()),
            None => landed.remove(key),
        };
    }
    *entries = landed;
    Ok(())
}
