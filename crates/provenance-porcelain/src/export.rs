//! Export: the canonical records of one scope as one document.

use crate::WorkFuture;
use serde::{Deserialize, Serialize};

/// The document format of an export.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Json,
    Jsonl,
    Markdown,
    Table,
    Toon,
}

/// The `export` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportInput {
    pub format: ExportFormat,
    pub output: Option<String>,
}

/// A document that a read gives. When the input names an output file, the port
/// writes the text to that file too.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Document {
    pub text: String,
}

/// The port that exports the records of the bound repository and scope.
pub trait ExportPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn export(&self, input: ExportInput) -> WorkFuture<'_, Document, Self::Failure>;
}

impl<P: ExportPort> crate::Porcelain<P> {
    /// Exports the records of the bound scope in the format of the input.
    pub async fn export(&self, input: ExportInput) -> Result<Document, P::Failure> {
        self.port.export(input).await
    }
}
