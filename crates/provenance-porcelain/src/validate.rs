//! Validate: check one artifact file against its closed schema and its record checks.

use crate::schema::Artifact;
use crate::WorkFuture;
use serde::{Deserialize, Serialize};

/// The `validate` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateInput {
    pub artifact: Artifact,
    pub input: String,
    pub format: String,
}

/// The result of a validation that passed. A validation that fails is a failure of
/// the port.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ValidationReport {
    pub artifact: String,
    pub input: String,
    pub valid: bool,
}

/// The port that validates artifact files.
pub trait ValidatePort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn validate(&self, input: ValidateInput) -> WorkFuture<'_, ValidationReport, Self::Failure>;
}

impl<P: ValidatePort> crate::Porcelain<P> {
    /// Validates the artifact file of the input.
    pub async fn validate(&self, input: ValidateInput) -> Result<ValidationReport, P::Failure> {
        self.port.validate(input).await
    }
}
