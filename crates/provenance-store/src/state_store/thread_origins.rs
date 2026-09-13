use super::StateStore;
use provenance_core::{ScopeId, StableId};

impl StateStore {
    pub(crate) fn origin_requires_review(
        &self,
        scope: &ScopeId,
        message: Option<&StableId>,
    ) -> anyhow::Result<bool> {
        let Some(id) = message else {
            return Ok(false);
        };
        Ok(self.list_messages(scope)?.iter().any(|m| {
            m.id == *id && m.schema_version == provenance_core::review::REVIEW_SCHEMA_VERSION
        }))
    }

    pub(crate) fn validate_requirement_origin(
        &self,
        scope: &ScopeId,
        thread: Option<&StableId>,
        message: Option<&StableId>,
    ) -> anyhow::Result<()> {
        if thread.is_none() && message.is_none() {
            return Ok(());
        }
        let messages = self.list_messages(scope)?;
        let origin_message = message
            .map(|id| {
                let mut matches = messages.iter().filter(|m| m.id == *id);
                let record = matches
                    .next()
                    .filter(|m| m.scope_id == *scope)
                    .ok_or_else(|| {
                        anyhow::anyhow!("origin Message does not exist in this scope")
                    })?;
                anyhow::ensure!(
                    matches.next().is_none(),
                    "origin Message identity is not unique"
                );
                Ok::<_, anyhow::Error>(record)
            })
            .transpose()?;
        let thread = thread
            .or_else(|| origin_message.map(|m| &m.thread_id))
            .expect("an origin supplies a Thread or Message");
        let threads = self.list_threads(scope)?;
        let mut matches = threads.iter().filter(|t| t.id == *thread);
        let container = matches
            .next()
            .filter(|t| t.scope_id == *scope)
            .ok_or_else(|| anyhow::anyhow!("origin Thread does not exist in this scope"))?;
        anyhow::ensure!(
            matches.next().is_none(),
            "origin Thread identity is not unique"
        );
        self.ensure_node_exists(
            scope,
            container.parent.node_type,
            &container.parent.node_id,
            "origin parent",
        )?;
        if let Some(message) = origin_message {
            anyhow::ensure!(
                message.thread_id == *thread,
                "origin Message does not belong to this Thread and scope"
            );
        } else {
            anyhow::ensure!(
                container.schema_version != provenance_core::review::REVIEW_SCHEMA_VERSION,
                "an enrolled origin Thread requires a Discussion and Message outcome"
            );
        }
        Ok(())
    }
}
