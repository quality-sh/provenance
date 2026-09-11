use super::{guard, journal, DiscussionAction, WriteDiscussion};
use crate::{
    canonical_digest,
    publication::with_staged_state,
    shards,
    state_store::{PostMessageInput, StateStore},
};
use provenance_core::{
    review::REVIEW_SCHEMA_VERSION,
    threads::{DiscussionEntry, DiscussionFact, DiscussionStatus},
    Message, Thread, ThreadStatus,
};
use provenance_macros::rule;

pub(super) fn intent(input: &WriteDiscussion) -> anyhow::Result<String> {
    let bytes = canonical_digest::canonical_bytes(input)?;
    anyhow::ensure!(
        bytes.len() <= 1_048_576,
        "Discussion request exceeds the operation byte budget"
    );
    Ok(canonical_digest::digest(&bytes))
}

impl StateStore {
    /// Creates a distinct Discussion root or changes one addressed Discussion.
    #[rule("rule_record_comments_have_separate_reply_threads")]
    pub fn write_discussion(&self, input: WriteDiscussion) -> anyhow::Result<DiscussionEntry> {
        let digest = intent(&input)?;
        self.with_repository_publication(|| {
            self.authorize_discussion(&input)?;
            if let Some(receipt) = self.discussion_receipt(&input)? { return Ok(receipt); }
            let heads = self.discussion_heads(&input.scope_id)?;
            let head = match &input.action {
                DiscussionAction::Start { .. } => None,
                DiscussionAction::Reply { discussion_id, expected_version, .. }
                | DiscussionAction::SetStatus { discussion_id, expected_version, .. } => {
                    let head = heads.into_iter().find(|e| e.discussion_id == *discussion_id)
                        .ok_or_else(|| anyhow::anyhow!("Discussion does not exist"))?;
                    anyhow::ensure!(head.parent == input.parent, "Discussion parent membership mismatch");
                    anyhow::ensure!(head.version == *expected_version, "stale Discussion version");
                    let matching = self.list_threads(&input.scope_id)?.into_iter().filter(|t| t.parent == input.parent).collect::<Vec<_>>();
                    let canonical = provenance_core::threads::choose_canonical_active_thread(&matching);
                    anyhow::ensure!(canonical.is_some_and(|t| t.id == head.thread_id),
                        "Discussion requires the canonical active Thread; closed containers refuse replies and reopening");
                    Some(head)
                }
            };
            match &input.action {
                DiscussionAction::Start { body, .. } | DiscussionAction::Reply { body, .. } => {
                    anyhow::ensure!(!body.trim().is_empty(), "message body must not be empty");
                    if let Some(head) = &head {
                        anyhow::ensure!(head.status == DiscussionStatus::Active, "resolved Discussion refuses replies");
                    }
                }
                DiscussionAction::SetStatus { status, .. } => {
                    anyhow::ensure!(head.as_ref().unwrap().status != *status, "Discussion already has this status");
                }
            }
            with_staged_state(&self.layout, false, |layout| {
                let staged = Self::new(layout.clone());
                guard::with_writer(&shards::threads_path(layout, &input.scope_id), "*", || {
                    guard::with_writer(&shards::messages_path(layout, &input.scope_id), "*", || {
                        staged.commit_discussion(input, head, digest)
                    })
                })
            })
        })
    }

    /// Changes one Discussion without changing siblings or its Thread status.
    #[rule("rule_reply_threads_resolve_independently")]
    fn commit_discussion(
        &self,
        input: WriteDiscussion,
        head: Option<DiscussionEntry>,
        digest: String,
    ) -> anyhow::Result<DiscussionEntry> {
        let scope = &input.scope_id;
        let (thread, message, status, fact) = match input.action {
            DiscussionAction::Start { role, body } => {
                let result = self.write_thread_message(PostMessageInput {
                    scope_id: scope.clone(),
                    parent: input.parent.clone(),
                    role,
                    body,
                })?;
                (
                    result.thread,
                    Some(result.message),
                    DiscussionStatus::Active,
                    DiscussionFact::Started,
                )
            }
            action => {
                let head = head.as_ref().unwrap();
                let thread = self
                    .list_threads(scope)?
                    .into_iter()
                    .find(|t| t.id == head.thread_id)
                    .unwrap();
                match action {
                    DiscussionAction::Reply { role, body, .. } => {
                        let message =
                            self.append_discussion_message(scope, &thread.id, role, body)?;
                        (
                            thread,
                            Some(message),
                            DiscussionStatus::Active,
                            DiscussionFact::Replied,
                        )
                    }
                    DiscussionAction::SetStatus { status, .. } => {
                        (thread, None, status, DiscussionFact::StatusChanged)
                    }
                    DiscussionAction::Start { .. } => unreachable!(),
                }
            }
        };
        self.enroll_discussion_records(scope, &input.parent, &thread, message.as_ref(), fact)?;
        let entry = DiscussionEntry {
            schema_version: REVIEW_SCHEMA_VERSION,
            scope_id: scope.clone(),
            id: journal::new_id(),
            parent: input.parent,
            thread_id: thread.id,
            discussion_id: head
                .as_ref()
                .map_or_else(journal::new_id, |e| e.discussion_id.clone()),
            root_message_id: head.as_ref().map_or_else(
                || message.as_ref().unwrap().id.clone(),
                |e| e.root_message_id.clone(),
            ),
            version: head.as_ref().map_or(1, |e| e.version + 1),
            predecessor: head.map(|e| e.id),
            status,
            fact,
            message_id: message.map(|m| m.id),
            actor: input.actor,
            request_id: input.request_id,
            intent_digest: digest,
        };
        anyhow::ensure!(
            serde_json::to_vec(&entry)?.len() as u64 <= journal::ENTRY_BYTES,
            "Discussion receipt exceeds the entry byte budget"
        );
        journal::write_new(
            &journal::entry_path(&self.layout, scope, &entry.request_id),
            &entry,
        )?;
        Ok(entry)
    }

    fn enroll_discussion_records(
        &self,
        scope: &provenance_core::ScopeId,
        parent: &provenance_core::ThreadParent,
        thread: &Thread,
        message: Option<&Message>,
        fact: DiscussionFact,
    ) -> anyhow::Result<()> {
        self.mutate_jsonl_records(
            &shards::threads_path(&self.layout, scope),
            |threads: &mut Vec<Thread>| {
                if fact != DiscussionFact::StatusChanged {
                    provenance_core::threads::archive_non_canonical_siblings(
                        threads, parent, &thread.id,
                    );
                }
                let current = threads.iter_mut().find(|t| t.id == thread.id).unwrap();
                anyhow::ensure!(
                    current.status == ThreadStatus::Active,
                    "Discussion Thread closed during publication"
                );
                current.schema_version = REVIEW_SCHEMA_VERSION;
                Ok(())
            },
        )?;
        if let Some(message) = message {
            let path = shards::messages_path(&self.layout, scope);
            guard::with_writer(&path, "*", || {
                self.mutate_jsonl_records(&path, |messages: &mut Vec<Message>| {
                    messages
                        .iter_mut()
                        .find(|m| m.id == message.id)
                        .unwrap()
                        .schema_version = REVIEW_SCHEMA_VERSION;
                    Ok(())
                })
            })?;
        }
        self.enroll_review_manifest()?;
        Ok(())
    }

    pub(super) fn enroll_review_manifest(&self) -> anyhow::Result<()> {
        let mut manifest = self.manifest()?;
        manifest.schema_version = REVIEW_SCHEMA_VERSION;
        std::fs::write(
            self.layout.manifest_path(),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        Ok(())
    }
}
