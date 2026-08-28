use provenance_core::{
    ArtifactLinkTargetType, IdeationTarget, IdeationTargetType, PromotionState, ProposalCard,
    ScopeId, Topic,
};
use provenance_macros::rule;
use serde::Serialize;
use std::path::{Component, Path};

use super::StateStore;

#[derive(Debug, Clone)]
pub struct ProposalDemand {
    changed_paths: Vec<String>,
    targets: Vec<IdeationTarget>,
}

impl ProposalDemand {
    pub fn for_changed_paths<I, S>(paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::new(paths.into_iter().map(Into::into).collect(), Vec::new())
    }

    pub fn for_target(target: IdeationTarget) -> Self {
        Self::new(Vec::new(), vec![target])
    }

    pub(crate) fn for_topic<I, S>(topic: &Topic, paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut targets = vec![
            IdeationTarget {
                artifact_type: IdeationTargetType::Topic,
                artifact_id: topic.id.clone(),
            },
            IdeationTarget {
                artifact_type: IdeationTargetType::Requirement,
                artifact_id: topic.requirement_id.clone(),
            },
        ];
        targets.extend(topic.links.iter().map(|link| IdeationTarget {
            artifact_type: match link.target_type {
                ArtifactLinkTargetType::Source => IdeationTargetType::Source,
                ArtifactLinkTargetType::Requirement => IdeationTargetType::Requirement,
                ArtifactLinkTargetType::Resolution => IdeationTargetType::Resolution,
                ArtifactLinkTargetType::Rule => IdeationTargetType::Rule,
            },
            artifact_id: link.target_id.clone(),
        }));
        Self::new(paths.into_iter().map(Into::into).collect(), targets)
    }

    pub fn new(mut changed_paths: Vec<String>, mut targets: Vec<IdeationTarget>) -> Self {
        changed_paths = changed_paths
            .into_iter()
            .map(|path| normalize_repo_path(&path).into_string())
            .collect();
        changed_paths.sort();
        changed_paths.dedup();
        sort_targets(&mut targets);
        Self {
            changed_paths,
            targets,
        }
    }

    pub(crate) fn extend_targets<I>(&mut self, targets: I)
    where
        I: IntoIterator<Item = IdeationTarget>,
    {
        self.targets.extend(targets);
        sort_targets(&mut self.targets);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "trigger", rename_all = "snake_case")]
pub enum ProposalSurfaceReason {
    EvidenceSite { path: String },
    Territory { target: IdeationTarget },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurfacedProposal {
    pub proposal: ProposalCard,
    pub reasons: Vec<ProposalSurfaceReason>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopicClaim {
    #[serde(flatten)]
    pub topic: Topic,
    pub surfaced_proposals: Vec<SurfacedProposal>,
}

impl StateStore {
    pub(crate) fn topic_structural_territory(
        &self,
        scope: &ScopeId,
        topic: &Topic,
    ) -> anyhow::Result<Vec<IdeationTarget>> {
        let mut targets = self
            .list_questions(scope)?
            .into_iter()
            .filter(|question| question.topic_id == topic.id)
            .map(|question| IdeationTarget {
                artifact_type: IdeationTargetType::Question,
                artifact_id: question.id,
            })
            .collect::<Vec<_>>();
        if let Some(domain_id) = self
            .list_requirements(scope)?
            .into_iter()
            .find(|requirement| requirement.id == topic.requirement_id)
            .and_then(|requirement| requirement.domain_id)
        {
            targets.push(IdeationTarget {
                artifact_type: IdeationTargetType::Domain,
                artifact_id: domain_id,
            });
        }
        Ok(targets)
    }

    pub fn surface_proposals(
        &self,
        scope: &ScopeId,
        demand: &ProposalDemand,
    ) -> anyhow::Result<Vec<SurfacedProposal>> {
        anyhow::ensure!(
            !demand.changed_paths.is_empty() || !demand.targets.is_empty(),
            "proposal demand must include at least one changed path or territory target"
        );

        Ok(self
            .list_proposal_cards(scope)?
            .into_iter()
            .filter_map(|proposal| {
                let reasons = surfacing_reasons(&proposal, demand);
                (!reasons.is_empty()).then_some(SurfacedProposal { proposal, reasons })
            })
            .collect())
    }
}

/// An undisposed proposal surfaces exactly when changed work intersects a cited
/// file or directory, or the demanded territory equals its typed target.
#[rule("rule_proposal_surfacing")]
pub(super) fn surfacing_reasons(
    proposal: &ProposalCard,
    demand: &ProposalDemand,
) -> Vec<ProposalSurfaceReason> {
    if !matches!(
        proposal.promotion_state,
        PromotionState::Proposed | PromotionState::Asserted
    ) {
        return Vec::new();
    }
    let mut reasons = Vec::new();
    for path in &demand.changed_paths {
        if proposal
            .traceability
            .evidence_references
            .iter()
            .filter_map(|reference| reference.file_path.as_deref())
            .any(|cited| evidence_path_matches(cited, path))
        {
            reasons.push(ProposalSurfaceReason::EvidenceSite { path: path.clone() });
        }
    }
    for target in &demand.targets {
        if proposal.traceability.target == *target {
            reasons.push(ProposalSurfaceReason::Territory {
                target: target.clone(),
            });
        }
    }
    reasons
}

/// Evidence paths are repository-relative lexical coordinates, not filesystem
/// lookups. `.` and `..` components are normalized without requiring either
/// path to exist, and a citation matches both itself and changed descendants.
fn evidence_path_matches(cited: &str, changed: &str) -> bool {
    let cited = normalize_repo_path(cited);
    let changed = Path::new(changed);
    changed == cited.as_std_path() || changed.starts_with(cited.as_std_path())
}

fn normalize_repo_path(path: &str) -> camino::Utf8PathBuf {
    let mut parts = Vec::new();
    for component in Path::new(path).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if parts.last().is_some_and(|part| part != "..") {
                    parts.pop();
                } else {
                    parts.push("..".to_owned());
                }
            }
            Component::Normal(value) => parts.push(value.to_string_lossy().into_owned()),
            Component::RootDir => parts.push("/".to_owned()),
            Component::Prefix(value) => {
                parts.push(value.as_os_str().to_string_lossy().into_owned());
            }
        }
    }
    let mut normalized = camino::Utf8PathBuf::new();
    for part in parts {
        normalized.push(part);
    }
    normalized
}

const fn target_sort_key(target: &IdeationTarget) -> u8 {
    match target.artifact_type {
        IdeationTargetType::Source => 0,
        IdeationTargetType::Requirement => 1,
        IdeationTargetType::Resolution => 2,
        IdeationTargetType::Rule => 3,
        IdeationTargetType::Topic => 4,
        IdeationTargetType::Question => 5,
        IdeationTargetType::Domain => 6,
    }
}

fn sort_targets(targets: &mut Vec<IdeationTarget>) {
    targets.sort_by(|left, right| {
        target_sort_key(left)
            .cmp(&target_sort_key(right))
            .then_with(|| left.artifact_id.as_str().cmp(right.artifact_id.as_str()))
    });
    targets.dedup();
}
