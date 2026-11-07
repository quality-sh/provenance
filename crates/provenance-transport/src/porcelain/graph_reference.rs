//! The graph reference adapter: issue, show, verify, and exact-export.

use super::RepositoryPort;
use provenance_porcelain::graph_reference::{
    GraphReferencePort, ReferenceInput, ReferenceIssueInput,
};
use provenance_porcelain::WorkFuture;
use provenance_store::graph_reference::{
    ExactExport, ExternalCorrelation, GraphReference, GraphReferenceSummary, GraphReferences,
    Verification,
};

impl GraphReferencePort for RepositoryPort {
    type Failure = anyhow::Error;
    type Reference = GraphReference;
    type Summary = GraphReferenceSummary;
    type Verification = Verification;
    type Export = ExactExport;

    fn issue(&self, input: ReferenceIssueInput) -> WorkFuture<'_, GraphReference, anyhow::Error> {
        Box::pin(async move {
            let correlation = input
                .correlation_system
                .zip(input.correlation_key)
                .map(|(system, key)| ExternalCorrelation { system, key });
            let repository = self.context.open()?;
            Ok(GraphReferences::open(&self.context.repo)?.issue(
                &repository,
                input.commit.as_deref(),
                correlation,
            )?)
        })
    }

    fn show(&self, input: ReferenceInput) -> WorkFuture<'_, GraphReferenceSummary, anyhow::Error> {
        Box::pin(async move {
            let reference = read_reference(&input.reference)?;
            Ok(GraphReferences::open(&self.context.repo)?.show(&reference)?)
        })
    }

    fn verify(&self, input: ReferenceInput) -> WorkFuture<'_, Verification, anyhow::Error> {
        Box::pin(async move {
            let reference = read_reference(&input.reference)?;
            Ok(GraphReferences::open(&self.context.repo)?.verify(&reference)?)
        })
    }

    fn exact_export(&self, input: ReferenceInput) -> WorkFuture<'_, ExactExport, anyhow::Error> {
        Box::pin(async move {
            let reference = read_reference(&input.reference)?;
            Ok(GraphReferences::open(&self.context.repo)?.exact_export(&reference)?)
        })
    }
}

fn read_reference(path: &str) -> anyhow::Result<GraphReference> {
    Ok(GraphReference::from_json(&std::fs::read(path)?)?)
}
