//! Parses that the readers of one view share.
//!
//! In one catch-up pass the graph validator, the ideation validator, the
//! relation rows and the Discussion check each list the six graph kinds.
//! The stores of one scope of the pass share one [`SharedParses`]: the
//! first list of a shard parses it, and each later list of the same shard
//! gets a copy of that parse.
//!
//! A [`SharedParses`] holds the view that its parses read, and only a store
//! over that view shares it (see [`super::ViewStore::sharing`]). No write
//! changes a view. So a copy is the parse of the bytes that the store that
//! asks for it reads, and a store over another view or over the files never
//! gets it.
//!
//! A copy is a read of its shard through the store that asks for it, so it
//! passes through the read fence of a fenced store like a parse does.

use crate::storage::StateView;
use camino::{Utf8Path, Utf8PathBuf};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

type Parses = HashMap<(Utf8PathBuf, TypeId), Box<dyn Any + Send>>;

/// One view and the parses of its shards that the stores over it share.
pub struct SharedParses {
    view: Arc<StateView>,
    parses: Mutex<Parses>,
}

// Only catch-up, which the host build has, shares parses.
#[cfg_attr(not(feature = "host"), allow(dead_code))]
impl SharedParses {
    /// No parses yet of the shards of `view`.
    pub(crate) fn new(view: Arc<StateView>) -> Arc<Self> {
        Arc::new(Self {
            view,
            parses: Mutex::new(HashMap::new()),
        })
    }

    /// The view that each parse read.
    pub(super) const fn view(&self) -> &Arc<StateView> {
        &self.view
    }

    fn parses(&self) -> std::sync::MutexGuard<'_, Parses> {
        // Each change inserts one whole value, so a poisoned map is intact.
        self.parses.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl std::fmt::Debug for SharedParses {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SharedParses")
            .field("keys", &self.view.entries().len())
            .finish_non_exhaustive()
    }
}

/// The records of the shard at `path`: a copy of a shared parse, or the
/// result of `parse`. A store that shares parses shares a parse that
/// succeeds.
pub(super) fn shared<T: Clone + Send + 'static>(
    store: &super::StateStore,
    path: &Utf8Path,
    parse: impl FnOnce() -> anyhow::Result<Vec<T>>,
) -> anyhow::Result<Vec<T>> {
    let Some(shared) = store.shared_parses() else {
        return parse();
    };
    let key = (path.to_path_buf(), TypeId::of::<Vec<T>>());
    let hit = shared
        .parses()
        .get(&key)
        .and_then(|parsed| parsed.downcast_ref::<Vec<T>>())
        .cloned();
    if let Some(records) = hit {
        store.check_read(path)?;
        return Ok(records);
    }
    let records = parse()?;
    shared.parses().insert(key, Box::new(records.clone()));
    Ok(records)
}

/// [`super::readers::read_jsonl`] through [`shared`].
pub(super) fn read_jsonl_shared<T: serde::de::DeserializeOwned + Clone + Send + 'static>(
    store: &super::StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    shared(store, path, || super::readers::read_jsonl(store, path))
}
