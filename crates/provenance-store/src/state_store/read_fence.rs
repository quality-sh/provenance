//! The one check of every state read: the read is recorded for the tests,
//! and it is refused when it is outside the read fence of this thread.
//!
//! A catch-up parses a family again only when a file in one of its input
//! directories changed. The family reader runs inside [`with_read_fence`] of
//! those directories. Each read of a store on this thread passes through
//! [`check`], also a copy of a shared parse. A read outside the directories
//! fails the parse, also when the reader drops the error. So a reader cannot
//! read a file that the catch-up does not watch for its family.

use camino::{Utf8Path, Utf8PathBuf};
use std::cell::RefCell;

struct Fence {
    scope_dir: Utf8PathBuf,
    directories: Vec<&'static str>,
    outside: Option<Utf8PathBuf>,
}

impl Fence {
    /// Whether `path` is below one of the directories of the fence. The
    /// parts of a shard path have `/` or the native separator between them.
    fn allows(&self, path: &Utf8Path) -> bool {
        let Ok(below) = path.strip_prefix(&self.scope_dir) else {
            return false;
        };
        let directory = below.as_str().split(['/', '\\']).next().unwrap_or_default();
        self.directories.contains(&directory)
    }
}

thread_local! {
    static FENCE: RefCell<Option<Fence>> = const { RefCell::new(None) };
}

/// Runs `body` with the state reads of this thread fenced to `directories`
/// below `scope_dir`. A read outside them fails the result. The fence ends
/// with the body, also when it panics.
pub fn with_read_fence<R>(
    scope_dir: Utf8PathBuf,
    directories: Vec<&'static str>,
    body: impl FnOnce() -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    struct Restore(Option<Fence>);
    impl Drop for Restore {
        fn drop(&mut self) {
            let previous = self.0.take();
            FENCE.with(|fence| *fence.borrow_mut() = previous);
        }
    }
    let fence = Fence {
        scope_dir,
        directories,
        outside: None,
    };
    let previous = FENCE.with(|current| current.borrow_mut().replace(fence));
    let restore = Restore(previous);
    let result = body();
    let outside = FENCE.with(|fence| {
        fence
            .borrow_mut()
            .as_mut()
            .and_then(|fence| fence.outside.take())
    });
    drop(restore);
    outside.map_or(result, |path| Err(outside_error(&path)))
}

/// Records one read of the state file or directory at `path`. Inside a
/// read fence, a path outside its directories is refused.
pub(super) fn check(path: &Utf8Path) -> anyhow::Result<()> {
    crate::test_probes::record_read(path);
    FENCE.with(|fence| {
        let mut fence = fence.borrow_mut();
        match fence.as_mut() {
            Some(fence) if !fence.allows(path) => {
                fence.outside.get_or_insert_with(|| path.to_owned());
                Err(outside_error(path))
            }
            _ => Ok(()),
        }
    })
}

fn outside_error(path: &Utf8Path) -> anyhow::Error {
    anyhow::anyhow!("a family reader read {path}, which is outside its input directories")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Implementation aid: the fence refuses a read outside its directories,
    /// also when the reader drops the error. No Rule binds it.
    #[test]
    fn a_read_outside_the_fence_fails_the_body() {
        let scope = Utf8PathBuf::from("/repo/.provenance/state/scopes/default");
        let inside = scope.join("ideation/landings.jsonl");
        let outside = scope.join("rules/rule.jsonl");
        let read = with_read_fence(scope.clone(), vec!["ideation"], || {
            check(&inside)?;
            check(&scope.join("ideation"))?;
            let _dropped = check(&outside);
            Ok(())
        });
        assert_eq!(
            read.unwrap_err().to_string(),
            outside_error(&outside).to_string()
        );
        assert!(check(&outside).is_ok(), "the fence ends with the body");
        assert!(with_read_fence(scope.clone(), vec![], || check(&scope)).is_err());
    }
}
