use crate::{
    cli::{IdeationArtifactKind, SchemaCommand},
    output,
};
use serde_json::{json, Value};

mod artifacts;
mod common;

pub(super) fn handle(command: SchemaCommand) -> anyhow::Result<()> {
    match command {
        SchemaCommand::Show { artifact, format } => {
            output::print(format, &schema_for(artifact))?;
        }
    }
    Ok(())
}

fn schema_for(artifact: IdeationArtifactKind) -> Value {
    let schema = match artifact {
        IdeationArtifactKind::Contribution => artifacts::contribution::schema(),
        IdeationArtifactKind::SynthesisPacket => artifacts::synthesis_packet::schema(),
        IdeationArtifactKind::Proposal => artifacts::proposal::schema(),
        IdeationArtifactKind::Assertion => artifacts::lifecycle::assertion_schema(),
        IdeationArtifactKind::Disposition => artifacts::lifecycle::disposition_schema(),
        IdeationArtifactKind::Manifest => artifacts::manifest::schema(),
        IdeationArtifactKind::GraphReference => artifacts::graph_reference::reference_schema(),
        IdeationArtifactKind::GraphReferenceExport => artifacts::graph_reference::export_schema(),
    };

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "artifact": artifact.name(),
        "schema": schema,
        "$defs": common::definitions()
    })
}

pub(super) fn validate_graph_reference_export(value: &Value) -> anyhow::Result<()> {
    let schema = artifacts::graph_reference::export_schema();
    let validator = jsonschema::JSONSchema::compile(&schema).map_err(|error| {
        anyhow::anyhow!("failed to compile graph reference export schema: {error}")
    })?;
    anyhow::ensure!(
        validator.is_valid(value),
        "graph reference export violates its closed schema"
    );
    Ok(())
}

#[cfg(test)]
mod tests;
