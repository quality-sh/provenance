//! Publication of review changes to graph record files.

use crate::{
    cache::ProjectionFamily,
    state_store::{readers, RecordStamp, StateStore},
};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use std::collections::BTreeSet;

/// Refuses a review write if it changes a file outside the graph record files.
/// The record files include the Message month shards that already exist.
///
/// `prepare` borrows a staged store with the identity of `live`. The section
/// is taken through `live`, and the live manifest and record files are read
/// through the held store before it runs; while it runs, live access refuses.
/// The check compares the view where the stage started with its overlay.
#[rule("rule_review_writes_change_only_record_files")]
pub(super) fn with_record_state<R>(
    live: &StateStore,
    stamp: RecordStamp,
    prepare: impl FnOnce(&StateStore) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    live.with_repository_publication(|held| {
        let allowed = record_files(held)?;
        crate::publication::with_staged_state(held, false, stamp, |staged| {
            let result = prepare(staged)?;
            let changes = staged.stage_changes()?;
            for key in changes.puts().keys().chain(changes.deletes()) {
                anyhow::ensure!(
                    allowed.contains(Utf8Path::new(key.as_str())),
                    "review write changes a non-record file: {}",
                    key.as_str()
                );
            }
            Ok(result)
        })
    })
}

/// The record files of every scope of the manifest, below the state
/// directory.
fn record_files(held: &StateStore) -> anyhow::Result<BTreeSet<Utf8PathBuf>> {
    let layout = &held.layout;
    let manifest = held.manifest()?;
    let mut allowed = Vec::new();
    for scope in &manifest.scopes {
        allowed.extend(
            ProjectionFamily::ALL
                .into_iter()
                .map(|family| family.shard_path(layout, &scope.id)),
        );
        // Messages can also be in the month shards that already exist.
        allowed.extend(readers::message_shard_paths_of(held, &scope.id)?);
    }
    Ok(allowed
        .iter()
        .map(|path| {
            path.strip_prefix(layout.state_dir())
                .map(Utf8Path::to_owned)
        })
        .collect::<Result<BTreeSet<_>, _>>()?)
}
