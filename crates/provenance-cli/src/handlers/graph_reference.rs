use crate::repo_context::RepoContext;
use crate::{cli::graph::GraphReferenceCommand, output};
use provenance_store::graph_reference::{ExternalCorrelation, GraphReference, GraphReferences};

pub(super) fn handle(command: GraphReferenceCommand) -> anyhow::Result<()> {
    match command {
        GraphReferenceCommand::Issue {
            context,
            commit,
            correlation_system,
            correlation_key,
        } => {
            let correlation = correlation_system
                .zip(correlation_key)
                .map(|(system, key)| ExternalCorrelation { system, key });
            let reference =
                GraphReferences::open(&context.open()?)?.issue(commit.as_deref(), correlation)?;
            output::print_json(&reference)?;
        }
        GraphReferenceCommand::Show { repo, reference } => {
            let reference = read_reference(&reference)?;
            output::print_json(&references_for(repo, &reference)?.show(&reference)?)?;
        }
        GraphReferenceCommand::Verify { repo, reference } => {
            let reference = read_reference(&reference)?;
            output::print_json(&references_for(repo, &reference)?.verify(&reference)?)?;
        }
        GraphReferenceCommand::ExactExport { repo, reference } => {
            let reference = read_reference(&reference)?;
            output::print_json(&references_for(repo, &reference)?.exact_export(&reference)?)?;
        }
    }
    Ok(())
}

/// The references of the repository opened for the scope the reference names.
fn references_for(
    repo: camino::Utf8PathBuf,
    reference: &GraphReference,
) -> anyhow::Result<GraphReferences> {
    let repository = RepoContext::new(repo, reference.scope_id.as_str()).open()?;
    Ok(GraphReferences::open(&repository)?)
}

fn read_reference(path: &camino::Utf8Path) -> anyhow::Result<GraphReference> {
    Ok(GraphReference::from_json(&std::fs::read(path)?)?)
}
