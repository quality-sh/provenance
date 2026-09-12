//! The exact-export document: a pinned graph that travels on its own.
//!
//! A reference is a claim about a repository. An exact export is the graph
//! itself, handed to someone who may never see that repository, so what a
//! reference can defer to Git this document has to carry.

use super::{
    canonical::{canonical_bytes, digest},
    ensure_graph_schema_version, incomplete, mismatch, projection, validate_prefixed_hash,
    GraphExport, GraphReferenceError,
};
use provenance_core::{SchemaVersion, SUPPORTED_SCHEMA_VERSION};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExactExport {
    pub schema_version: u32,
    pub operation: &'static str,
    pub reference_id: String,
    pub graph_digest: String,
    pub graph: GraphExport,
}

/// The digest a pinned graph hashes to.
///
/// One definition of `graph_digest` wherever it is written down: the reference
/// records it when it is issued, the export document carries it, and both are
/// this hash over the same canonical bytes. Holders outside this crate need it
/// to say what digest a graph in their hands should carry.
pub fn graph_digest(graph: &GraphExport) -> Result<String, GraphReferenceError> {
    let mut content = graph.clone();
    for record in &mut content.sources {
        record.created = None;
        record.updated = None;
    }
    for record in &mut content.requirements {
        record.created = None;
        record.updated = None;
    }
    for record in &mut content.resolutions {
        record.created = None;
        record.updated = None;
    }
    for record in &mut content.rules {
        record.created = None;
        record.updated = None;
    }
    Ok(digest(&canonical_bytes(&content)?))
}

impl ExactExport {
    /// Decides whether an exact-export document may be read as the graph it
    /// claims to be.
    ///
    /// The document travels alone. Whoever holds it may have no repository to
    /// check it against, so what it is checked against has to be inside it.
    /// The digest is what makes it self-verifying offline: the graph that
    /// travelled is hashed again over the same canonical bytes the reference
    /// was cut from, and a document whose recorded digest is not that hash is
    /// refused, naming both digests. Swapping the whole graph object for
    /// another repository's graph and editing one field of one record inside
    /// it both land here, because nothing else in the document changes when
    /// they do.
    ///
    /// This is the offline half of the verification story. The online half is
    /// `GraphReferences::verify_and_load`, carrying
    /// `#[rule("rule_reference_verified")]`, which re-materializes the graph
    /// from the pinned commit and so can check what a document cannot carry:
    /// that the commit exists, that the repository is the one named, and that
    /// the reference id is the hash of those parts. A holder with the
    /// repository should use that. A holder with only this document gets the
    /// digest, which is the part that says the records in front of them are
    /// the records that were pinned.
    ///
    /// The reference id is checked for shape and not recomputed, because it
    /// cannot be: the id is the hash of repository identity, store path,
    /// scope, commit, and digest, and the document carries none of the first
    /// four. A holder who also has the reference closes that gap without a
    /// repository, by matching both fields across the two documents: same
    /// reference id, same graph digest. Then the digest verified here is the
    /// digest the reference names, and the id names where it came from.
    ///
    /// Shape is settled before the digest is taken. Families the projection
    /// cannot hold, record schema versions it does not support, collaboration
    /// state that should not have travelled, and records belonging to another
    /// scope are refused first, so a document that is not a legal pinned graph
    /// is reported as that and not as a hash that failed to match.
    pub fn from_json(bytes: &[u8]) -> Result<Self, GraphReferenceError> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Document {
            schema_version: u32,
            operation: String,
            reference_id: String,
            graph_digest: String,
            graph: GraphExport,
        }

        let mut unknown = None;
        let mut deserializer = serde_json::Deserializer::from_slice(bytes);
        let mut document: Document = serde_ignored::deserialize(&mut deserializer, |path| {
            if unknown.is_none() {
                unknown = Some(path.to_string());
            }
        })
        .map_err(incomplete)?;
        if let Some(path) = unknown {
            return Err(incomplete(format!("unknown field `{path}`")));
        }
        ensure_graph_schema_version("exact export", SchemaVersion(document.schema_version))?;
        if document.operation != "exact-export" {
            return Err(GraphReferenceError::Incomplete {
                detail: "exact export operation must be 'exact-export'".into(),
            });
        }
        validate_prefixed_hash("reference_id", &document.reference_id, "grf1_", 64)?;
        validate_prefixed_hash("graph_digest", &document.graph_digest, "sha256:", 64)?;
        ensure_graph_schema_version("graph", SchemaVersion(document.graph.schema_version))?;
        document.graph.validate_schema_versions()?;
        document.graph.validate_no_collaboration_fields()?;
        projection::validate_scope_ownership(&document.graph, &document.graph.scope.id)?;
        let graph_digest = graph_digest(&document.graph)?;
        if graph_digest != document.graph_digest {
            return mismatch("graph_digest", &document.graph_digest, &graph_digest);
        }
        Ok(Self {
            schema_version: SUPPORTED_SCHEMA_VERSION.0,
            operation: "exact-export",
            reference_id: document.reference_id,
            graph_digest,
            graph: document.graph,
        })
    }
}
