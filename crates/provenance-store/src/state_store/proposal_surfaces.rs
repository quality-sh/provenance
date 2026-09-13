use provenance_core::{
    ArtifactLinkTargetType, IdeationTarget, IdeationTargetType, PromotionState, ProposalCard,
    ScopeId, Topic,
};
use provenance_macros::rule;
use serde::Serialize;

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

    pub fn for_topic(topic: &Topic) -> Self {
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
        Self::new(Vec::new(), targets)
    }

    pub fn new(mut changed_paths: Vec<String>, mut targets: Vec<IdeationTarget>) -> Self {
        changed_paths.sort();
        changed_paths.dedup();
        targets.sort_by(|left, right| {
            target_sort_key(left)
                .cmp(&target_sort_key(right))
                .then_with(|| left.artifact_id.as_str().cmp(right.artifact_id.as_str()))
        });
        targets.dedup();
        Self {
            changed_paths,
            targets,
        }
    }
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "trigger", rename_all = "snake_case")]
pub enum ProposalSurfaceReason {
    EvidenceSite { path: String },
    Territory { target: IdeationTarget },
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SurfacedProposal {
    pub proposal: ProposalCard,
    pub reasons: Vec<ProposalSurfaceReason>,
}

#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TopicClaim {
    #[serde(flatten)]
    pub topic: Topic,
    pub surfaced_proposals: Vec<SurfacedProposal>,
}

impl StateStore {
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

/// A proposal nobody has disposed of surfaces to a worker when the work
/// touches ground the proposal already claims. The returned reasons are those
/// overlaps; an empty list means the proposal stays out of the worker's way.
///
/// Undisposed is read from the projected promotion state rather than the state
/// the stored row claims: only `proposed` and `asserted` pass. Once a human
/// accepts, rejects, or defers a proposal, or it is marked duplicate or
/// superseded, it never surfaces again, however well it matches.
///
/// Either trigger alone surfaces the proposal:
/// - evidence site: the work touches a file the proposal cited as evidence;
/// - territory: the work lands on the artifact the proposal is about, matched
///   on both artifact type and id.
///
/// Every reason names the overlap that produced it, so the worker can see why
/// the proposal arrived.
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
            .any(|reference| reference.file_path.as_deref() == Some(path.as_str()))
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

const fn target_sort_key(target: &IdeationTarget) -> u8 {
    match target.artifact_type {
        IdeationTargetType::Source => 0,
        IdeationTargetType::Requirement => 1,
        IdeationTargetType::Resolution => 2,
        IdeationTargetType::Rule => 3,
        IdeationTargetType::Topic => 4,
        IdeationTargetType::Question => 5,
        IdeationTargetType::Domain => 6,
        IdeationTargetType::Boundary => 7,
    }
}
