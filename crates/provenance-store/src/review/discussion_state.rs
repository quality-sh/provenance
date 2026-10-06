use crate::{shards, state_store::StateStore};
use provenance_core::{
    threads::{Discussion, DiscussionOrigin, DiscussionOutcome},
    Message, NodeType, Question, Requirement, Resolution, Rule, ScopeId, Source, StableId, Thread,
    ThreadParent, Topic,
};
use std::collections::{BTreeMap, BTreeSet};

trait DiscussionParent {
    fn parent_id(&self) -> &StableId;
    fn parent_scope(&self) -> &ScopeId;
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
    fn index_discussion_parents(
        &self,
        scope: &ScopeId,
    ) -> anyhow::Result<BTreeMap<(&'static str, String), Vec<String>>> {
        fn index_one<K: DiscussionParent>(
            index: &mut BTreeMap<(&'static str, String), Vec<String>>,
            word: &'static str,
            records: &[K],
        ) {
            for record in records {
                index
                    .entry((word, record.parent_id().as_str().to_owned()))
                    .or_default()
                    .push(record.parent_scope().as_str().to_owned());
            }
        }
        let mut index = BTreeMap::new();
        index_one(&mut index, "source", &self.list_sources(scope)?);
        index_one(&mut index, "requirement", &self.list_requirements(scope)?);
        index_one(&mut index, "resolution", &self.list_resolutions(scope)?);
        index_one(&mut index, "rule", &self.list_rules(scope)?);
        index_one(&mut index, "topic", &self.list_topics(scope)?);
        index_one(&mut index, "question", &self.list_questions(scope)?);
        Ok(index)
    }
}

impl StateStore {
    /// Reads the scope's Discussion records.
    pub fn list_discussions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Discussion>> {
        crate::state_store::readers::read_jsonl(
            self,
            &shards::discussions_path(&self.layout, scope),
        )
    }

    /// Reads the scope's Discussions and refuses one whose Thread, parent, or
    /// Message membership disagrees with the Thread and Message records.
    pub(crate) fn validated_discussions(&self, scope: &ScopeId) -> anyhow::Result<Vec<Discussion>> {
        let discussions = self.list_discussions(scope)?;
        let threads = self.list_threads(scope)?;
        let messages = self.list_messages(scope)?;
        // Index each shard once, so validation runs one pass over these maps
        // instead of one shard scan per Discussion.
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
        let parents_by_key = self.index_discussion_parents(scope)?;
        let mut ids = BTreeSet::new();
        let mut membership = BTreeSet::new();
        for discussion in &discussions {
            discussion.validate()?;
            anyhow::ensure!(discussion.scope_id == *scope, "Discussion scope mismatch");
            anyhow::ensure!(
                ids.insert(discussion.discussion_id.as_str()),
                "duplicate Discussion identity"
            );
            let thread = threads_by_key
                .get(&(discussion.thread_id.as_str(), scope.as_str()))
                .copied()
                .ok_or_else(|| anyhow::anyhow!("Discussion Thread is missing"))?;
            let Some(parent_kind) = discussion_kind_word(discussion.parent.node_type) else {
                anyhow::bail!("Discussion parent mismatch");
            };
            anyhow::ensure!(
                thread.parent == discussion.parent,
                "Discussion parent mismatch"
            );
            let in_scope = parents_by_key
                .get(&(parent_kind, discussion.parent.node_id.as_str().to_owned()))
                .is_some_and(|scopes| scopes.len() == 1 && scopes[0].as_str() == scope.as_str());
            anyhow::ensure!(
                in_scope,
                "Discussion parent does not exist uniquely in this scope"
            );
            for id in &discussion.message_ids {
                anyhow::ensure!(
                    membership.insert(id.as_str()),
                    "Message belongs to multiple Discussions"
                );
                let matches = messages_by_id.get(id.as_str()).is_some_and(|candidates| {
                    candidates.len() == 1
                        && candidates
                            .iter()
                            .any(|m| m.scope_id == *scope && m.thread_id == discussion.thread_id)
                });
                anyhow::ensure!(matches, "Discussion Message membership mismatch");
            }
        }
        Ok(discussions)
    }

    pub(super) fn validate_discussion_origin(
        &self,
        scope: &ScopeId,
        origin: &DiscussionOrigin,
    ) -> anyhow::Result<()> {
        let discussions = self.validated_discussions(scope)?;
        self.validate_discussion_origin_among(scope, origin, &discussions)
    }

