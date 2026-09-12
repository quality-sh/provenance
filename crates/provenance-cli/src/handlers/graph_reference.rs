use crate::{cli::graph::GraphReferenceCommand, output};
use provenance_store::graph_reference::{ExternalCorrelation, GraphReference, GraphReferences};

pub(super) fn handle(command: GraphReferenceCommand) -> anyhow::Result<()> {
    match command {
        GraphReferenceCommand::Issue {
            repo,
            scope,
            commit,
            correlation_system,
            correlation_key,
        } => {
            let correlation = correlation_system
                .zip(correlation_key)
                .map(|(system, key)| ExternalCorrelation { system, key });
            let reference =
                GraphReferences::open(&repo)?.issue(&scope, commit.as_deref(), correlation)?;
            output::print_json(&reference)?;
        }
        GraphReferenceCommand::Show { repo, reference } => {
            let reference = read_reference(&reference)?;
            output::print_json(&GraphReferences::open(&repo)?.show(&reference)?)?;
        }
        GraphReferenceCommand::Verify { repo, reference } => {
            let reference = read_reference(&reference)?;
            output::print_json(&GraphReferences::open(&repo)?.verify(&reference)?)?;
        }
        GraphReferenceCommand::ExactExport { repo, reference } => {
            let reference = read_reference(&reference)?;
            output::print_json(&GraphReferences::open(&repo)?.exact_export(&reference)?)?;
        }
    }
    Ok(())
}

fn read_reference(path: &camino::Utf8Path) -> anyhow::Result<GraphReference> {
    Ok(GraphReference::from_json(&std::fs::read(path)?)?)
}
