//! Typed-spec apply deletes the Discussions of the records that it deletes.

use super::guard;
use crate::{
    shards,
    state_store::{readers, StateStore},
};
use camino::Utf8Path;
use provenance_core::{
    review::ReviewRecord, threads::Discussion, Message, NodeType, Requirement, Resolution, Rule,
    ScopeId, Source, StableId, Thread,
};
use provenance_macros::rule;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::BTreeSet;

/// The Thread and Message that a record cites as its origin.
trait Origin {
    fn origin(&self) -> (&Option<StableId>, &Option<StableId>);
    fn clear_origin(&mut self);
}

macro_rules! origin {
    ($($record:ty),*) => {$(
        impl Origin for $record {
            fn origin(&self) -> (&Option<StableId>, &Option<StableId>) {
                (&self.origin_thread, &self.origin_message)
            }

            fn clear_origin(&mut self) {
                self.origin_thread = None;
                self.origin_message = None;
            }
        }
    )*};
}
origin!(Source, Requirement, Resolution, Rule);

/// The ids of the deleted Threads and Messages.
struct Removed<'a> {
    threads: BTreeSet<&'a str>,
    messages: BTreeSet<String>,
}

impl Removed<'_> {
    fn cited_by(&self, record: &impl Origin) -> bool {
        let (thread, message) = record.origin();
        thread
            .as_ref()
            .is_some_and(|id| self.threads.contains(id.as_str()))
            || message
                .as_ref()
                .is_some_and(|id| self.messages.contains(id.as_str()))
    }
}

impl StateStore {
    /// Deletes each Discussion whose parent is one of `records`, active or
    /// resolved, with its Thread and the Messages of that Thread. A surviving
    /// record that cites a deleted Thread or Message loses that origin. Git
    /// keeps the history of the deleted Discussions.
    #[rule("rule_typed_deletion_deletes_discussions")]
    pub(super) fn delete_record_discussions(
        &self,
        scope: &ScopeId,
        records: &[ReviewRecord],
    ) -> anyhow::Result<()> {
        let deleted = self
            .list_discussions(scope)?
            .into_iter()
            .filter(|discussion| anchored_to(discussion, records))
            .collect::<Vec<_>>();
        if deleted.is_empty() {
            return Ok(());
        }
        let discussion_ids = deleted
            .iter()
            .map(|discussion| discussion.discussion_id.as_str())
            .collect::<BTreeSet<_>>();
        let mut removed = Removed {
            threads: deleted
                .iter()
                .map(|discussion| discussion.thread_id.as_str())
                .collect(),
            messages: BTreeSet::new(),
        };
        let discussions_path = shards::discussions_path(&self.layout, scope);
        let threads_path = shards::threads_path(&self.layout, scope);
        // Only the month shards that hold a removed Message are written.
        let mut message_paths = Vec::new();
        for path in readers::message_shard_paths(&self.layout, scope)? {
            let messages: Vec<Message> = readers::read_jsonl(self, &path)?;
            let before = removed.messages.len();
            removed.messages.extend(
                messages
                    .iter()
                    .filter(|m| removed.threads.contains(m.thread_id.as_str()))
                    .map(|m| m.id.as_str().to_owned()),
            );
            if removed.messages.len() > before {
                message_paths.push(path);
            }
        }
        let mut paths = vec![discussions_path.clone(), threads_path.clone()];
        paths.extend(message_paths.iter().cloned());
        let kinds = [
            NodeType::Source,
            NodeType::Requirement,
            NodeType::Resolution,
            NodeType::Rule,
        ];
        paths.extend(
            kinds
                .into_iter()
                .map(|kind| shards::path_for(&self.layout, scope, kind)),
        );
        guard::with_writers(&paths, "*", || {
            self.mutate_jsonl_records(&discussions_path, |rows: &mut Vec<Discussion>| {
                rows.retain(|d| !discussion_ids.contains(d.discussion_id.as_str()));
                Ok(())
            })?;
            for path in &message_paths {
                self.mutate_jsonl_records(path, |rows: &mut Vec<Message>| {
                    rows.retain(|m| !removed.threads.contains(m.thread_id.as_str()));
                    Ok(())
                })?;
            }
            self.mutate_jsonl_records(&threads_path, |rows: &mut Vec<Thread>| {
                rows.retain(|t| !removed.threads.contains(t.id.as_str()));
                Ok(())
            })?;
            let path = |kind| shards::path_for(&self.layout, scope, kind);
            self.clear_origins::<Source>(&path(NodeType::Source), &removed)?;
            self.clear_origins::<Requirement>(&path(NodeType::Requirement), &removed)?;
            self.clear_origins::<Resolution>(&path(NodeType::Resolution), &removed)?;
            self.clear_origins::<Rule>(&path(NodeType::Rule), &removed)
        })
    }

    /// Removes the origin from each record in `path` that cites a removed
    /// Thread or Message, and leaves the file as it is if no record does.
    #[rule("rule_typed_deletion_clears_origins")]
    fn clear_origins<T>(&self, path: &Utf8Path, removed: &Removed) -> anyhow::Result<()>
    where
        T: Origin + DeserializeOwned + Serialize,
    {
        if !path.exists() {
            return Ok(());
        }
        self.mutate_jsonl_records_if(path, |rows: &mut Vec<T>| {
            let mut cited = false;
            for row in rows.iter_mut() {
                if removed.cited_by(&*row) {
                    row.clear_origin();
                    cited = true;
                }
            }
            Ok(cited)
        })?;
        Ok(())
    }
}

fn anchored_to(discussion: &Discussion, records: &[ReviewRecord]) -> bool {
    records.iter().any(|record| {
        discussion.parent.node_type == record.kind() && discussion.parent.node_id == *record.id()
    })
}
