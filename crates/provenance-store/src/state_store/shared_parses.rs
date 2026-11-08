//! Parses that the readers of one catch-up pass share.
//!
//! In one pass the graph validator, the ideation validator, the relation
//! rows and the Discussion check each list the six graph kinds. Inside
//! [`with_shared_parses`], the first list of a shard parses it. Each later
//! list of the same shard on this thread gets a copy of that parse.
//! A copy is the parse of the bytes that the body reads. A catch-up pass
//! and the live-update watcher each read one view of the state, which no
//! write changes. So every store of the body reads the same bytes.
//!
//! A copy is a read of its shard through the store that asks for it, so it
//! passes through the read fence of a fenced store like a parse does.

use camino::{Utf8Path, Utf8PathBuf};
use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::HashMap;

type Parses = HashMap<(Utf8PathBuf, TypeId), Box<dyn Any>>;

thread_local! {
    static PARSES: RefCell<Option<Parses>> = const { RefCell::new(None) };
}

/// Runs `body` with parses shared on this thread. The shared parses end
/// with the body, also when it panics.
pub fn with_shared_parses<R>(body: impl FnOnce() -> R) -> R {
    struct Restore(Option<Parses>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            PARSES.with(|parses| *parses.borrow_mut() = previous);
        }
    }
    let previous = PARSES.with(|parses| parses.borrow_mut().replace(HashMap::new()));
    let _restore = Restore(previous);
    body()
}

/// The records of the shard at `path`: a copy of a shared parse, or the
/// result of `parse`. Inside [`with_shared_parses`], a parse that succeeds
/// is shared.
pub(super) fn shared<T: Clone + 'static>(
    store: &super::StateStore,
    path: &Utf8Path,
    parse: impl FnOnce() -> anyhow::Result<Vec<T>>,
) -> anyhow::Result<Vec<T>> {
    let key = (path.to_path_buf(), TypeId::of::<Vec<T>>());
    let hit = PARSES.with(|parses| {
        parses
            .borrow()
            .as_ref()
            .and_then(|parses| parses.get(&key))
            .and_then(|parsed| parsed.downcast_ref::<Vec<T>>())
            .cloned()
    });
    if let Some(records) = hit {
        store.check_read(path)?;
        return Ok(records);
    }
    let records = parse()?;
    PARSES.with(|parses| {
        if let Some(parses) = parses.borrow_mut().as_mut() {
            parses.insert(key, Box::new(records.clone()));
        }
    });
    Ok(records)
}

/// [`super::readers::read_jsonl`] through [`shared`].
pub(super) fn read_jsonl_shared<T: serde::de::DeserializeOwned + Clone + 'static>(
    store: &super::StateStore,
    path: &Utf8Path,
) -> anyhow::Result<Vec<T>> {
    shared(store, path, || super::readers::read_jsonl(store, path))
}
