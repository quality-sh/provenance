//! The schema adapter: the closed JSON Schema of each artifact kind.

use super::ArtifactPort;
use provenance_porcelain::schema::{Artifact, SchemaPort, SchemaShowInput};
use provenance_porcelain::WorkFuture;
use serde_json::{json, Value};

mod artifacts;
mod common;

impl SchemaPort for ArtifactPort {
    type Failure = anyhow::Error;

    fn schema_show(&self, input: SchemaShowInput) -> WorkFuture<'_, Value, anyhow::Error> {
        Box::pin(async move { Ok(schema_for(input.artifact)) })
    }
}

fn schema_for(artifact: Artifact) -> Value {
    let schema = match artifact {
        Artifact::Contribution => artifacts::contribution::schema(),
        Artifact::SynthesisPacket => artifacts::synthesis_packet::schema(),
        Artifact::Proposal => artifacts::proposal::schema(),
        Artifact::Assertion => artifacts::lifecycle::assertion_schema(),
        Artifact::Disposition => artifacts::lifecycle::disposition_schema(),
        Artifact::GraphReference => artifacts::graph_reference::reference_schema(),
        Artifact::GraphReferenceExport => artifacts::graph_reference::export_schema(),
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
    let validator = jsonschema::draft7::new(&schema).map_err(|error| {
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
