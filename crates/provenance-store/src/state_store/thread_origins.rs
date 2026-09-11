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
        let (thread, message) = match (thread, message) {
            (None, None) => return Ok(()),
            (Some(thread), Some(message)) => (thread, message),
            _ => anyhow::bail!("Requirement origin needs both Thread and Message"),
        };
        let threads = self.list_threads(scope)?;
        let container = threads
            .iter()
            .find(|t| t.id == *thread && t.scope_id == *scope)
            .ok_or_else(|| anyhow::anyhow!("origin Thread does not exist in this scope"))?;
        self.ensure_node_exists(
            scope,
            container.parent.node_type,
            &container.parent.node_id,
            "origin parent",
        )?;
        anyhow::ensure!(
            self.list_messages(scope)?
                .iter()
                .any(|m| m.id == *message && m.thread_id == *thread && m.scope_id == *scope),
            "origin Message does not belong to this Thread and scope"
        );
        Ok(())
    }
}
