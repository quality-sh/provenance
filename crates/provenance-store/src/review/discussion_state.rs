use super::journal;
use crate::state_store::StateStore;
use provenance_core::{
    review::JournalEntry,
    threads::{DiscussionEntry, DiscussionOrigin},
    Message, NodeType, Requirement, ScopeId, Thread,
};
use std::collections::{BTreeMap, BTreeSet};

impl StateStore {
    pub(super) fn discussion_entries(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<Vec<DiscussionEntry>> {
        Ok(self
            .journal_entries(scope)?
            .into_iter()
            .filter_map(|e| match e {
                JournalEntry::Discussion(e) => Some(*e),
                JournalEntry::Requirement(_) => None,
            })
            .collect())
    }

    pub(super) fn discussion_heads(&self, scope: &ScopeId) -> anyhow::Result<Vec<DiscussionEntry>> {
        let entries = self.discussion_entries(scope)?;
        let threads = self.list_threads(scope)?;
        let messages = self.list_messages(scope)?;
        let requirements = self.list_requirements(scope)?;
        // Index each shard once. Entry validation below runs one pass over
        // these maps instead of one shard scan per entry, which kept
        // validation quadratic as Discussions and Messages accumulated.
        let mut threads_by_key = BTreeMap::<(&str, &str), &Thread>::new();
        for thread in &threads {
            threads_by_key
                .entry((thread.id.as_str(), thread.scope_id.as_str()))
                .or_insert(thread);
        }
        let mut messages_by_id = BTreeMap::<&str, Vec<&Message>>::new();
        for message in &messages {
            messages_by_id
                .entry(message.id.as_str())
                .or_default()
                .push(message);
        }
        let mut requirements_by_id = BTreeMap::<&str, Vec<&Requirement>>::new();
        for requirement in &requirements {
            requirements_by_id
                .entry(requirement.id.as_str())
                .or_default()
                .push(requirement);
        }
        let mut chains = BTreeMap::<&str, Vec<&DiscussionEntry>>::new();
        let mut ids = BTreeSet::new();
        let mut membership = BTreeSet::new();
        for entry in &entries {
            anyhow::ensure!(
                ids.insert(entry.id.as_str()),
                "duplicate Discussion entry identity"
            );
            anyhow::ensure!(!entry.actor.trim().is_empty(), "invalid Discussion actor");
            let thread = threads_by_key
                .get(&(entry.thread_id.as_str(), scope.as_str()))
                .copied()
                .ok_or_else(|| anyhow::anyhow!("Discussion Thread is missing"))?;
            anyhow::ensure!(
                thread.parent == entry.parent && entry.parent.node_type == NodeType::Requirement,
                "Discussion parent mismatch"
            );
            let matches = requirements_by_id
                .get(entry.parent.node_id.as_str())
                .is_some_and(|records| records.len() == 1 && records[0].scope_id == *scope);
            anyhow::ensure!(matches, "Requirement does not exist uniquely in this scope");
            if let Some(id) = &entry.message_id {
                anyhow::ensure!(
                    membership.insert(id.as_str()),
                    "Message belongs to multiple Discussion entries"
                );
                let matches = messages_by_id.get(id.as_str()).is_some_and(|candidates| {
                    candidates.len() == 1
                        && candidates
                            .iter()
                            .any(|m| m.scope_id == *scope && m.thread_id == entry.thread_id)
                });
                anyhow::ensure!(matches, "Discussion Message membership mismatch");
            }
            chains
                .entry(entry.discussion_id.as_str())
                .or_default()
                .push(entry);
        }
        let entry_thread_ids: BTreeSet<&str> =
            entries.iter().map(|e| e.thread_id.as_str()).collect();
        for message in messages
            .iter()
            .filter(|m| m.schema_version == provenance_core::review::REVIEW_SCHEMA_VERSION)
        {
            anyhow::ensure!(
                membership.contains(message.id.as_str()),
                "enrolled Message has no Discussion membership"
            );
        }
        for thread in threads
            .iter()
            .filter(|t| t.schema_version == provenance_core::review::REVIEW_SCHEMA_VERSION)
        {
            anyhow::ensure!(
                entry_thread_ids.contains(thread.id.as_str()),
                "enrolled Thread has no Discussion history"
            );
        }
        let mut heads = Vec::new();
        for mut chain in chains.into_values() {
            chain.sort_by_key(|e| e.version);
            let mut previous: Option<&DiscussionEntry> = None;
            for entry in chain {
                entry.validate_after(previous)?;
                previous = Some(entry);
            }
            heads.push(previous.unwrap().clone());
        }
        Ok(heads)
    }

    pub(super) fn validate_discussion_origin(
        &self,
        scope: &ScopeId,
        origin: &DiscussionOrigin,
    ) -> anyhow::Result<()> {
        self.discussion_heads(scope)?;
        let entries = self.discussion_entries(scope)?;
        self.validate_discussion_origin_among(scope, origin, &entries)
    }

    /// Checks one origin against already-read Discussion entries, so callers
    /// that validated the Discussion state once need not revalidate per origin.
    pub(super) fn validate_discussion_origin_among(
        &self,
        scope: &ScopeId,
        origin: &DiscussionOrigin,
        entries: &[DiscussionEntry],
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            entries
                .iter()
                .any(|e| e.discussion_id == origin.discussion_id
                    && e.thread_id == origin.thread_id
                    && e.message_id.as_ref() == Some(&origin.message_id)),
            "outcome origin is not a Message in this Discussion and scope"
        );
        self.validate_requirement_origin(scope, Some(&origin.thread_id), Some(&origin.message_id))
    }

    pub fn discussion_receipt(
        &self,
        input: &super::WriteDiscussion,
    ) -> anyhow::Result<Option<DiscussionEntry>> {
        self.with_repository_publication(|| {
            self.authorize_discussion(input)?;
            let path = journal::entry_path(&self.layout, &input.scope_id, &input.request_id);
            if !path.try_exists()? {
                return Ok(None);
            }
            let JournalEntry::Discussion(entry) = journal::read_journal_entry(&self.layout, &path)?
            else {
                anyhow::bail!("request ID belongs to a Requirement save");
            };
            anyhow::ensure!(
                entry.scope_id == input.scope_id
                    && entry.request_id == input.request_id
                    && entry.actor == input.actor
                    && entry.parent == input.parent
                    && entry.intent_digest == super::discussion_writes::intent(input)?,
                "Discussion request ID was reused with different intent"
            );
            Ok(Some(*entry))
        })
    }

    pub(super) fn authorize_discussion(
        &self,
        input: &super::WriteDiscussion,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(!input.actor.trim().is_empty(), "invalid Discussion actor");
        anyhow::ensure!(
            self.manifest()?
                .scopes
                .iter()
                .any(|s| s.id == input.scope_id),
            "Discussion scope is not in the manifest"
        );
        anyhow::ensure!(
            input.parent.node_type == NodeType::Requirement,
            "addressed Discussions currently require a Requirement parent"
        );
        let mut ids = BTreeSet::new();
        for thread in self.list_threads(&input.scope_id)? {
            anyhow::ensure!(
                ids.insert(thread.id.as_str().to_owned()),
                "duplicate Thread identity"
            );
            if thread.parent == input.parent {
                anyhow::ensure!(
                    thread.scope_id == input.scope_id,
                    "Thread parent scope mismatch"
                );
            }
        }
        let record = self.requirement(&input.scope_id, &input.parent.node_id)?;
        anyhow::ensure!(
            record.scope_id == input.scope_id,
            "Requirement parent scope mismatch"
        );
        super::owner_matches(&record, input.declared_by.as_deref())
    }
}
