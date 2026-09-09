use serde::{Deserialize, Serialize};

use super::{CanonicalArtifactType, DispositionDecision, IdentityType};
use crate::model::ids::{SchemaVersion, ScopeId, StableId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DispositionActor {
    pub identity_type: IdentityType,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct CanonicalArtifact {
    pub artifact_type: CanonicalArtifactType,
    pub artifact_id: StableId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExternalActionCorrelation {
    pub system: String,
    pub scope: String,
    pub kind: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DispositionRecord {
    pub schema_version: SchemaVersion,
    pub scope_id: ScopeId,
    pub id: StableId,
    pub proposal_id: StableId,
    pub decision: DispositionDecision,
    pub rationale: String,
    pub actor: DispositionActor,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_artifact: Option<CanonicalArtifact>,
    #[serde(
        default,
        deserialize_with = "deserialize_external_action",
        skip_serializing_if = "Option::is_none"
    )]
    pub external_action: Option<ExternalActionCorrelation>,
}

fn deserialize_external_action<'de, D>(
    deserializer: D,
) -> Result<Option<ExternalActionCorrelation>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    ExternalActionCorrelation::deserialize(deserializer).map(Some)
}

/// Must this disposition rest on an assertion recorded before it?
///
/// Accepting a proposal is a claim about its evidence, so an acceptance
/// normally has to name a proposal a swarm already asserted. Rejecting or
/// deferring one is not such a claim and needs no assertion.
///
/// A fork tournament ends the other way round. A person reads the competing
/// artifacts, picks one, and lands the decision as a ratified resolution,
/// which the disposition then names as its canonical artifact. There is no
/// swarm assertion to wait for and never will be: the artifact the person
/// ratified is the evidence, and demanding an assertion behind it would make
/// human acceptance impossible. So an acceptance recorded by a human that
/// names a canonical artifact stands on that artifact instead.
///
/// Both gates that judge an acceptance - the ideation aggregate in this crate
/// and `provenance-store`'s disposition write gate - ask this one question, so
/// the two cannot drift apart.
pub fn disposition_requires_prior_assertion(disposition: &DispositionRecord) -> bool {
    disposition.decision == DispositionDecision::Accepted
        && !(disposition.actor.identity_type == IdentityType::Human
            && disposition.canonical_artifact.is_some())
}

pub fn validate_disposition_intrinsic(disposition: &DispositionRecord) -> anyhow::Result<()> {
    anyhow::ensure!(
        !disposition.rationale.trim().is_empty(),
        "disposition rationale must not be empty"
    );
    anyhow::ensure!(
        !disposition.actor.id.trim().is_empty(),
        "disposition actor id must not be empty"
    );
    if let Some(action) = &disposition.external_action {
        for (name, value) in [
            ("system", &action.system),
            ("scope", &action.scope),
            ("kind", &action.kind),
            ("key", &action.key),
        ] {
            anyhow::ensure!(
                !value.trim().is_empty(),
                "external action {name} must not be empty"
            );
        }
    }
    Ok(())
}
