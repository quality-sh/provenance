//! Graph references: issue an immutable reference to committed graph state, and read,
//! verify, or export the graph that a reference pins.

use crate::WorkFuture;
use serde::Deserialize;

/// The `graph-reference issue` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReferenceIssueInput {
    pub commit: Option<String>,
    pub correlation_system: Option<String>,
    pub correlation_key: Option<String>,
    pub format: String,
}

/// The `graph-reference show`, `verify`, and `exact-export` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReferenceInput {
    pub reference: String,
    pub format: String,
}

/// The port that issues and reads the graph references of the bound repository. Each
/// result is the JSON document of the reference operation.
pub trait GraphReferencePort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn issue(&self, input: ReferenceIssueInput)
        -> WorkFuture<'_, serde_json::Value, Self::Failure>;
    fn show(&self, input: ReferenceInput) -> WorkFuture<'_, serde_json::Value, Self::Failure>;
    fn verify(&self, input: ReferenceInput) -> WorkFuture<'_, serde_json::Value, Self::Failure>;
    fn exact_export(
        &self,
        input: ReferenceInput,
    ) -> WorkFuture<'_, serde_json::Value, Self::Failure>;
}

impl<P: GraphReferencePort> crate::Porcelain<P> {
    /// Issues a reference to the committed graph state of the bound scope.
    pub async fn reference_issue(
        &self,
        input: ReferenceIssueInput,
    ) -> Result<serde_json::Value, P::Failure> {
        self.port.issue(input).await
    }

    /// The metadata and the graph-family counts of a reference.
    pub async fn reference_show(
        &self,
        input: ReferenceInput,
    ) -> Result<serde_json::Value, P::Failure> {
        self.port.show(input).await
    }

    /// Verifies the identity and the graph content of a reference at its Git revision.
    pub async fn reference_verify(
        &self,
        input: ReferenceInput,
    ) -> Result<serde_json::Value, P::Failure> {
        self.port.verify(input).await
    }

    /// The canonical graph that a reference pins, rebuilt from its Git revision.
    pub async fn reference_exact_export(
        &self,
        input: ReferenceInput,
    ) -> Result<serde_json::Value, P::Failure> {
        self.port.exact_export(input).await
    }
}
