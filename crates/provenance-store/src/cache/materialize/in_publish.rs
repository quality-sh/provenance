//! The catch-up of an embedder that keeps its read database up to date
//! itself, after each of its publishes.

use super::catch_up::{catch_up_with_guard, CatchUpReport};
use crate::publication::PublicationGuard;
use crate::repository::Repository;

/// Catches the read database of `repository` up with its canonical state,
/// for an embedder that keeps the read database up to date itself.
///
/// The embedder calls it after each of its publishes returns, while it
/// still keeps the other publishes of the repository out in its own way:
/// its next publish starts only after this call ends. The call runs one
/// catch-up pass in one write of the read database, and builds the
/// projection when the database holds none. It takes no publication guard
/// and never waits. A call with no publish since the last pass commits
/// nothing.
///
/// One pass at a time writes the read database of one repository and its
/// clones. When another pass runs, this call refuses at once with a
/// [`CatchUpRunning`] error and writes nothing. The other pass can be a call
/// of this function or, with `host`, the catch-up of a read,
/// `catch_up_state` or a rebuild of `materialize`. With `host`, a read
/// whose own catch-up starts while this pass runs answers as a read whose
/// catch-up failed: at the stored serial, stamped `catch_up_failed`, with
/// the refusal as its freshness error.
///
/// When the pass fails, its write of the read database rolls back. The
/// canonical state keeps the publish, and the read database stays at the
/// serial of the last pass that succeeded. Without `host` each read then
/// answers at that serial, stamped `annotate_only` with no freshness error,
/// and a `refuse_stale` read refuses `stale`. With `host` the next
/// `catch_up` read catches the database up itself. The next call of this
/// function that succeeds catches it up in both builds.
///
/// [`CatchUpRunning`]: crate::cache::CatchUpRunning
pub async fn catch_up_in_publish(repository: &Repository) -> anyhow::Result<CatchUpReport> {
    let guard = PublicationGuard::held_by_caller(repository)?;
    catch_up_with_guard(guard, repository.read_database()?).await
}
