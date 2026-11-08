//! Parses that the readers of one catch-up pass share.
//!
//! In one pass the graph validator, the ideation validator, the relation
//! rows and the Discussion check each list the six graph kinds. Inside
//! [`with_shared_parses`], the first list of a shard parses it, and each
//! later list of the same shard on this thread gets a copy of that parse.
//! The caller holds the publication guard for the whole body, and checks
//! after the body that the bytes did not change, so a copy is the parse of
//! the current bytes.

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
/// result of `parse`, which is shared when it succeeds inside
/// [`with_shared_parses`].
pub(super) fn shared<T: Clone + 'static>(
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
