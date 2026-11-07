//! Import: put the records of one scope export in place of the stored scope.

use crate::WorkFuture;
use serde::{Deserialize, Serialize};

/// The `import` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportInput {
    pub input: String,
    #[serde(default)]
    pub dry_run: bool,
}

/// The import result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ImportOutput {
    pub status: String,
    pub dry_run: bool,
    pub records: usize,
}

/// The port that imports one scope export into the bound scope.
pub trait ImportPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn import(&self, input: ImportInput) -> WorkFuture<'_, ImportOutput, Self::Failure>;
}

impl<P: ImportPort> crate::Porcelain<P> {
    /// Imports the export file into the bound scope. A dry run checks it and writes nothing.
    pub async fn import(&self, input: ImportInput) -> Result<ImportOutput, P::Failure> {
        self.port.import(input).await
    }
}

/// The schema of the import result.
pub fn output_schema() -> serde_json::Value {
    crate::result_schema::<ImportOutput>()
}

/// One line that tells a person what the import did.
pub fn render_readable(output: &ImportOutput) -> String {
    if output.dry_run {
        format!(
            "The import of {} records passes its checks. The dry run changed nothing.",
            output.records
        )
    } else {
        format!("Imported {} records.", output.records)
    }
}
