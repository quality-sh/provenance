//! Stamp changed record content while the canonical publication lock is held.
use camino::Utf8Path;
use provenance_core::{
    model::relations::RelationOwner, Requirement, Resolution, Rule, Source, Stamp,
};
use serde::{de::DeserializeOwned, Serialize};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use super::StateStore;

pub(super) trait GraphRecord:
    RelationOwner + Clone + PartialEq + DeserializeOwned + Serialize
{
    fn validate_write(&self, _previous: Option<&Self>) -> anyhow::Result<()> {
        Ok(())
    }
    fn set_stamps(&mut self, _previous: Option<&Self>, _stamp: Option<&Stamp>) {}
}

macro_rules! stamps {
    () => {
        fn set_stamps(&mut self, previous: Option<&Self>, stamp: Option<&Stamp>) {
            self.created = previous.map_or_else(|| stamp.cloned(), |r| r.created.clone());
            self.updated = if previous.is_some_and(|r| r == self) {
                previous.and_then(|r| r.updated.clone())
            } else {
                stamp.cloned()
            };
        }
    };
}
impl GraphRecord for Source {
    stamps!();
}
impl GraphRecord for Requirement {
    stamps!();
}
impl GraphRecord for Resolution {
    stamps!();
}
impl GraphRecord for Rule {
    stamps!();
    fn validate_write(&self, previous: Option<&Self>) -> anyhow::Result<()> {
        previous
            .map_or_else(|| self.validate_archive(), |r| self.validate_transition(r))
            .map_err(|error| {
                crate::write_error::SourceFailure::wrap(
                    crate::write_error::WriteFailure::InvalidUpdate,
                    error,
                )
            })
    }
}
impl GraphRecord for provenance_core::Boundary {}
impl GraphRecord for provenance_core::Topic {}
impl GraphRecord for provenance_core::Question {}

impl StateStore {
    fn current_record_stamp(&self) -> anyhow::Result<Option<Stamp>> {
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
            record.set_stamps(previous, stamp.as_ref().and_then(Option::as_ref));
        }
        Ok(())
    }

    pub(super) fn mutate_graph_record<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        mutate: impl FnOnce(&mut Vec<T>) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        self.mutate_jsonl_records(path, |records: &mut Vec<T>| {
            let before = records.clone();
            let result = mutate(records)?;
            self.stamp_records(&before, records)?;
            records
                .iter()
                .find(|r| r.id() == result.id())
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("record mutation returned a missing record"))
        })
    }

    pub(super) fn replace_graph_records<T: GraphRecord>(
        &self,
        path: &Utf8Path,
        mut replacement: Vec<T>,
    ) -> anyhow::Result<()> {
        self.mutate_jsonl_records(path, |records| {
            self.stamp_records(records, &mut replacement)?;
            *records = replacement;
            Ok(())
        })
    }
}
