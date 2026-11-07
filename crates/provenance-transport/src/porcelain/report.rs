//! The report adapters: render an envelope file, and build the envelope of a repository.

use super::export::deliver;
use super::{ArtifactPort, RepositoryPort};
use anyhow::Context;
use camino::Utf8Path;
use provenance_porcelain::export::Document;
use provenance_porcelain::report::{
    ReportBuildInput, ReportBuildPort, ReportFormat, ReportRenderInput, ReportRenderPort,
};
use provenance_porcelain::WorkFuture;
use provenance_report::{render_envelope, RenderError, RenderFormat};

impl ReportRenderPort for ArtifactPort {
    type Failure = anyhow::Error;

    fn report_render(&self, input: ReportRenderInput) -> WorkFuture<'_, Document, anyhow::Error> {
        Box::pin(async move {
            let path = &input.input;
            let raw =
                std::fs::read_to_string(path).with_context(|| format!("failed to read {path}"))?;
            let format = match input.format {
                ReportFormat::Json => RenderFormat::Json,
                ReportFormat::Markdown => RenderFormat::Markdown,
            };
            let rendered = match render_envelope(&raw, format) {
                Ok(rendered) => rendered,
                Err(RenderError::Parse(source)) => {
                    return Err(anyhow::Error::new(source))
                        .with_context(|| format!("failed to parse report envelope {path}"));
                }
                Err(error) => return Err(error.into()),
            };
            deliver(rendered, input.output.as_deref())
        })
    }
}

impl ReportBuildPort for RepositoryPort {
    type Failure = anyhow::Error;

    fn report_build(&self, input: ReportBuildInput) -> WorkFuture<'_, Document, anyhow::Error> {
        Box::pin(async move {
            let repo = self.context.repo.as_path();
            let scan_path = input.path.as_deref().map_or(repo, Utf8Path::new);
            let envelope =
                provenance_report::build::build_envelope(&provenance_report::build::BuildInput {
                    repo,
                    scan_path,
                    scope: &self.context.scope,
                    base: &input.base,
                    head: &input.head,
                    repository: &input.repository,
                })?;
            deliver(
                serde_json::to_string_pretty(&envelope)?,
                input.output.as_deref(),
            )
        })
    }
}
