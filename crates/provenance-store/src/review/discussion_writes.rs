use super::{guard, DiscussionAction, WriteDiscussion};
use crate::{
    canonical_digest,
    review::publication::with_record_state,
    shards,
    state_store::{PostMessageInput, StateStore},
};
use provenance_core::{
    threads::{Discussion, DiscussionStatus},
    Thread, ThreadStatus,
};
use provenance_macros::rule;

pub(super) fn check_size(input: &WriteDiscussion) -> anyhow::Result<()> {
    anyhow::ensure!(
        canonical_digest::canonical_bytes(input)?.len() <= 1_048_576,
        "Discussion request exceeds the operation byte budget"
    );
    Ok(())
}

impl StateStore {
    /// Creates a distinct Discussion root or changes one addressed Discussion.
    #[rule("rule_record_comments_have_separate_reply_threads")]
    pub fn write_discussion(&self, input: WriteDiscussion) -> anyhow::Result<Discussion> {
        self.with_repository_publication(|| self.write_discussion_in_publication(input, None))
    }

    pub(super) fn write_discussion_in_publication(
        &self,
        input: WriteDiscussion,
        resolved_head: Option<Discussion>,
    ) -> anyhow::Result<Discussion> {
        check_size(&input)?;
        self.authorize_discussion(&input)?;
        let head = match &input.action {
            DiscussionAction::Start { .. } => {
                self.validated_discussions(&input.scope_id)?;
                None
            }
            DiscussionAction::Reply {
                discussion_id,
                expected_version,
                ..
            }
            | DiscussionAction::SetStatus {
                discussion_id,
                expected_version,
                ..
            } => {
                let head = if let Some(head) = resolved_head {
                    head
                } else {
                    self.validated_discussions(&input.scope_id)?
                        .into_iter()
                        .find(|d| d.discussion_id == *discussion_id)
                        .ok_or_else(|| {
                            crate::write_error::SourceFailure::wrap(
                                crate::write_error::WriteFailure::ResourceNotFound,
                                anyhow::anyhow!("Discussion does not exist"),
                            )
                        })?
                };
                crate::write_error::ensure!(
                    ResourceNotFound,
                    head.discussion_id == *discussion_id,
                    "Discussion does not exist"
                );
                crate::write_error::ensure!(
                    DiscussionMembershipMismatch,
                    head.parent == input.parent,
                    "Discussion parent membership mismatch"
                );
                crate::write_error::ensure!(
                    DiscussionVersionConflict,
                    head.version == *expected_version,
                    "stale Discussion version"
                );
                let matching = self
                    .list_threads(&input.scope_id)?
                    .into_iter()
                    .filter(|t| t.parent == input.parent)
                    .collect::<Vec<_>>();
                let canonical = provenance_core::threads::choose_canonical_active_thread(&matching);
                crate::write_error::ensure!(
                    DiscussionClosed,
                    canonical.is_some_and(|t| t.id == head.thread_id),
                    "Discussion requires the canonical active Thread; closed containers refuse replies and reopening"
                );
                Some(head)
            }
        };
        match &input.action {
            DiscussionAction::Start { body, .. } | DiscussionAction::Reply { body, .. } => {
                crate::write_error::ensure!(
                    EmptyMessageBody,
                    !body.trim().is_empty(),
                    "message body must not be empty"
                );
                if let Some(head) = &head {
                    crate::write_error::ensure!(
                        DiscussionResolved,
                        head.status == DiscussionStatus::Active,
                        "resolved Discussion refuses replies"
                    );
                }
            }
            DiscussionAction::SetStatus { status, .. } => {
                anyhow::ensure!(
                    head.as_ref().unwrap().status != *status,
                    "Discussion already has this status"
                );
            }
        }
        with_record_state(&self.layout, |layout| {
            let staged = Self::new(layout.clone());
            guard::with_writer(&shards::threads_path(layout, &input.scope_id), "*", || {
                guard::with_writer(&shards::messages_path(layout, &input.scope_id), "*", || {
                    staged.commit_discussion(input, head, None)
                })
            })
        })
    }

    /// Changes one Discussion without changing siblings or its Thread status.
    ///
    /// Decision-cycle feedback reuses this under its own staged publication so
    /// the disposition and the feedback Message publish together or not at all.
    #[rule("rule_reply_threads_resolve_independently")]
    pub(super) fn commit_discussion(
        &self,
        input: WriteDiscussion,
        head: Option<Discussion>,
        feedback_for: Option<provenance_core::StableId>,
    ) -> anyhow::Result<Discussion> {
        let scope = &input.scope_id;
        let (thread, message, status) = match input.action {
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
                        (thread, Some(message), DiscussionStatus::Active)
                    }
                    DiscussionAction::SetStatus { status, .. } => (thread, None, status),
                    DiscussionAction::Start { .. } => unreachable!(),
                }
            }
        };
        let status_only = message.is_none();
        self.validate_discussion_thread(scope, &input.parent, &thread, status_only)?;
        let discussion = if let Some(mut discussion) = head {
            discussion.version += 1;
            discussion.status = status;
            discussion.message_ids.extend(message.map(|m| m.id));
            discussion
        } else {
            let root = message.expect("a started Discussion has a root Message").id;
            Discussion {
                scope_id: scope.clone(),
                discussion_id: super::new_id(),
                parent: input.parent,
                thread_id: thread.id,
                root_message_id: root.clone(),
                message_ids: vec![root],
                status,
                version: 1,
                actor: input.actor,
                outcomes: Vec::new(),
                disposition_id: feedback_for,
            }
        };
        let path = shards::discussions_path(&self.layout, scope);
        guard::with_writer(&path, "*", || {
            self.mutate_jsonl_records(&path, |discussions: &mut Vec<Discussion>| {
                match discussions
                    .iter_mut()
                    .find(|d| d.discussion_id == discussion.discussion_id)
                {
                    Some(current) => *current = discussion.clone(),
                    None => discussions.push(discussion.clone()),
                }
                Ok(())
            })
        })?;
        Ok(discussion)
    }

    fn validate_discussion_thread(
        &self,
        scope: &provenance_core::ScopeId,
        parent: &provenance_core::ThreadParent,
        thread: &Thread,
        status_only: bool,
    ) -> anyhow::Result<()> {
        self.mutate_jsonl_records(
            &shards::threads_path(&self.layout, scope),
            |threads: &mut Vec<Thread>| {
                if !status_only {
                    provenance_core::threads::archive_non_canonical_siblings(
                        threads, parent, &thread.id,
                    );
                }
                let current = threads.iter_mut().find(|t| t.id == thread.id).unwrap();
                anyhow::ensure!(
                    current.status == ThreadStatus::Active,
                    "Discussion Thread closed during publication"
                );
                Ok(())
            },
        )?;

        Ok(())
    }
}
