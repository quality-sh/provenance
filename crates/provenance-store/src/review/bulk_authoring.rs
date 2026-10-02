//! Guarded lifecycle updates that change more than one record in a family.

use super::guard;
use crate::publication::with_staged_state;
use crate::state_store::record_stamps::GraphRecord;
use crate::state_store::StateStore;
use camino::Utf8Path;
use provenance_core::review::ReviewRecord;

impl StateStore {
    pub(crate) fn mutate_native_records<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        let relative = path.strip_prefix(self.layout.root())?.to_owned();
        let stamp = self.current_record_stamp()?;
        self.with_repository_publication(|| {
            with_staged_state(&self.layout, false, |layout| {
                let staged = Self::staged(layout.clone(), stamp);
                let staged_path = layout.root().join(&relative);
                guard::with_writer(&staged_path, "*", || {
                    let changes =
                        staged.mutate_jsonl_records(&staged_path, |records: &mut Vec<T>| {
                            let before = records.clone();
                            mutate(records)?;
                            for record in records.iter_mut() {
                                if before.iter().any(|previous| {
                                    previous.id() == record.id() && previous != record
                                }) {
                                    record.set_review_schema_version(
                                        provenance_core::review::REVIEW_SCHEMA_VERSION,
                                    );
                                }
                            }
                            staged.stamp_records(&before, records)?;
                            crate::state_store::read_budget::ensure_slice_within_read_budget(
                                records,
                            )?;
                            Ok(records
                                .iter()
                                .filter_map(|after| {
                                    let previous = before
                                        .iter()
                                        .find(|previous| previous.id() == after.id())?;
                                    (previous != after).then(|| (previous.clone(), after.clone()))
                                })
                                .collect::<Vec<_>>())
                        })?;
                    let scope = changes
                        .first()
                        .map(|(_, after)| ReviewRecord::from(after.clone()).scope_id().clone());
                    for (before, after) in changes {
                        staged.commit_native_occurrence(
                            Some(&ReviewRecord::from(before)),
                            &ReviewRecord::from(after),
                        )?;
                    }
                    if let Some(scope) = scope {
                        staged.validate_graph_scope(&scope)?;
                        staged.enroll_review_manifest()?;
                    }
                    Ok(())
                })
            })
        })
    }
}
