//! Swarm-backtrace land: write the records of one swarm run directory to the bound scope.

use crate::WorkFuture;
use serde::{Deserialize, Serialize};

/// The `swarm-backtrace land` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SwarmBacktraceLandInput {
    pub run_dir: String,
    #[serde(default)]
    pub replace: bool,
}

/// The landing result: the count of each record kind that the run wrote.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SwarmBacktraceLandOutput {
    pub run_dir: String,
    pub contributions: usize,
    pub synthesis_packets: usize,
    pub proposals: usize,
    pub assertions: usize,
    pub replace: bool,
}

/// The port that lands one swarm run in the bound scope.
pub trait SwarmBacktraceLandPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn swarm_backtrace_land(
        &self,
        input: SwarmBacktraceLandInput,
    ) -> WorkFuture<'_, SwarmBacktraceLandOutput, Self::Failure>;
}

impl<P: SwarmBacktraceLandPort> crate::Porcelain<P> {
    /// Lands the records of the run directory in the bound scope.
    pub async fn swarm_backtrace_land(
        &self,
        input: SwarmBacktraceLandInput,
    ) -> Result<SwarmBacktraceLandOutput, P::Failure> {
        self.port.swarm_backtrace_land(input).await
    }
}

/// The schema of the landing result.
pub fn output_schema() -> serde_json::Value {
    crate::result_schema::<SwarmBacktraceLandOutput>()
}

/// One line that tells a person what the landing wrote.
pub fn render_readable(output: &SwarmBacktraceLandOutput) -> String {
    format!(
        "Landed {} contributions, {} synthesis packets, {} proposals, and {} assertions \
         from {}.",
        output.contributions,
        output.synthesis_packets,
        output.proposals,
        output.assertions,
        output.run_dir
    )
}
