//! The deletions that typed-spec apply refuses, decided once for the plan and
//! the apply.

use provenance_core::{NodeType, StableId};
use provenance_macros::rule;

use super::cascade::CascadeDeletion;
use super::view::{node_type, ScopeRecords, TypedSpecView};
use crate::families::review_families::REVIEW_FAMILIES;
use crate::state_store::{
    CascadedResource, DeletionRefusal, ReconcileState, ReconciledResource, TypedResourceKind,
};
use crate::write_error::WriteFailure;

/// One deletion that typed-spec apply refuses, with its cause.
pub(super) struct RefusedDeletion {
    kind: NodeType,
    id: StableId,
    refusal: DeletionRefusal,
}

impl RefusedDeletion {
    fn names(&self, kind: NodeType, id: &StableId) -> bool {
        self.kind == kind && &self.id == id
    }

    /// The refusal that typed-spec apply gives for this deletion.
    pub(super) fn refuse(self) -> anyhow::Error {
        match self.refusal {
            DeletionRefusal::ReviewSubmission { message } => crate::refusal::refuse!(
                (WriteFailure::EnrolledRecordDeletionConflict {
                    record_kind: self.kind,
                    record_id: self.id,
                    message: message.clone(),
                }),
                "{message}"
            ),
            DeletionRefusal::Disposition { message, .. } => {
                crate::refusal::refuse!(InvalidUpdate, "{message}")
            }
        }
    }
}

/// Lists each deletion that review history keeps, in the order in which apply
/// checks them: first each record that a Disposition names, in Disposition
/// order, then each record that has a review submission, in review record
/// order. Apply refuses with the first one.
#[rule("rule_typed_plan_shows_refused_deletions")]
pub(super) fn refused_deletions(
    view: &TypedSpecView,
    survivors: &ScopeRecords<'_>,
    cascade: &[CascadeDeletion],
) -> Vec<RefusedDeletion> {
    let mut refused: Vec<RefusedDeletion> = Vec::new();
    let is_refused = |refused: &[RefusedDeletion], kind, id: &StableId| {
        refused.iter().any(|entry| entry.names(kind, id))
    };
    for disposition in &view.review.dispositions {
        let Some(artifact) = &disposition.canonical_artifact else {
            continue;
        };
        let (kind, id) = (node_type(artifact.artifact_type), &artifact.artifact_id);
        if survivors.contain(kind, None, id) || is_refused(&refused, kind, id) {
            continue;
        }
        let message = format!(
            "cannot delete canonical {} {} because disposition {} names it; {}",
            kind.as_str(),
            id.as_str(),
            disposition.id.as_str(),
            correction(kind, id, cascade)
        );
        refused.push(RefusedDeletion {
            kind,
            id: id.clone(),
            refusal: DeletionRefusal::Disposition {
                disposition_id: disposition.id.clone(),
                message,
            },
        });
    }
    for family in REVIEW_FAMILIES {
        for id in record_ids(view, family.kind) {
            let kind = family.kind;
            if survivors.contain(kind, None, id)
                || is_refused(&refused, kind, id)
                || !view.review.has_submission(kind, id)
            {
                continue;
            }
            let message = format!(
                "cannot delete {} {} because it has a review submission; {}",
                kind.as_str(),
                id.as_str(),
                correction(kind, id, cascade)
            );
            refused.push(RefusedDeletion {
                kind,
                id: id.clone(),
                refusal: DeletionRefusal::ReviewSubmission { message },
            });
        }
    }
    refused
}

/// Gives each refused deletion the state `refused` and its cause, in the
/// entry of the record. Each deletion has an entry: the document or the
/// cascade deletes the record.
pub(super) fn report(
    refused: &[RefusedDeletion],
    resources: &mut [ReconciledResource],
    cascade: &mut [CascadedResource],
) {
    for entry in refused {
        let resource = resources
            .iter_mut()
            .find(|resource| entry.names(node_kind(resource.kind), &resource.id));
        if let Some(resource) = resource {
            resource.state = ReconcileState::Refused;
            resource.refusal = Some(entry.refusal.clone());
            continue;
        }
        let cascaded = cascade
            .iter_mut()
            .find(|cascaded| entry.names(cascaded.kind, &cascaded.id));
        if let Some(cascaded) = cascaded {
            cascaded.state = ReconcileState::Refused;
            cascaded.refusal = Some(entry.refusal.clone());
        }
    }
}

/// How the caller keeps the record: the typed document keeps the record, or
/// the deleted Requirements that make the cascade delete it.
fn correction(kind: NodeType, id: &StableId, cascade: &[CascadeDeletion]) -> String {
    let cause = cascade
        .iter()
        .find(|deletion| deletion.kind == kind && &deletion.id == id);
    let Some(cause) = cause else {
        let (kind, id) = (kind.as_str(), id.as_str());
        return format!("keep {kind} {id} in the typed document");
    };
    // A Resolution or a Rule stays when one Requirement that it cites stays.
    // A Question stays only when its Requirement and the Requirement of its
    // Topic stay.
    let joint = match kind {
        NodeType::Resolution | NodeType::Rule => " or ",
        NodeType::Source | NodeType::Requirement | NodeType::Topic | NodeType::Question => " and ",
    };
    let requirements = cause
        .requirements
        .iter()
        .map(|requirement| format!("requirement {}", requirement.as_str()))
        .collect::<Vec<_>>()
        .join(joint);
    format!("keep {requirements} in the typed document")
}

fn record_ids(view: &TypedSpecView, kind: NodeType) -> Vec<&StableId> {
    match kind {
        NodeType::Source => view.sources.iter().map(|r| &r.id).collect(),
        NodeType::Requirement => view.requirements.iter().map(|r| &r.id).collect(),
        NodeType::Rule => view.rules.iter().map(|r| &r.id).collect(),
        NodeType::Resolution => view.resolutions.iter().map(|r| &r.id).collect(),
        NodeType::Topic => view.topics.iter().map(|r| &r.id).collect(),
        NodeType::Question => view.questions.iter().map(|r| &r.id).collect(),
    }
}

const fn node_kind(kind: TypedResourceKind) -> NodeType {
    match kind {
        TypedResourceKind::Source => NodeType::Source,
        TypedResourceKind::Requirement => NodeType::Requirement,
        TypedResourceKind::Rule => NodeType::Rule,
    }
}

#[cfg(test)]
mod tests;
