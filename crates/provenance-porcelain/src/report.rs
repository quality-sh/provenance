//! Reports: build the pull request report envelope from a repository, and render an
//! envelope as a document.

use crate::export::Document;
use crate::WorkFuture;
use serde::Deserialize;

/// The document format of a rendered report.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReportFormat {
    Json,
    Markdown,
}

/// The `report render` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportRenderInput {
    pub input: String,
    pub format: ReportFormat,
    pub output: Option<String>,
}

/// The `report build` input.
#[derive(Clone, Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReportBuildInput {
    pub base: String,
    pub head: String,
    pub repository: String,
    pub path: Option<String>,
    pub output: Option<String>,
    pub format: String,
}

/// The port that renders report envelopes.
pub trait ReportRenderPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn report_render(&self, input: ReportRenderInput)
        -> WorkFuture<'_, Document, Self::Failure>;
}

/// The port that builds the report envelope of the bound repository and scope.
pub trait ReportBuildPort: Send + Sync {
    /// The failure that this port reports.
    type Failure;

    fn report_build(&self, input: ReportBuildInput) -> WorkFuture<'_, Document, Self::Failure>;
}

impl<P: ReportRenderPort> crate::Porcelain<P> {
    /// Renders the report envelope file of the input.
    pub async fn report_render(&self, input: ReportRenderInput) -> Result<Document, P::Failure> {
        self.port.report_render(input).await
    }
}

impl<P: ReportBuildPort> crate::Porcelain<P> {
    /// Builds the report envelope between two revisions of the bound repository.
    pub async fn report_build(&self, input: ReportBuildInput) -> Result<Document, P::Failure> {
        self.port.report_build(input).await
    }
}
