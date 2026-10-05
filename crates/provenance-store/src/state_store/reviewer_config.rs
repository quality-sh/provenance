use super::StateStore;
use crate::publication::with_staged_state;

impl StateStore {
    /// Sets the repository actors who can record review decisions.
    pub fn set_disposition_actor_ids(&self, actor_ids: Vec<String>) -> anyhow::Result<()> {
        anyhow::ensure!(
            actor_ids.iter().all(|id| !id.trim().is_empty()),
            "disposition actor IDs must not be empty"
        );
        self.with_repository_publication(|| {
            let mut manifest = self.manifest()?;
            manifest.disposition_actor_ids = actor_ids;
            with_staged_state(&self.layout, false, |layout| {
                std::fs::write(
                    layout.manifest_path(),
                    format!("{}\n", serde_json::to_string_pretty(&manifest)?),
                )?;
                Ok(())
            })
        })
    }
}
