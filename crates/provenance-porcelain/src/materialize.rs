//! Materialize: rebuild the local query cache from the committed graph files.

use crate::WorkFuture;
use serde::{Deserialize, Serialize};

/// The `materialize` input. It has no fields: the command rebuilds the cache of the
/// repository that the consumer binds.
#[derive(Clone, Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MaterializeInput {}

/// The rebuilt cache.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MaterializeOutput {
    pub records_loaded: u64,
    pub cache_recreated: bool,
}

/// The port that rebuilds the cache of one repository.
pub trait MaterializePort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn materialize(
        &self,
        input: MaterializeInput,
    ) -> WorkFuture<'_, MaterializeOutput, Self::Failure>;
}

impl<P: MaterializePort> crate::Porcelain<P> {
    /// Rebuilds the cache of the bound repository.
    pub async fn materialize(
        &self,
        input: MaterializeInput,
    ) -> Result<MaterializeOutput, P::Failure> {
        self.port.materialize(input).await
    }
}

/// The schema of the materialize result.
pub fn output_schema() -> serde_json::Value {
    crate::result_schema::<MaterializeOutput>()
}

/// One line that tells a person what the rebuild did.
pub fn render_readable(output: &MaterializeOutput) -> String {
    let file = if output.cache_recreated {
        " in a new cache file"
    } else {
        ""
    };
    format!("The cache has {} records{file}.", output.records_loaded)
}
