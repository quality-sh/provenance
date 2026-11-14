//! The deletions that typed-spec apply refuses, decided once for the plan and
//! the apply.

use provenance_core::{
    CanonicalArtifactType, NodeType, Question, Requirement, Resolution, Rule, ScopeId, Source,
    StableId, Topic,
};
use provenance_macros::rule;

use super::view::TypedSpecView;
use crate::families::review_families::REVIEW_FAMILIES;
use crate::state_store::{
    CascadedResource, DeletionRefusal, ReconcileState, ReconciledResource, TypedResourceKind,
};
use crate::write_error::WriteFailure;

/// The records that a typed-spec publication keeps, by kind.
pub(super) struct Survivors<'a> {
    pub(super) sources: &'a [Source],
    pub(super) requirements: &'a [Requirement],
    pub(super) rules: &'a [Rule],
    pub(super) resolutions: &'a [Resolution],
    pub(super) topics: &'a [Topic],
    pub(super) questions: &'a [Question],
}

impl Survivors<'_> {
    /// Whether the publication keeps the record, in `scope` when it is given.
    fn contain(&self, kind: NodeType, scope: Option<&ScopeId>, id: &StableId) -> bool {
        match kind {
            NodeType::Source => holds(self.sources, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Requirement => holds(self.requirements, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Rule => holds(self.rules, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Resolution => holds(self.resolutions, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Topic => holds(self.topics, |r| (&r.scope_id, &r.id), scope, id),
            NodeType::Question => holds(self.questions, |r| (&r.scope_id, &r.id), scope, id),
        }
    }
}

fn holds<T>(
    records: &[T],
    identity: fn(&T) -> (&ScopeId, &StableId),
    scope: Option<&ScopeId>,
    id: &StableId,
) -> bool {
    records.iter().map(identity).any(|(record_scope, record_id)| {
        record_id == id && scope.is_none_or(|scope| scope == record_scope)
    })
}

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
    scope: &ScopeId,
    survivors: &Survivors<'_>,
    declared: &[ReconciledResource],
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
        if survivors.contain(kind, Some(scope), id) || is_refused(&refused, kind, id) {
            continue;
        }
        let message = format!(
            "cannot delete canonical {} {} because disposition {} names it; {}",
            kind.as_str(),
            id.as_str(),
            disposition.id.as_str(),
            correction(kind, id, declared)
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
                correction(kind, id, declared)
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
/// entry of the record. A record that has no entry gets a cascade entry.
pub(super) fn report(
    refused: &[RefusedDeletion],
    resources: &mut [ReconciledResource],
    cascade: &mut Vec<CascadedResource>,
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
            continue;
        }
        cascade.push(CascadedResource {
            kind: entry.kind,
            id: entry.id.clone(),
            state: ReconcileState::Refused,
            changes: Vec::new(),
            refusal: Some(entry.refusal.clone()),
        });
    }
}

/// How the caller keeps the record: a declaration stays in the typed
/// document, and a record that the cascade deletes keeps its Requirement.
fn correction(kind: NodeType, id: &StableId, declared: &[ReconciledResource]) -> String {
    let declaration = declared.iter().any(|resource| {
        node_kind(resource.kind) == kind
            && &resource.id == id
            && resource.state == ReconcileState::Deleted
    });
    if declaration {
        format!("keep the {} in the typed document", kind.as_str())
    } else {
        format!(
            "keep the Requirement that the {} cites in the typed document",
            kind.as_str()
        )
    }
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

const fn node_type(kind: CanonicalArtifactType) -> NodeType {
    match kind {
        CanonicalArtifactType::Source => NodeType::Source,
        CanonicalArtifactType::Requirement => NodeType::Requirement,
        CanonicalArtifactType::Resolution => NodeType::Resolution,
        CanonicalArtifactType::Rule => NodeType::Rule,
        CanonicalArtifactType::Topic => NodeType::Topic,
        CanonicalArtifactType::Question => NodeType::Question,
    }
}
