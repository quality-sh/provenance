//! Validation and canonical ordering for the artifact links a shaping record
//! carries.

use provenance_core::{ArtifactLink, ArtifactLinkTargetType, ScopeId};

use super::StateStore;

impl StateStore {
    pub(in crate::state_store) fn validate_artifact_links(
        &self,
        scope_id: &ScopeId,
        links: &[ArtifactLink],
    ) -> anyhow::Result<()> {
        for link in links {
            let exists = match link.target_type {
                ArtifactLinkTargetType::Source => self
                    .list_sources(scope_id)?
                    .iter()
                    .any(|source| source.id == link.target_id),
                ArtifactLinkTargetType::Requirement => self
                    .list_requirements(scope_id)?
                    .iter()
                    .any(|requirement| requirement.id == link.target_id),
                ArtifactLinkTargetType::Resolution => self
                    .list_resolutions(scope_id)?
                    .iter()
                    .any(|resolution| resolution.id == link.target_id),
                ArtifactLinkTargetType::Rule => self
                    .list_rules(scope_id)?
                    .iter()
                    .any(|rule| rule.id == link.target_id),
            };
            anyhow::ensure!(exists, "linked artifact does not exist");
        }
        Ok(())
    }
}

pub(in crate::state_store) fn sort_artifact_links(links: &mut Vec<ArtifactLink>) {
    links.sort_by(|a, b| {
        artifact_link_target_key(a.target_type)
            .cmp(artifact_link_target_key(b.target_type))
            .then(a.target_id.as_str().cmp(b.target_id.as_str()))
    });
    links.dedup();
}

const fn artifact_link_target_key(target_type: ArtifactLinkTargetType) -> &'static str {
    match target_type {
        ArtifactLinkTargetType::Source => "source",
        ArtifactLinkTargetType::Requirement => "requirement",
        ArtifactLinkTargetType::Resolution => "resolution",
        ArtifactLinkTargetType::Rule => "rule",
    }
}
