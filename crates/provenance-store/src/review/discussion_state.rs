use crate::refusal::state_fault;
use crate::{
    shards,
    state_store::{message_ids::ShardLine, StateStore},
};
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
        let discussions: Vec<(Discussion, ShardLine)> =
            crate::state_store::readers::read_jsonl_located(
                self,
                &shards::discussions_path(&self.layout, scope),
            )?;
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
        for (discussion, at) in &discussions {
            discussion.validate()?;
            let id = discussion.discussion_id.as_str();
            if discussion.scope_id != *scope {
                return Err(state_fault(
                    at,
                    format!(
                        "the discussion names scope {}, not the scope of its file",
                        discussion.scope_id.as_str()
                    ),
                ));
            }
            if !ids.insert(id) {
                return Err(state_fault(
                    at,
                    format!("the discussion id {id} is stored again"),
                ));
            }
            let thread_id = discussion.thread_id.as_str();
            let Some(thread) = threads_by_key.get(&(thread_id, scope.as_str())).copied() else {
                return Err(state_fault(
                    at,
                    format!("the discussion names thread {thread_id}, which does not exist"),
                ));
            };
            let parent_kind = discussion_kind_word(discussion.parent.node_type);
            let parent = discussion.parent.node_id.as_str();
            if thread.parent != discussion.parent {
                return Err(state_fault(
                    at,
                    format!(
                        "the discussion names parent {parent}; its thread {thread_id} names parent {}",
                        thread.parent.node_id.as_str()
                    ),
                ));
            }
            let in_scope = parents_by_key
                .get(&(parent_kind, parent.to_owned()))
                .is_some_and(|scopes| scopes.len() == 1 && scopes[0].as_str() == scope.as_str());
            if !in_scope {
                return Err(state_fault(
                    at,
                    format!(
                        "the discussion names parent {parent}, which does not exist one time in this scope"
                    ),
                ));
            }
            for message in &discussion.message_ids {
                let message = message.as_str();
                if !membership.insert(message) {
                    return Err(state_fault(
                        at,
                        format!("the discussion names message {message}, which another discussion names"),
                    ));
                }
                let matches = messages_by_id.get(message).is_some_and(|candidates| {
                    candidates.len() == 1
                        && candidates
                            .iter()
                            .any(|m| m.scope_id == *scope && m.thread_id == discussion.thread_id)
                });
                if !matches {
                    return Err(state_fault(
                        at,
                        format!(
                            "Discussion Message membership mismatch: the discussion names message {message}, which is not one message of thread {thread_id}"
                        ),
                    ));
                }
            }
        }
        Ok(discussions
            .into_iter()
            .map(|(discussion, _)| discussion)
            .collect())
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
        crate::write_error::ensure!(
            InvalidUpdate,
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
        self.permitting(&[&path], "*", |store| {
            store.mutate_jsonl_records(&path, |discussions: &mut Vec<Discussion>| {
                let discussion = discussions
                    .iter_mut()
                    .find(|d| d.discussion_id == origin.discussion_id)
                    .ok_or_else(|| {
                        crate::refusal::fault!("outcome origin Discussion is missing")
                    })?;
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
        crate::write_error::ensure_actor(&input.actor)?;
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
            NodeType::Topic | NodeType::Question => crate::write_error::ensure!(
                InvalidUpdate,
                input.declared_by.is_none(),
                "a topic or question parent takes no declared owner"
            ),
            NodeType::Source | NodeType::Requirement | NodeType::Resolution | NodeType::Rule => {
                parent_owner_matches(owner.as_deref(), input.declared_by.as_deref())?;
            }
        }
        let mut ids = BTreeSet::new();
        let threads: Vec<(Thread, ShardLine)> = crate::state_store::readers::read_jsonl_located(
            self,
            &shards::threads_path(&self.layout, &input.scope_id),
        )?;
        for (thread, at) in threads {
            if !ids.insert(thread.id.as_str().to_owned()) {
                return Err(state_fault(
                    &at,
                    format!("thread {} is stored again", thread.id.as_str()),
                ));
            }
            if thread.parent == input.parent && thread.scope_id != input.scope_id {
                return Err(state_fault(
                    &at,
                    format!(
                        "thread {} names scope {}, not the scope of its file",
                        thread.id.as_str(),
                        thread.scope_id.as_str()
                    ),
                ));
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
        return Err(crate::refusal::refuse!(
            (crate::write_error::WriteFailure::record_ownership_conflict(saved)),
            "declared_by must match the existing owner; omit it for a manual record"
        ));
    }
    Ok(())
}

pub(super) const fn discussion_kind_word(kind: NodeType) -> &'static str {
    match kind {
        NodeType::Source => "source",
        NodeType::Requirement => "requirement",
        NodeType::Resolution => "resolution",
        NodeType::Rule => "rule",
        NodeType::Topic => "topic",
        NodeType::Question => "question",
    }
}

#[cfg_attr(not(feature = "host"), allow(dead_code))]
pub(super) const fn parent_table(kind: NodeType) -> &'static str {
    match kind {
        NodeType::Source => "sources",
        NodeType::Requirement => "requirements",
        NodeType::Resolution => "resolutions",
        NodeType::Rule => "rules",
        NodeType::Topic => "topics",
        NodeType::Question => "questions",
    }
}

impl StateStore {
    pub(super) fn resolve_discussion_parent(
        &self,
        scope: &ScopeId,
        parent: &ThreadParent,
    ) -> anyhow::Result<Option<String>> {
        match parent.node_type {
            NodeType::Source => self.resolve_parent_among(
                &self.list_sources(scope)?,
                &shards::sources_path(&self.layout, scope),
                scope,
                parent,
            ),
            NodeType::Requirement => self.resolve_parent_among(
                &self.list_requirements(scope)?,
                &shards::requirements_path(&self.layout, scope),
                scope,
                parent,
            ),
            NodeType::Resolution => self.resolve_parent_among(
                &self.list_resolutions(scope)?,
                &shards::resolutions_path(&self.layout, scope),
                scope,
                parent,
            ),
            NodeType::Rule => self.resolve_parent_among(
                &self.list_rules(scope)?,
                &shards::rules_path(&self.layout, scope),
                scope,
                parent,
            ),
            NodeType::Topic => self.resolve_parent_among(
                &self.list_topics(scope)?,
                &shards::topics_path(&self.layout, scope),
                scope,
                parent,
            ),
            NodeType::Question => self.resolve_parent_among(
                &self.list_questions(scope)?,
                &shards::questions_path(&self.layout, scope),
                scope,
                parent,
            ),
        }
    }

    fn resolve_parent_among<T: DiscussionParent>(
        &self,
        records: &[T],
        path: &camino::Utf8Path,
        scope: &ScopeId,
        parent: &ThreadParent,
    ) -> anyhow::Result<Option<String>> {
        let id = parent.node_id.as_str();
        let matches = records
            .iter()
            .filter(|record| record.parent_id() == &parent.node_id)
            .collect::<Vec<_>>();
        if matches.len() > 1 {
            return Err(state_fault(
                &self.parent_line(path, id, 1)?,
                format!("Discussion parent {id} is stored again"),
            ));
        }
        let record = matches.first().ok_or_else(|| {
            crate::refusal::refuse!(
                ResourceNotFound,
                "Discussion parent does not exist in this scope"
            )
        })?;
        if record.parent_scope() != scope {
            return Err(state_fault(
                &self.parent_line(path, id, 0)?,
                format!(
                    "Discussion parent {id} names scope {}, not the scope of its file",
                    record.parent_scope().as_str()
                ),
            ));
        }
        Ok(record.owner_field().map(str::to_owned))
    }

    /// The line of the record after `skip` earlier records with the id `id`.
    fn parent_line(
        &self,
        path: &camino::Utf8Path,
        id: &str,
        skip: usize,
    ) -> anyhow::Result<ShardLine> {
        crate::state_store::readers::read_jsonl_located::<serde_json::Value>(self, path)?
            .into_iter()
            .filter(|(record, _)| record["id"] == id)
            .nth(skip)
            .map(|(_, at)| at)
            .ok_or_else(|| crate::refusal::fault!("record {id} has no line in its file"))
    }
}
