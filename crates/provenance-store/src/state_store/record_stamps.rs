//! Stamp changed record content while the canonical publication lock is held.
use camino::Utf8Path;
use provenance_core::{
    review::{ReviewRecord, ReviewRecordKind, REVIEW_SCHEMA_VERSION},
    NodeType, Stamp,
};
use serde::{de::DeserializeOwned, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::read_budget::{ensure_slice_within_read_budget, ensure_within_read_budget, ReadBudget};
use super::{RecordStamp, StateStore};

pub trait GraphRecord:
    Clone
    + Into<ReviewRecord>
    + PartialEq
    + DeserializeOwned
    + Serialize
    + ReadBudget
    + ReviewRecordKind
    + StoredReviewRecord
{
    fn id(&self) -> &provenance_core::StableId {
        self.review_id()
    }

    fn validate_write(&self, previous: Option<&Self>) -> anyhow::Result<()> {
        match (self.clone().into(), previous.cloned().map(Into::into)) {
            (ReviewRecord::Rule(rule), Some(ReviewRecord::Rule(previous))) => {
                rule.validate_transition(&previous).map_err(invalid_update)
            }
            (ReviewRecord::Rule(rule), None) => rule.validate_archive().map_err(invalid_update),
            (ReviewRecord::Topic(topic), Some(ReviewRecord::Topic(previous))) => {
                topic.validate_transition(&previous).map_err(invalid_update)
            }
            (ReviewRecord::Topic(topic), None) => topic.validate_archive().map_err(invalid_update),
            (ReviewRecord::Question(question), Some(ReviewRecord::Question(previous))) => question
                .validate_transition(&previous)
                .map_err(invalid_update),
            (ReviewRecord::Question(question), None) => {
                question.validate_archive().map_err(invalid_update)
            }
            _ => Ok(()),
        }
    }
    fn set_stamps(&mut self, previous: Option<&Self>, stamp: Option<&Stamp>) -> anyhow::Result<()> {
        let mut value = serde_json::to_value(&*self)?;
        let Some(fields) = value.as_object_mut() else {
            return Ok(());
        };
        let previous_value = previous.map(serde_json::to_value).transpose()?;
        let created = match &previous_value {
            Some(record) => record
                .get("created")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
            None => serde_json::to_value(stamp)?,
        };
        let updated = if previous.is_some_and(|record| record == self) {
            previous_value
                .as_ref()
                .and_then(|record| record.get("updated"))
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::to_value(stamp)?
        };
        fields.insert("created".into(), created);
        fields.insert("updated".into(), updated);
        *self = serde_json::from_value(value)?;
        Ok(())
    }
}

impl<T> GraphRecord for T where
    T: Clone
        + Into<ReviewRecord>
        + PartialEq
        + DeserializeOwned
        + Serialize
        + ReadBudget
        + ReviewRecordKind
        + StoredReviewRecord
{
}

fn invalid_update(error: anyhow::Error) -> anyhow::Error {
    crate::write_error::SourceFailure::wrap(crate::write_error::WriteFailure::InvalidUpdate, error)
}

pub trait StoredReviewRecord {}

macro_rules! define_stored_review_records {
    (
        export { $(
            $variant:ident {
                record: $record:ty,
                field: $field:ident,
                path: $path:ident,
                meta: $meta:tt,
                node: [$kind:ident],
                reader: $reader:tt,
                id: $id:ident,
                loader: $loader:tt,
                graph: $graph:tt,
                import: $import:tt,
                catalog: $catalog:tt,
                route: $route:tt,
                review: $review:ident
            };
        )* }
        $($other:tt)*
    ) => {
        $(impl StoredReviewRecord for $record {})*
    };
}

crate::cache::family_table::record_family_rows!(define_stored_review_records);

impl StateStore {
    pub(crate) fn current_record_stamp(&self) -> anyhow::Result<Option<Stamp>> {
        match &self.record_stamp {
            RecordStamp::Omit => return Ok(None),
            RecordStamp::Fixed(stamp) => return Ok(Some(stamp.clone())),
            RecordStamp::ResolveFromRepository => {}
        }
        let output = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(self.layout.root())
            .output();
        // A new repository without a commit can still accept its first graph.
        let Ok(output) = output else {
            return Ok(None);
        };
        if !output.status.success() {
            return Ok(None);
        }
        let commit = String::from_utf8(output.stdout)?.trim().to_owned();
        provenance_core::model::record_stamps::validate_commit(&commit)?;
        Ok(Some(Stamp {
            commit,
            at: OffsetDateTime::now_utc().format(&Rfc3339)?,
        }))
    }

    pub(super) fn stamp_records<T: GraphRecord>(
        &self,
        before: &[T],
        after: &mut [T],
    ) -> anyhow::Result<()> {
        let by_id: std::collections::BTreeMap<_, _> =
            before.iter().map(|r| (r.id().as_str(), r)).collect();
        let mut stamp = None;
        for record in after {
            let previous = by_id.get(record.id().as_str()).copied();
            record.validate_write(previous)?;
            if previous != Some(&*record) && stamp.is_none() {
                stamp = Some(self.current_record_stamp()?);
            }
            record.set_stamps(previous, stamp.as_ref().and_then(Option::as_ref))?;
        }
        Ok(())
    }

    pub(crate) fn enroll_graph_record<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        id: &provenance_core::StableId,
    ) -> anyhow::Result<T> {
        self.mutate_graph_record(path, |records: &mut Vec<T>| {
            let record = records
                .iter_mut()
                .find(|record| record.id() == id)
                .ok_or_else(|| anyhow::anyhow!("created graph record is missing"))?;
            record.set_review_schema_version(REVIEW_SCHEMA_VERSION);
            Ok(record.clone())
        })
    }

    pub(crate) fn mutate_graph_record<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        self.mutate_graph_record_with_etag(path, None, mutate)
    }

    pub(crate) fn mutate_graph_record_with_etag<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        expected_etag: Option<&str>,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        if T::KIND != NodeType::Requirement && !crate::review::guard::writer_allows_path(path) {
            return self.save_native_record(path, expected_etag, mutate);
        }
        self.mutate_graph_record_guarded(path, mutate)
            .map(|(_, record)| record)
    }

    pub(crate) fn mutate_graph_record_guarded<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<(Option<T>, T)> {
        self.mutate_jsonl_records(path, |records: &mut Vec<T>| {
            let before = records.clone();
            let result = mutate(records)?;
            self.stamp_records(&before, records)?;
            let stamped = records
                .iter()
                .find(|r| r.id() == result.id())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("record mutation returned a missing record"))?;
            ensure_within_read_budget(&stamped)?;
            let previous = before
                .into_iter()
                .find(|record| record.id() == stamped.id());
            Ok((previous, stamped))
        })
    }

    pub(super) fn replace_graph_records<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        replacement: Vec<T>,
    ) -> anyhow::Result<()> {
        let has_stored_record = !super::readers::read_jsonl::<T>(self, path)?.is_empty();
        if T::KIND != NodeType::Requirement
            && has_stored_record
            && !crate::review::guard::writer_allows_path(path)
        {
            return self.replace_native_records(path, replacement);
        }
        self.replace_graph_records_guarded(path, replacement)
            .map(|_| ())
    }

    pub(crate) fn replace_graph_records_guarded<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        mut replacement: Vec<T>,
    ) -> anyhow::Result<Vec<T>> {
        self.mutate_jsonl_records(path, |records| {
            let before = records.clone();
            self.stamp_records(records, &mut replacement)?;
            ensure_slice_within_read_budget(&replacement)?;
            *records = replacement;
            Ok(before)
        })
    }
}
