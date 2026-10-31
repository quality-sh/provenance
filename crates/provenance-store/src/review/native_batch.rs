//! Canonical staged writer for one or more native graph record changes.

use crate::{
    review::publication::with_record_state,
    state_store::{
        read_budget::ensure_slice_within_read_budget, record_stamps::GraphRecord, RecordStamp,
        StateStore,
    },
    write_error::{SourceFailure, WriteFailure},
};
use camino::Utf8Path;
use provenance_core::{review::ReviewRecord, StableId};

pub struct NativeRecordBatch<'a> {
    store: &'a StateStore,
}

impl StateStore {
    pub(crate) fn with_native_record_batch<R>(
        &self,
        write: impl FnOnce(&NativeRecordBatch<'_>) -> anyhow::Result<R>,
    ) -> anyhow::Result<R> {
        let stamp = self.current_record_stamp()?;
        with_record_state(self, RecordStamp::resolved(stamp), |staged| {
            write(&NativeRecordBatch { store: staged })
        })
    }

    pub(crate) fn save_native_record<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        expected_etag: Option<&str>,
        mutate: impl FnOnce(&Self, &mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let relative = path.strip_prefix(self.layout.root())?.to_owned();
        self.with_native_record_batch(|batch| {
            let staged_path = batch.store.layout.root().join(relative);
            batch.mutate_one_with_store(&staged_path, expected_etag, mutate)
        })
    }
}

impl NativeRecordBatch<'_> {
    pub(crate) const fn store(&self) -> &StateStore {
        self.store
    }

    pub(crate) fn mutate_one<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        expected_etag: Option<&str>,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        self.mutate_one_with_store(path, expected_etag, |_, records| mutate(records))
    }

    /// Like [`Self::mutate_one`], but `mutate` also gets the store that holds
    /// the staged section.
    fn mutate_one_with_store<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        expected_etag: Option<&str>,
        mutate: impl FnOnce(&StateStore, &mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        let (result, changes) = self.write(path, expected_etag, |held, records| {
            let result = mutate(held, records)?;
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
        self.write(path, None, |_, records| {
            mutate(records)?;
            Ok(((), None))
        })
        .map(|_| ())
    }

    fn write<T, R>(
        &self,
        path: &Utf8Path,
        expected_etag: Option<&str>,
        mutate: impl FnOnce(&StateStore, &mut Vec<T>) -> anyhow::Result<(R, Option<StableId>)>,
    ) -> anyhow::Result<(R, Vec<(T, T)>)>
    where
        T: GraphRecord,
    {
        self.store.permitting(&[path], "*", |store| {
            let (result, changes) =
                store.mutate_jsonl_records_with_store(path, |held, records: &mut Vec<T>| {
                    let before = records.clone();
                    let (result, guarded_id) = mutate(held, records)?;
                    held.stamp_records(&before, records)?;
                    ensure_slice_within_read_budget(records)?;
                    if let (Some(expected), Some(id)) = (expected_etag, guarded_id.as_ref()) {
                        check_etag(&before, id, expected)?;
                    }
                    let changes = records
                        .iter()
                        .filter_map(|after| {
                            let previous =
                                before.iter().find(|record| record.id() == after.id())?;
                            (previous != after).then(|| (previous.clone(), after.clone()))
                        })
                        .collect::<Vec<_>>();
                    Ok((result, changes))
                })?;
            if let Some((_, after)) = changes.first() {
                let review: ReviewRecord = after.clone().into();
                store.validate_graph_scope(review.scope_id())?;
            }
            Ok((result, changes))
        })
    }
}

fn check_etag<T: GraphRecord>(before: &[T], id: &StableId, expected: &str) -> anyhow::Result<()> {
    let record = before
        .iter()
        .find(|record| record.id() == id)
        .ok_or_else(|| anyhow::anyhow!("native update cannot create a graph record"))?;
    let record: ReviewRecord = record.clone().into();
    let current_etag = super::save::etag(&record)?;
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
