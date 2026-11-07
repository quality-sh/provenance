//! Schema show: the JSON Schema of one artifact kind.

use crate::WorkFuture;
use serde::Deserialize;

/// One artifact kind that has a closed schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Artifact {
    Contribution,
    SynthesisPacket,
    Proposal,
    Assertion,
    Disposition,
    GraphReference,
    GraphReferenceExport,
}

impl Artifact {
    /// The name of the artifact kind, as the input gives it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Contribution => "contribution",
            Self::SynthesisPacket => "synthesis-packet",
            Self::Proposal => "proposal",
            Self::Assertion => "assertion",
            Self::Disposition => "disposition",
            Self::GraphReference => "graph-reference",
            Self::GraphReferenceExport => "graph-reference-export",
        }
    }
}

/// The `schema show` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SchemaShowInput {
    pub artifact: Artifact,
    pub format: String,
}

/// The port that gives the artifact schemas.
pub trait SchemaPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn schema_show(
        &self,
        input: SchemaShowInput,
    ) -> WorkFuture<'_, serde_json::Value, Self::Failure>;
}

impl<P: SchemaPort> crate::Porcelain<P> {
    /// The schema document of the artifact kind of the input.
    pub async fn schema_show(
        &self,
        input: SchemaShowInput,
    ) -> Result<serde_json::Value, P::Failure> {
        self.port.schema_show(input).await
    }
}
