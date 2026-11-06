//! Dictionary import: import the ASD-STE100 dictionary from one local PDF file.

use crate::WorkFuture;
use serde::{Deserialize, Serialize};

/// The `dictionary import` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DictionaryImportInput {
    pub pdf: String,
}

/// The import result: the dictionary identity and the row counts. It holds no
/// dictionary entry content.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DictionaryImportOutput {
    pub issue: u8,
    pub source_sha256: String,
    pub data_sha256: String,
    pub extractor_version: String,
    pub approved_rows: usize,
    pub unapproved_rows: usize,
}

/// The port that imports the dictionary into the bound repository.
pub trait DictionaryImportPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn dictionary_import(
        &self,
        input: DictionaryImportInput,
    ) -> WorkFuture<'_, DictionaryImportOutput, Self::Failure>;
}

impl<P: DictionaryImportPort> crate::Porcelain<P> {
    /// Imports the dictionary PDF and records its reference in the bound repository.
    pub async fn dictionary_import(
        &self,
        input: DictionaryImportInput,
    ) -> Result<DictionaryImportOutput, P::Failure> {
        self.port.dictionary_import(input).await
    }
}

/// The schema of the dictionary import result.
pub fn import_output_schema() -> serde_json::Value {
    crate::result_schema::<DictionaryImportOutput>()
}

/// One line that tells a person what the import did.
pub fn render_import_readable(output: &DictionaryImportOutput) -> String {
    format!(
        "Imported the ASD-STE100 Issue {} dictionary: {} approved rows and {} other rows \
         (data SHA-256 {}).",
        output.issue, output.approved_rows, output.unapproved_rows, output.data_sha256
    )
}
