//! Canonical staged writer for one or more native graph record changes.

use super::{guard, journal};
use crate::{
    publication::with_staged_state,
    state_store::{
        read_budget::ensure_slice_within_read_budget, record_stamps::GraphRecord, StateStore,
    },
    write_error::{SourceFailure, WriteFailure},
};
use camino::Utf8Path;
use provenance_core::{
    review::{ReviewRecord, REVIEW_SCHEMA_VERSION},
    StableId,
};

pub(crate) struct NativeRecordBatch<'a> {
    store: &'a StateStore,
}

impl StateStore {
    pub(crate) fn with_native_record_batch<R>(
        &self,
        write: impl FnOnce(&NativeRecordBatch<'_>) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let stamp = self.current_record_stamp()?;
        with_staged_state(&self.layout, false, |layout| {
            let staged = Self::staged(layout.clone(), stamp);
            write(&NativeRecordBatch { store: &staged })
        })
    }

    pub(crate) fn save_native_record<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        expected_etag: Option<&str>,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let relative = path.strip_prefix(self.layout.root())?.to_owned();
        self.with_native_record_batch(|batch| {
            let staged_path = batch.store.layout.root().join(relative);
            batch.mutate_one(&staged_path, expected_etag, mutate)
        })
    }
}

impl NativeRecordBatch<'_> {
    pub(crate) fn store(&self) -> &StateStore {
        self.store
    }

    pub(crate) fn mutate_one<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        expected_etag: Option<&str>,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let (result, changes) = self.write(path, expected_etag, |records| {
            let result = mutate(records)?;
            Ok((result.clone(), Some(result.id().clone())))
        })?;
        Ok(changes
            .into_iter()
            .map(|(_, after)| after)
            .find(|record| record.id() == result.id())
            .unwrap_or(result))
    }

    pub(crate) fn mutate_all<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        self.write(path, None, |records| {
            mutate(records)?;
            Ok(((), None))
        })
        .map(|_| ())
    }

    fn write<T, R>(
        &self,
        path: &Utf8Path,
        expected_etag: Option<&str>,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<(R, Option<StableId>)>,
    ) -> anyhow::Result<(R, Vec<(T, T)>)>
    where
        T: GraphRecord,
    {
        guard::with_writer(path, "*", || {
            let (result, changes) = self.store.mutate_jsonl_records(path, |records: &mut Vec<T>| {
                let before = records.clone();
                let (result, guarded_id) = mutate(records)?;
                enroll_closed_changes(&before, records)?;
                self.store.stamp_records(&before, records)?;
                ensure_slice_within_read_budget(records)?;
                if let (Some(expected), Some(id)) = (expected_etag, guarded_id.as_ref()) {
                    check_etag(self.store, &before, id, expected)?;
                }
                let changes = records
                    .iter()
                    .filter_map(|after| {
                        let previous = before.iter().find(|record| record.id() == after.id())?;
                        (previous != after).then(|| (previous.clone(), after.clone()))
                    })
                    .collect::<Vec<_>>();
                Ok((result, changes))
            })?;
            for (before, after) in &changes {
                let before: ReviewRecord = before.clone().into();
                let after: ReviewRecord = after.clone().into();
                self.store.commit_native_occurrence(Some(&before), &after)?;
            }
            if let Some((_, after)) = changes.first() {
                let review: ReviewRecord = after.clone().into();
                self.store.validate_graph_scope(review.scope_id())?;
                self.store.enroll_review_manifest()?;
            }
            Ok((result, changes))
        })
    }
}

fn enroll_closed_changes<T: GraphRecord>(before: &[T], after: &mut [T]) -> anyhow::Result<()> {
    for record in after {
        let changed = before.iter().find(|previous| previous.id() == record.id()) != Some(&*record);
        if changed
            && ReviewRecord::deserialize_closed(T::KIND, &serde_json::to_value(&*record)?).is_ok()
        {
            record.set_review_schema_version(REVIEW_SCHEMA_VERSION);
        }
    }
    Ok(())
}

fn check_etag<T: GraphRecord>(
    store: &StateStore,
    before: &[T],
    id: &StableId,
    expected: &str,
) -> anyhow::Result<()> {
    let record = before
        .iter()
        .find(|record| record.id() == id)
        .ok_or_else(|| anyhow::anyhow!("native update cannot create a graph record"))?;
    let record: ReviewRecord = record.clone().into();
    let current_etag = store
        .head(&record)?
        .map(|entry| entry.etag)
        .unwrap_or(journal::etag(&record, None)?);
    if expected != current_etag {
        return Err(SourceFailure::wrap(
            WriteFailure::RecordEditConflict {
                record_kind: T::KIND,
                current_etag,
            },
            anyhow::anyhow!("stale native record edit etag"),
        ));
    }
    Ok(())
}
