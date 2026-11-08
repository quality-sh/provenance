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
    };

    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "artifact": artifact.name(),
        "schema": schema,
        "$defs": common::definitions()
    })
}

#[cfg(test)]
mod tests;
