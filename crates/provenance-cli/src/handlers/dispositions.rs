use super::common::canonical_artifact;
use crate::cli::ideation::DispositionsCommand;
use crate::output;
use provenance_core::{
    DispositionActor, DispositionDecision, ExternalActionCorrelation, IdentityType, ScopeId,
    StableId,
};
use provenance_store::{
    layout::ProvenanceLayout,
    state_store::{CreateDispositionInput, StateStore},
};

pub(super) fn handle(
    command: DispositionsCommand,
    actor_claim: Option<&provenance_core::RbacClaim>,
) -> anyhow::Result<()> {
    match command {
        DispositionsCommand::Create {
            repo,
            scope,
            id,
            proposal_id,
            decision,
            rationale,
            actor_id,
            actor_type,
            actor_name,
            canonical_artifact_type,
            canonical_artifact_id,
            external_system,
            external_scope,
            external_kind,
            external_key,
            format,
        } => {
            // The recorded actor is the acting principal on the disposition
            // path: the subcommand's own --actor-id is an explicit argv
            // attestation, so it doubles as the claim when the global flag
            // did not carry one (it is shadowed inside this subcommand).
            let fallback = provenance_core::RbacClaim {
                actor_id: actor_id.clone(),
            };
            let claim = actor_claim.unwrap_or(&fallback);
            let disposition = StateStore::new(ProvenanceLayout::new(repo)).create_disposition(
                Some(claim),
                CreateDispositionInput {
                    scope_id: ScopeId::new(scope)?,
                    id: StableId::new(id)?,
                    proposal_id: StableId::new(proposal_id)?,
                    decision: DispositionDecision::parse(&decision)?,
                    rationale,
                    actor: DispositionActor {
                        identity_type: IdentityType::parse(&actor_type)?,
                        id: actor_id,
                        name: actor_name,
                    },
                    canonical_artifact: canonical_artifact(
                        canonical_artifact_type,
                        canonical_artifact_id,
                    )?,
                    external_action: match (
                        external_system,
                        external_scope,
                        external_kind,
                        external_key,
                    ) {
                        (Some(system), Some(scope), Some(kind), Some(key)) => {
                            Some(ExternalActionCorrelation {
                                system,
                                scope,
                                kind,
                                key,
                            })
                        }
                        (None, None, None, None) => None,
                        _ => anyhow::bail!(
                            "external action requires system, scope, kind, and key together"
                        ),
                    },
                },
            )?;
            output::print(format, &disposition)?;
        }
        DispositionsCommand::List {
            repo,
            scope,
            format,
        } => {
            let store = StateStore::new(ProvenanceLayout::new(repo));
            let scope_id = ScopeId::new(scope)?;
            let dispositions = store.with_repository_publication(|| {
                store.validate_ideation_scope(&scope_id)?;
                store.list_dispositions(&scope_id)
            })?;
            output::print(format, &dispositions)?;
        }
    }
    Ok(())
}
