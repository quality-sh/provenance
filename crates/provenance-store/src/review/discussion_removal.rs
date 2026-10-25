//! Typed-spec apply deletes the Discussions of the records that it deletes.

use super::guard;
use crate::{
    shards,
    state_store::{readers, StateStore},
};
use provenance_core::{review::ReviewRecord, threads::Discussion, Message, ScopeId, Thread};
use provenance_macros::rule;
use std::collections::BTreeSet;

impl StateStore {
    /// Deletes each Discussion whose parent is one of `records`, active or
    /// resolved, with its Thread and the Messages of that Thread. Git keeps
    /// the history of the deleted Discussions.
    #[rule("rule_typed_deletion_deletes_discussions")]
    pub(super) fn delete_record_discussions(
        &self,
        scope: &ScopeId,
        records: &[ReviewRecord],
    ) -> anyhow::Result<()> {
        let removed = self
            .list_discussions(scope)?
            .into_iter()
            .filter(|discussion| anchored_to(discussion, records))
            .collect::<Vec<_>>();
        if removed.is_empty() {
            return Ok(());
        }
        let discussion_ids = removed
            .iter()
            .map(|discussion| discussion.discussion_id.as_str())
            .collect::<BTreeSet<_>>();
        let thread_ids = removed
            .iter()
            .map(|discussion| discussion.thread_id.as_str())
            .collect::<BTreeSet<_>>();
        let discussions_path = shards::discussions_path(&self.layout, scope);
        let threads_path = shards::threads_path(&self.layout, scope);
        // Only the month shards that hold a removed Message are written.
        let mut message_paths = Vec::new();
        for path in readers::message_shard_paths(&self.layout, scope)? {
            let messages: Vec<Message> = readers::read_jsonl(self, &path)?;
            if messages
                .iter()
                .any(|m| thread_ids.contains(m.thread_id.as_str()))
            {
                message_paths.push(path);
            }
        }
        let mut paths = vec![discussions_path.clone(), threads_path.clone()];
        paths.extend(message_paths.iter().cloned());
        guard::with_writers(&paths, "*", || {
            self.mutate_jsonl_records(&discussions_path, |rows: &mut Vec<Discussion>| {
                rows.retain(|d| !discussion_ids.contains(d.discussion_id.as_str()));
                Ok(())
            })?;
            for path in &message_paths {
                self.mutate_jsonl_records(path, |rows: &mut Vec<Message>| {
                    rows.retain(|m| !thread_ids.contains(m.thread_id.as_str()));
                    Ok(())
                })?;
            }
            self.mutate_jsonl_records(&threads_path, |rows: &mut Vec<Thread>| {
                rows.retain(|t| !thread_ids.contains(t.id.as_str()));
                Ok(())
            })
        })
    }
}

fn anchored_to(discussion: &Discussion, records: &[ReviewRecord]) -> bool {
    records.iter().any(|record| {
        discussion.parent.node_type == record.kind() && discussion.parent.node_id == *record.id()
    })
}