    /// Checks one origin against already-read Discussions, so callers that
    /// validated the Discussion state once need not revalidate per origin.
    pub(super) fn validate_discussion_origin_among(
        &self,
        scope: &ScopeId,
        origin: &DiscussionOrigin,
        discussions: &[Discussion],
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            discussions
                .iter()
                .any(|d| d.discussion_id == origin.discussion_id
                    && d.thread_id == origin.thread_id
                    && d.message_ids.contains(&origin.message_id)),
            "outcome origin is not a Message in this Discussion and scope"
        );
        self.validate_requirement_origin(scope, Some(&origin.thread_id), Some(&origin.message_id))
    }

    /// Adds one record write to the Discussion of its origin, with the
    /// revision that the write gave the record.
    pub(super) fn add_discussion_outcome(
        &self,
        scope: &ScopeId,
        origin: &DiscussionOrigin,
        record_kind: NodeType,
        record_id: &StableId,
        revision: &StableId,
    ) -> anyhow::Result<()> {
        let outcome = DiscussionOutcome {
            message_id: origin.message_id.clone(),
            record_kind,
            record_id: record_id.clone(),
            revision: revision.clone(),
        };
        let path = shards::discussions_path(&self.layout, scope);
        super::guard::with_writer(&path, "*", || {
            self.mutate_jsonl_records(&path, |discussions: &mut Vec<Discussion>| {
                let discussion = discussions
                    .iter_mut()
                    .find(|d| d.discussion_id == origin.discussion_id)
                    .ok_or_else(|| anyhow::anyhow!("outcome origin Discussion is missing"))?;
                if !discussion.outcomes.contains(&outcome) {
                    discussion.outcomes.push(outcome);
                }
                Ok(())
            })
        })
    }

    pub(super) fn authorize_discussion(
        &self,
        input: &super::WriteDiscussion,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(!input.actor.trim().is_empty(), "invalid Discussion actor");
        crate::write_error::ensure!(
            ResourceNotFound,
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
            NodeType::Source | NodeType::Requirement | NodeType::Resolution | NodeType::Rule => {
                parent_owner_matches(owner.as_deref(), input.declared_by.as_deref())?;
            }
            NodeType::Domain | NodeType::Boundary => unreachable!(),
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

pub(super) const fn discussion_kind_word(kind: NodeType) -> Option<&'static str> {
    match kind {
        NodeType::Source => Some("source"),
        NodeType::Requirement => Some("requirement"),
        NodeType::Resolution => Some("resolution"),
        NodeType::Rule => Some("rule"),
        NodeType::Topic => Some("topic"),
        NodeType::Question => Some("question"),
        NodeType::Domain | NodeType::Boundary => None,
    }
}

pub(super) const fn parent_table(kind: NodeType) -> Option<&'static str> {
    match kind {
        NodeType::Source => Some("sources"),
        NodeType::Requirement => Some("requirements"),
        NodeType::Resolution => Some("resolutions"),
        NodeType::Rule => Some("rules"),
        NodeType::Topic => Some("topics"),
        NodeType::Question => Some("questions"),
        NodeType::Domain | NodeType::Boundary => None,
    }
}

impl StateStore {
    pub(super) fn resolve_discussion_parent(
        &self,
        scope: &ScopeId,
        parent: &ThreadParent,
    ) -> anyhow::Result<Option<String>> {
        crate::write_error::ensure!(
            UnsupportedThreadParent,
            discussion_kind_word(parent.node_type).is_some(),
            "thread parent kind does not take Discussions: {parent:?}"
        );
        match parent.node_type {
            NodeType::Source => {
                Self::resolve_parent_among(&self.list_sources(scope)?, scope, parent)
            }
            NodeType::Requirement => {
                Self::resolve_parent_among(&self.list_requirements(scope)?, scope, parent)
            }
            NodeType::Resolution => {
                Self::resolve_parent_among(&self.list_resolutions(scope)?, scope, parent)
            }
            NodeType::Rule => Self::resolve_parent_among(&self.list_rules(scope)?, scope, parent),
            NodeType::Topic => Self::resolve_parent_among(&self.list_topics(scope)?, scope, parent),
            NodeType::Question => {
                Self::resolve_parent_among(&self.list_questions(scope)?, scope, parent)
            }
            NodeType::Domain | NodeType::Boundary => anyhow::bail!(
                "thread parent kind `{}` does not take Discussions",
                discussion_kind_word(parent.node_type).unwrap_or("unsupported")
            ),
        }
    }

    fn resolve_parent_among<T: DiscussionParent>(
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
        let record = matches.first().ok_or_else(|| {
            crate::write_error::SourceFailure::wrap(
                crate::write_error::WriteFailure::ResourceNotFound,
                anyhow::anyhow!("Discussion parent does not exist in this scope"),
            )
        })?;
        anyhow::ensure!(
            record.parent_scope() == scope,
            "Discussion parent scope mismatch"
        );
        Ok(record.owner_field().map(str::to_owned))
    }
}
