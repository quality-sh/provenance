//! Publication of review changes to graph record files.

use crate::{
    cache::ProjectionFamily,
    layout::ProvenanceLayout,
    state_store::{readers, StateStore},
};
use camino::{Utf8Path, Utf8PathBuf};
use provenance_macros::rule;
use std::collections::{BTreeMap, BTreeSet};

/// Refuses a review write if it changes a file outside the graph record files.
/// The record files include the Message month shards that already exist.
#[rule("rule_review_writes_change_only_record_files")]
pub(super) fn with_record_state<R>(
    live: &ProvenanceLayout,
    prepare: impl FnOnce(&ProvenanceLayout) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    crate::publication::with_staged_state(live, false, |staged| {
        let manifest = StateStore::new(live.clone()).manifest()?;
        let mut allowed = Vec::new();
        for scope in &manifest.scopes {
            allowed.extend(
                ProjectionFamily::ALL
                    .into_iter()
                    .map(|family| family.shard_path(live, &scope.id)),
            );
            // Messages can also be in the month shards that already exist.
            allowed.extend(readers::message_shard_paths(live, &scope.id)?);
        }
        let allowed = allowed
            .iter()
            .map(|path| path.strip_prefix(live.state_dir()).map(Utf8Path::to_owned))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let before = files(&live.state_dir())?;
        let result = prepare(staged)?;
        let after = files(&staged.state_dir())?;
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
