//! Publication of review changes to graph record files.

use crate::{
    cache::ProjectionFamily,
    state_store::{readers, RecordStamp, StateStore},
};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use std::collections::{BTreeMap, BTreeSet};

/// Refuses a review write if it changes a file outside the graph record files.
/// The record files include the Message month shards that already exist.
///
/// `prepare` borrows a staged store with the identity of `live`. The live
/// manifest and files are read before it runs; while it runs, live access
/// refuses.
#[rule("rule_review_writes_change_only_record_files")]
pub(super) fn with_record_state<R>(
    live: &StateStore,
    stamp: RecordStamp,
    prepare: impl FnOnce(&StateStore) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    let layout = &live.layout;
    crate::publication::with_staged_state(layout, false, |staged_layout| {
        let manifest = live.manifest()?;
        let mut allowed = Vec::new();
        for scope in &manifest.scopes {
            allowed.extend(
                ProjectionFamily::ALL
                    .into_iter()
                    .map(|family| family.shard_path(layout, &scope.id)),
            );
            // Messages can also be in the month shards that already exist.
            allowed.extend(readers::message_shard_paths(layout, &scope.id)?);
        }
        let allowed = allowed
            .iter()
            .map(|path| {
                path.strip_prefix(layout.state_dir())
                    .map(Utf8Path::to_owned)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let before = files(&layout.state_dir())?;
        let staged =
            StateStore::staged_view(live.identity_for_stage(), staged_layout.clone(), stamp);
        let result = crate::publication::with_stage_body(layout, || prepare(&staged))?;
        let after = files(&staged_layout.state_dir())?;
        for path in before.keys().chain(after.keys()) {
            anyhow::ensure!(
                before.get(path) == after.get(path) || allowed.contains(path),
                "review write changes a non-record file: {path}"
            );
        }
        Ok(result)
    })
}

fn files(root: &Utf8Path) -> anyhow::Result<BTreeMap<Utf8PathBuf, Vec<u8>>> {
    fn visit(
        root: &Utf8Path,
        directory: &Utf8Path,
        result: &mut BTreeMap<Utf8PathBuf, Vec<u8>>,
    ) -> anyhow::Result<()> {
        for entry in std::fs::read_dir(directory)? {
            let path = Utf8PathBuf::from_path_buf(entry?.path())
                .map_err(|_| anyhow::anyhow!("state path is not UTF-8"))?;
            if path.is_dir() {
                visit(root, &path, result)?;
            } else {
                result.insert(path.strip_prefix(root)?.to_owned(), std::fs::read(path)?);
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result)?;
    Ok(result)
}
