use super::{serde_name, PostMessageInput, PostMessageResult, StateStore};
use crate::shards;
use crate::write_error::{publication_started, SourceFailure, WriteFailure};
use provenance_core::{
    Message, NodeType, StableId, Thread, ThreadStatus, SUPPORTED_SCHEMA_VERSION,
};

impl StateStore {
    pub fn post_thread_message(
        &self,
        input: PostMessageInput,
    ) -> anyhow::Result<PostMessageResult> {
        self.with_repository_publication(|| {
            anyhow::ensure!(
                !self
                    .list_threads(&input.scope_id)?
                    .iter()
                    .any(|t| t.parent == input.parent
                        && t.schema_version == provenance_core::review::REVIEW_SCHEMA_VERSION),
                "enrolled Thread requires an addressed Discussion write"
            );
            self.write_thread_message(input)
        })
    }

    pub(crate) fn write_thread_message(
        &self,
        input: PostMessageInput,
    ) -> anyhow::Result<PostMessageResult> {
        let PostMessageInput {
            scope_id,
            parent,
            role,
            body,
        } = input;
        crate::write_error::ensure!(
            EmptyMessageBody,
            !body.trim().is_empty(),
            "message body must not be empty"
        );
        match parent.node_type {
            NodeType::Source
            | NodeType::Requirement
            | NodeType::Resolution
            | NodeType::Rule
            | NodeType::Topic
            | NodeType::Question => {}
            NodeType::Domain | NodeType::Boundary => {
                return Err(SourceFailure::wrap(
                    WriteFailure::UnsupportedThreadParent,
                    anyhow::anyhow!(
                        "thread parent kind `{}` is not supported; threads attach to a source, \
                 requirement, resolution, rule, topic, or question",
                        serde_name(&parent.node_type)?
                    ),
                ))
            }
        }
        let threads_path = shards::threads_path(&self.layout, &scope_id);
        let thread = self.mutate_jsonl_records(&threads_path, |threads: &mut Vec<Thread>| {
            let matching: Vec<_> = threads
                .iter()
                .filter(|thread| thread.parent == parent)
                .cloned()
                .collect();
            let thread = if let Some(canonical) =
                provenance_core::threads::choose_canonical_active_thread(&matching)
            {
                let canonical = canonical.clone();
                provenance_core::threads::archive_non_canonical_siblings(
                    threads,
                    &parent,
                    &canonical.id,
                );
                canonical
            } else {
                let base_id = format!(
                    "thread_{}_{}",
                    serde_name(&parent.node_type)?,
                    parent.node_id.as_str()
                );
                let thread = Thread {
                    schema_version: SUPPORTED_SCHEMA_VERSION,
                    scope_id: scope_id.clone(),
                    id: next_thread_id(threads, &base_id)?,
                    parent: parent.clone(),
                    status: ThreadStatus::Active,
                    created_at: 1,
                };
                threads.push(thread.clone());
                thread
            };
            threads.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
            Ok(thread)
        })?;

        let message = self.append_discussion_message(&scope_id, &thread.id, role, body)?;

        Ok(PostMessageResult { thread, message })
    }
    pub(crate) fn append_discussion_message(
        &self,
        scope_id: &provenance_core::ScopeId,
        thread_id: &StableId,
        role: provenance_core::MessageRole,
        body: String,
    ) -> anyhow::Result<Message> {
        let existing = self.list_messages(scope_id)?;
        let created_at = existing
            .iter()
            .map(|m| m.created_at)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Message timestamp overflow"))?;
        let id = StableId::new(format!("msg_{created_at:06}"))?;
        anyhow::ensure!(
            !existing.iter().any(|m| m.id == id),
            "Message identity already exists in a legacy shard"
        );
        let messages_path = shards::messages_path(&self.layout, scope_id);
        self.mutate_jsonl_records(&messages_path, |messages: &mut Vec<Message>| {
            let message = Message {
                schema_version: SUPPORTED_SCHEMA_VERSION,
                scope_id: scope_id.clone(),
                id,
                thread_id: thread_id.clone(),
                role,
                body,
                created_at,
                ai_metadata: None,
            };
            messages.push(message.clone());
            messages.sort_by(|a, b| {
                a.created_at
                    .cmp(&b.created_at)
                    .then(a.id.as_str().cmp(b.id.as_str()))
            });
            Ok(message)
        })
        .map_err(publication_started)
    }
}

fn next_thread_id(threads: &[Thread], base_id: &str) -> anyhow::Result<StableId> {
    let mut candidate = base_id.to_string();
    let mut suffix = 2_u64;
    while threads.iter().any(|thread| thread.id.as_str() == candidate) {
        candidate = format!("{base_id}_{suffix}");
        suffix = suffix
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("thread id suffix overflow for {base_id}"))?;
    }
    StableId::new(candidate)
}
