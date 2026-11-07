//! Cargo init: set up one Cargo package with the Provenance SDK and the repository
//! files that init writes.

use crate::init::InitReport;
use crate::WorkFuture;
use serde::Deserialize;

/// The `__cargo-init` input that the cargo plugin passes on.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CargoInitInput {
    pub package: Option<String>,
    pub ste_pdf: Option<String>,
}

/// The port that sets up Cargo packages.
pub trait CargoInitPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn cargo_init(&self, input: CargoInitInput) -> WorkFuture<'_, InitReport, Self::Failure>;
}

impl<P: CargoInitPort> crate::Porcelain<P> {
    /// Sets up the Cargo package in the current directory.
    pub async fn cargo_init(&self, input: CargoInitInput) -> Result<InitReport, P::Failure> {
        self.port.cargo_init(input).await
    }
}
