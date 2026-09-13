use super::{DiscussionEntry, DiscussionFact, DiscussionStatus};

impl DiscussionEntry {
    /// Checks one immutable fact against its immediate predecessor.
    pub fn validate_after(&self, previous: Option<&Self>) -> anyhow::Result<()> {
        let Some(previous) = previous else {
            anyhow::ensure!(
                self.version == 1
                    && self.predecessor.is_none()
                    && self.fact == DiscussionFact::Started
                    && self.status == DiscussionStatus::Active
                    && self.message_id.as_ref() == Some(&self.root_message_id),
                "Discussion has no active root Message"
            );
            return Ok(());
        };
        anyhow::ensure!(
            previous.version.checked_add(1) == Some(self.version)
                && self.predecessor.as_ref() == Some(&previous.id),
            "Discussion conflict: missing or competing versions"
        );
        anyhow::ensure!(
            self.scope_id == previous.scope_id
                && self.discussion_id == previous.discussion_id
                && self.thread_id == previous.thread_id
                && self.parent == previous.parent
                && self.root_message_id == previous.root_message_id,
            "Discussion origin changed"
        );
        match self.fact {
            DiscussionFact::Replied => anyhow::ensure!(
                previous.status == DiscussionStatus::Active
                    && self.status == previous.status
                    && self.message_id.is_some(),
                "invalid Discussion reply"
            ),
            DiscussionFact::StatusChanged => anyhow::ensure!(
                self.status != previous.status && self.message_id.is_none(),
                "invalid Discussion status transition"
            ),
            DiscussionFact::Started => anyhow::bail!("Discussion has more than one root"),
        }
        Ok(())
    }
}
