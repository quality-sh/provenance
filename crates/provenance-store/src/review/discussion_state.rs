use super::journal;
use crate::state_store::StateStore;
use provenance_core::{
    review::JournalEntry,
    threads::{DiscussionEntry, DiscussionOrigin},
    Message, NodeType, ScopeId, StableId, Thread, ThreadParent,
};
use std::collections::{BTreeMap, BTreeSet};

use provenance_core::{Question, Requirement, Resolution, Rule, Source, Topic};

/// Every record of a kind that takes Discussions, as the parent index reads it.
trait IndexedParent {
    fn indexed_id(&self) -> &StableId;
    fn indexed_scope(&self) -> &ScopeId;
}

macro_rules! indexed_parent {
    ($($kind:ty),* $(,)?) => {
        $(
            impl IndexedParent for $kind {
                fn indexed_id(&self) -> &StableId { &self.id }
                fn indexed_scope(&self) -> &ScopeId { &self.scope_id }
            }
        )*
    };
}

indexed_parent!(Source, Requirement, Resolution, Rule, Topic, Question);

/// Indexes one parent kind's shard as `(kind word, id) -> scope words`, so a
/// parent resolves to exactly one record in this scope.
fn index_parent_kind<'a, K: IndexedParent>(
    index: &mut BTreeMap<(&'a str, &'a str), Vec<&'a str>>,
    kind: NodeType,
    records: &'a [K],
) {
    let word: &'static str = discussion_kind_word(kind).unwrap_or_default();
    for record in records {
        index
            .entry((word, record.indexed_id().as_str()))
            .or_default()
            .push(record.indexed_scope().as_str());
    }
}

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
                JournalEntry::Requirement(_) | JournalEntry::Cycle(_) => None,
            })
            .collect())
    }

    pub(super) fn discussion_heads(&self, scope: &ScopeId) -> anyhow::Result<Vec<DiscussionEntry>> {
        let entries = self.discussion_entries(scope)?;
        let threads = self.list_threads(scope)?;
        let messages = self.list_messages(scope)?;
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
        let mut parents_by_key = BTreeMap::<(&str, &str), Vec<&str>>::new();
        let sources = self.list_sources(scope)?;
        index_parent_kind(&mut parents_by_key, NodeType::Source, &sources);
        let requirements = self.list_requirements(scope)?;
        index_parent_kind(&mut parents_by_key, NodeType::Requirement, &requirements);
        let resolutions = self.list_resolutions(scope)?;
        index_parent_kind(&mut parents_by_key, NodeType::Resolution, &resolutions);
        let rules = self.list_rules(scope)?;
        index_parent_kind(&mut parents_by_key, NodeType::Rule, &rules);
        let topics = self.list_topics(scope)?;
        index_parent_kind(&mut parents_by_key, NodeType::Topic, &topics);
        let questions = self.list_questions(scope)?;
        index_parent_kind(&mut parents_by_key, NodeType::Question, &questions);
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
                thread.parent == entry.parent
                    && discussion_kind_word(entry.parent.node_type).is_some(),
                "Discussion parent mismatch"
            );
            let in_scope = parents_by_key
                .get(&(
                    discussion_kind_word(entry.parent.node_type).unwrap_or_default(),
                    entry.parent.node_id.as_str(),
                ))
                .is_some_and(|scopes| scopes.len() == 1 && scopes[0] == scope.as_str());
            anyhow::ensure!(
                in_scope,
                "Discussion parent does not exist uniquely in this scope"
            );
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

    /// Receipt resolution stays internal to the Store: the Discussion write
    /// path resolves request identity itself.
    pub(crate) fn discussion_receipt(
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
        let owner = self.resolve_discussion_parent(&input.scope_id, &input.parent)?;
        match input.parent.node_type {
            // A claim is a work lock, not authorship: any participant may
            // raise a concern on a topic or question.
            NodeType::Topic | NodeType::Question => anyhow::ensure!(
                input.declared_by.is_none(),
                "a topic or question parent takes no declared owner"
            ),
            _ => parent_owner_matches(owner.as_deref(), input.declared_by.as_deref())?,
        }
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
        Ok(())
    }
}

