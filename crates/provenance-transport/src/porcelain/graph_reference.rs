//! The graph reference adapter: issue, show, verify, and exact-export.

use super::RepositoryPort;
use provenance_porcelain::graph_reference::{
    GraphReferencePort, ReferenceInput, ReferenceIssueInput,
};
use provenance_porcelain::WorkFuture;
use provenance_store::graph_reference::{ExternalCorrelation, GraphReference, GraphReferences};
use serde_json::Value;

impl GraphReferencePort for RepositoryPort {
    type Failure = anyhow::Error;

    fn issue(&self, input: ReferenceIssueInput) -> WorkFuture<'_, Value, anyhow::Error> {
        Box::pin(async move {
            let correlation = input
                .correlation_system
                .zip(input.correlation_key)
                .map(|(system, key)| ExternalCorrelation { system, key });
            let repository = self.context.open()?;
            let reference = GraphReferences::open(&self.context.repo)?.issue(
                &repository,
                input.commit.as_deref(),
                correlation,
            )?;
            Ok(serde_json::to_value(reference)?)
        })
    }

    fn show(&self, input: ReferenceInput) -> WorkFuture<'_, Value, anyhow::Error> {
        Box::pin(async move {
            let reference = read_reference(&input.reference)?;
            Ok(serde_json::to_value(
                GraphReferences::open(&self.context.repo)?.show(&reference)?,
            )?)
        })
    }

    fn verify(&self, input: ReferenceInput) -> WorkFuture<'_, Value, anyhow::Error> {
        Box::pin(async move {
            let reference = read_reference(&input.reference)?;
            Ok(serde_json::to_value(
                GraphReferences::open(&self.context.repo)?.verify(&reference)?,
            )?)
        })
    }

    fn exact_export(&self, input: ReferenceInput) -> WorkFuture<'_, Value, anyhow::Error> {
        Box::pin(async move {
            let reference = read_reference(&input.reference)?;
            Ok(serde_json::to_value(
                GraphReferences::open(&self.context.repo)?.exact_export(&reference)?,
            )?)
        })
    }
}

fn read_reference(path: &str) -> anyhow::Result<GraphReference> {
    Ok(GraphReference::from_json(&std::fs::read(path)?)?)
}
