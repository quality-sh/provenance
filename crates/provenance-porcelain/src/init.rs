//! Init: set up the Provenance files of one repository and the guidance for its agents.

use crate::WorkFuture;
use serde::{Deserialize, Serialize};

/// How the caller runs provenance, so that the guidance names commands that the caller
/// can run.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum InvocationChannel {
    #[default]
    Native,
    Typescript,
}

/// The package manager that runs the TypeScript package.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
    Deno,
    Nub,
}

/// The `init` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitInput {
    pub path: String,
    pub scope: Option<String>,
    pub path_prefix: Option<String>,
    #[serde(default)]
    pub disposition_actor_id: Vec<String>,
    #[serde(default)]
    pub clear_disposition_actors: bool,
    pub ste_pdf: Option<String>,
    #[serde(default)]
    pub invocation_channel: InvocationChannel,
    pub package_manager: Option<PackageManager>,
}

/// The result of an init: the summary of the files that it wrote, and the notices
/// that go with it. The port gives the result only after it published each file.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct InitReport {
    /// The warnings and notes about the init, one for each line.
    pub notices: Vec<String>,
    /// The readable summary of the init.
    pub summary: String,
}

/// The port that sets up repositories.
pub trait InitPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn init(&self, input: InitInput) -> WorkFuture<'_, InitReport, Self::Failure>;
}

impl<P: InitPort> crate::Porcelain<P> {
    /// Sets up the repository at the path of the input.
    pub async fn init(&self, input: InitInput) -> Result<InitReport, P::Failure> {
        self.port.init(input).await
    }
}