/// The owner fact a Discussion parent carries: the record must exist uniquely
/// in the scope, and the kind decides which field owns it.
pub(super) fn parent_owner_matches(
    saved: Option<&str>,
    supplied: Option<&str>,
) -> anyhow::Result<()> {
    if saved != supplied {
        return Err(crate::write_error::SourceFailure::wrap(
            crate::write_error::WriteFailure::RecordOwnershipConflict,
            anyhow::anyhow!(
                "declared_by must match the existing owner; omit it for a manual record"
            ),
        ));
    }
    Ok(())
}

/// The wire word of one supported Discussion parent kind, or `None` for a
/// kind that takes no Discussion.
pub(super) fn discussion_kind_word(kind: NodeType) -> Option<&'static str> {
    match kind {
        NodeType::Source => Some("source"),
        NodeType::Requirement => Some("requirement"),
        NodeType::Resolution => Some("resolution"),
        NodeType::Rule => Some("rule"),
        NodeType::Topic => Some("topic"),
        NodeType::Question => Some("question"),
        _ => None,
    }
}

/// The projection table of one parent kind, named like its attested family.
pub(super) fn parent_table(kind: NodeType) -> Option<&'static str> {
    discussion_kind_word(kind).map(|word| match word {
        "source" => "sources",
        "requirement" => "requirements",
        "resolution" => "resolutions",
        "rule" => "rules",
        "topic" => "topics",
        _ => "questions",
    })
}

/// One record of a kind that takes Discussions, as the parent check reads it.
trait DiscussionParent {
    fn parent_id(&self) -> &StableId;
    fn parent_scope(&self) -> &ScopeId;
    /// The field that owns the record, if the kind has one.
    fn owner_field(&self) -> Option<&str>;
}

macro_rules! discussion_parent {
    ($($kind:ty => $owner:ident),* $(,)?) => {
        $(
            impl DiscussionParent for $kind {
                fn parent_id(&self) -> &StableId { &self.id }
                fn parent_scope(&self) -> &ScopeId { &self.scope_id }
                fn owner_field(&self) -> Option<&str> { self.$owner.as_deref() }
            }
        )*
    };
}

discussion_parent!(
    Source => declared_by,
    Requirement => declared_by,
    Resolution => made_by,
    Rule => declared_by,
    Topic => claimed_by,
    Question => claimed_by,
);

impl StateStore {
    /// Resolves one Discussion parent: the record must exist uniquely in the
    /// scope, and the kind's owner fact comes back for the ownership check.
    /// A topic or question claim is a work lock, so it is not an owner fact.
    pub(super) fn resolve_discussion_parent(
        &self,
        scope: &ScopeId,
        parent: &ThreadParent,
    ) -> anyhow::Result<Option<String>> {
        anyhow::ensure!(
            discussion_kind_word(parent.node_type).is_some(),
            "thread parent kind does not take Discussions: {parent:?}"
        );
        match parent.node_type {
            NodeType::Source => {
                self.resolve_parent_among(&self.list_sources(scope)?, scope, parent)
            }
            NodeType::Requirement => {
                self.resolve_parent_among(&self.list_requirements(scope)?, scope, parent)
            }
            NodeType::Resolution => {
                self.resolve_parent_among(&self.list_resolutions(scope)?, scope, parent)
            }
            NodeType::Rule => {
                self.resolve_parent_among(&self.list_rules(scope)?, scope, parent)
            }
            NodeType::Topic => {
                self.resolve_parent_among(&self.list_topics(scope)?, scope, parent)
            }
            NodeType::Question => {
                self.resolve_parent_among(&self.list_questions(scope)?, scope, parent)
            }
            kind => anyhow::bail!(
                "thread parent kind `{}` does not take Discussions",
                discussion_kind_word(kind).unwrap_or("unsupported")
            ),
        }
    }

    fn resolve_parent_among<T: DiscussionParent>(
        &self,
        records: &[T],
        scope: &ScopeId,
        parent: &ThreadParent,
    ) -> anyhow::Result<Option<String>> {
        let matches = records
            .iter()
            .filter(|record| record.parent_id() == &parent.node_id)
            .collect::<Vec<_>>();
        anyhow::ensure!(
            matches.len() <= 1,
            "Discussion parent identity is not unique"
        );
        let record = matches
            .first()
            .ok_or_else(|| anyhow::anyhow!("Discussion parent does not exist in this scope"))?;
        anyhow::ensure!(
            record.parent_scope() == scope,
            "Discussion parent scope mismatch"
        );
        Ok(record.owner_field().map(str::to_owned))
    }
}
