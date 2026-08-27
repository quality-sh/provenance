mod canonical;
mod export;
mod git;
pub(crate) mod projection;

use camino::Utf8Path;
use provenance_core::{ensure_supported_schema_version, SchemaVersion, SUPPORTED_SCHEMA_VERSION};
use provenance_macros::rule;
use serde::{Deserialize, Serialize};

pub use export::{graph_digest, ExactExport};
pub use projection::GraphExport;

use canonical::{canonical_bytes, digest, sha256};
use git::{GitRepository, TreeSource};
use projection::load_projection;

const STORE_PATH: &str = ".provenance/state";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalCorrelation {
    pub system: String,
    pub key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphReference {
    pub schema_version: u32,
    pub reference_id: String,
    pub repository_id: String,
    pub store_path: String,
    pub scope_id: String,
    pub commit: String,
    pub graph_digest: String,
    #[serde(
        default,
        deserialize_with = "deserialize_correlation",
        skip_serializing_if = "Option::is_none"
    )]
    pub correlation: Option<ExternalCorrelation>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GraphReferenceError {
    #[error("missing graph reference data: {detail}")]
    Missing { detail: String },
    #[error("mismatched graph reference {field}: expected {expected}, actual {actual}")]
    Mismatched {
        field: &'static str,
        expected: String,
        actual: String,
    },
    #[error("incomplete graph reference: {detail}")]
    Incomplete { detail: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphReferenceSummary {
    pub schema_version: u32,
    pub operation: &'static str,
    pub reference: GraphReference,
    pub counts: GraphCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verification {
    pub schema_version: u32,
    pub operation: &'static str,
    pub valid: bool,
    pub reference_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphCounts {
    pub sources: usize,
    pub domains: usize,
    pub requirements: usize,
    pub boundaries: usize,
    pub topics: usize,
    pub questions: usize,
    pub resolutions: usize,
    pub rules: usize,
    pub edges: usize,
}

pub struct GraphReferences {
    repository: GitRepository,
}

impl GraphReference {
    pub fn from_json(bytes: &[u8]) -> Result<Self, GraphReferenceError> {
        let reference: Self =
            serde_json::from_slice(bytes).map_err(|error| GraphReferenceError::Incomplete {
                detail: format!("reference JSON is invalid: {error}"),
            })?;
        reference.validate()?;
        Ok(reference)
    }

    /// Decides what a well-formed graph reference looks like.
    ///
    /// A reference is a claim about someone else's repository that its holder
    /// cannot check by reading it, so the shape of every field is fixed before
    /// anything is resolved: schema version 1; a `grf1_` reference id and a
    /// `git1_` repository id, each carrying exactly 64 lowercase hexadecimal
    /// digits; the store path `.provenance/state` and no other; a scope id
    /// `ScopeId` accepts; a full 40- or 64-character lowercase hexadecimal
    /// commit id, never an abbreviation, so the reference names one commit
    /// rather than a prefix that later grows ambiguous; a `sha256:` graph
    /// digest of 64 lowercase hexadecimal digits; and, when a correlation is
    /// present, both its system and its key filled in, since half a
    /// correlation points nowhere.
    ///
    /// The commit shape is deliberately stricter than `rule_source_commit_pin`:
    /// source pins are locators and may be abbreviated or uppercase, while a
    /// graph reference commit is a durable identity in a claim its holder
    /// cannot resolve against this repository.
    ///
    /// The same decision is replicated as JSON Schema in
    /// `provenance-cli/src/handlers/schema/artifacts/graph_reference.rs`
    /// (`reference_schema`), which is what a holder outside this codebase
    /// validates against; the two must reach the same verdict on every value.
    #[rule("rule_reference_wellformed")]
    fn validate(&self) -> Result<(), GraphReferenceError> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION.0 {
            return Err(GraphReferenceError::Incomplete {
                detail: format!(
                    "unsupported schema_version {}; expected {}",
                    self.schema_version, SUPPORTED_SCHEMA_VERSION.0
                ),
            });
        }
        validate_prefixed_hash("reference_id", &self.reference_id, "grf1_", 64)?;
        validate_prefixed_hash("repository_id", &self.repository_id, "git1_", 64)?;
        if self.store_path != STORE_PATH {
            return Err(GraphReferenceError::Incomplete {
                detail: format!("store_path must be '{STORE_PATH}'"),
            });
        }
        provenance_core::ScopeId::new(self.scope_id.clone()).map_err(incomplete)?;
        if !matches!(self.commit.len(), 40 | 64) || !self.commit.bytes().all(is_lower_hex_digit) {
            return Err(GraphReferenceError::Incomplete {
                detail: "commit must be a full 40- or 64-character hexadecimal object ID".into(),
            });
        }
        validate_prefixed_hash("graph_digest", &self.graph_digest, "sha256:", 64)?;
        if let Some(correlation) = &self.correlation {
            validate_correlation(correlation)?;
        }
        Ok(())
    }
}

impl GraphReferences {
    pub fn open(repo: &Utf8Path) -> Result<Self, GraphReferenceError> {
        Ok(Self {
            repository: GitRepository::open(repo)?,
        })
    }

    pub fn issue(
        &self,
        scope: &str,
        revision: Option<&str>,
        correlation: Option<ExternalCorrelation>,
    ) -> Result<GraphReference, GraphReferenceError> {
        if let Some(correlation) = &correlation {
            validate_correlation(correlation)?;
        }
        let implicit_head = revision.is_none();
        let commit = self.repository.resolve_commit(revision.unwrap_or("HEAD"))?;
        let graph = self.projection(TreeSource::Commit(&commit), scope)?;
        if implicit_head {
            let index = self.projection(TreeSource::Index, scope)?;
            let worktree = load_projection(self.repository.root(), scope)?;
            let committed_bytes = canonical_bytes(&graph)?;
            if committed_bytes != canonical_bytes(&index)?
                || committed_bytes != canonical_bytes(&worktree)?
            {
                return Err(GraphReferenceError::Incomplete {
                    detail: format!(
                        "implicit HEAD requires clean canonical state for scope '{scope}'; commit graph changes first"
                    ),
                });
            }
        }

        let repository_id = self.repository.identity(&commit)?;
        let graph_digest = digest(&canonical_bytes(&graph)?);
        let reference_id =
            reference_identity(&repository_id, STORE_PATH, scope, &commit, &graph_digest);
        Ok(GraphReference {
            schema_version: SUPPORTED_SCHEMA_VERSION.0,
            reference_id,
            repository_id,
            store_path: STORE_PATH.to_string(),
            scope_id: scope.to_string(),
            commit,
            graph_digest,
            correlation,
        })
    }

    pub fn show(
        &self,
        reference: &GraphReference,
    ) -> Result<GraphReferenceSummary, GraphReferenceError> {
        let graph = self.verify_and_load(reference)?;
        Ok(GraphReferenceSummary {
            schema_version: SUPPORTED_SCHEMA_VERSION.0,
            operation: "show",
            reference: reference.clone(),
            counts: GraphCounts::from(&graph),
        })
    }

    pub fn verify(&self, reference: &GraphReference) -> Result<Verification, GraphReferenceError> {
        self.verify_and_load(reference)?;
        Ok(Verification {
            schema_version: SUPPORTED_SCHEMA_VERSION.0,
            operation: "verify",
            valid: true,
            reference_id: reference.reference_id.clone(),
        })
    }

    /// Hands the pinned graph out as a document that can be checked without
    /// this repository.
    ///
    /// The digest travels with the graph rather than being left behind in the
    /// reference: `verify_and_load` has just recomputed it over these bytes
    /// and found it equal to the recorded one, so writing the recorded digest
    /// into the document writes a digest the graph beside it hashes to. That
    /// is what `ExactExport::from_json` checks again on the way in.
    pub fn exact_export(
        &self,
        reference: &GraphReference,
    ) -> Result<ExactExport, GraphReferenceError> {
        Ok(ExactExport {
            schema_version: SUPPORTED_SCHEMA_VERSION.0,
            operation: "exact-export",
            reference_id: reference.reference_id.clone(),
            graph_digest: reference.graph_digest.clone(),
            graph: self.verify_and_load(reference)?,
        })
    }

    fn projection(
        &self,
        source: TreeSource<'_>,
        scope: &str,
    ) -> Result<GraphExport, GraphReferenceError> {
        let tree = self.repository.materialize(source)?;
        load_projection(
            Utf8Path::from_path(tree.path()).ok_or_else(|| GraphReferenceError::Incomplete {
                detail: "temporary Git tree path is not UTF-8".into(),
            })?,
            scope,
        )
    }

    /// Decides when a graph reference is honoured.
    ///
    /// A reference is a claim about a repository its holder does not control,
    /// so none of it is taken on trust. Every read verb comes through here,
    /// and the shape rule runs first, so a malformed reference is refused
    /// before any Git work is done on its behalf. The pinned graph is then
    /// handed back only when four things hold at once:
    ///
    /// 1. the commit the reference names still resolves in this repository and
    ///    is that commit, not an annotated tag peeling to it;
    /// 2. the repository identity taken from that commit, which is the hash of
    ///    its root commits, matches the recorded one, because two repositories
    ///    can hold the same store path and the same scope name;
    /// 3. the graph materialized at that commit still hashes to the recorded
    ///    digest, so the holder gets the bytes that were pinned or an error,
    ///    never a graph that moved underneath the reference;
    /// 4. the reference id is the hash of exactly those parts. An id lifted
    ///    from another reference clears every check above it, because two
    ///    commits over the same graph agree on everything except their ids,
    ///    and quoting it names a graph nobody can produce.
    ///
    /// The checks run in that order and the first failure is returned, so a
    /// reference with several fields edited is reported by the earliest one.
    /// The order shapes the error, not the verdict: all four must pass.
    ///
    /// The rule is impure. It shells to Git and writes the pinned tree to a
    /// temporary directory, so the answer depends on the repository it is
    /// asked about and not on the reference alone. The same reference is
    /// honoured in one clone and refused in another, which is the point.
    #[rule("rule_reference_verified")]
    fn verify_and_load(
        &self,
        reference: &GraphReference,
    ) -> Result<GraphExport, GraphReferenceError> {
        reference.validate()?;
        let commit = self.repository.resolve_commit(&reference.commit)?;
        if commit != reference.commit {
            return mismatch("commit", &reference.commit, &commit);
        }
        let repository_id = self.repository.identity(&commit)?;
        if repository_id != reference.repository_id {
            return mismatch("repository_id", &reference.repository_id, &repository_id);
        }
        let graph = self.projection(TreeSource::Commit(&commit), &reference.scope_id)?;
        let graph_digest = digest(&canonical_bytes(&graph)?);
        if graph_digest != reference.graph_digest {
            return mismatch("graph_digest", &reference.graph_digest, &graph_digest);
        }
        let identity = reference_identity(
            &repository_id,
            STORE_PATH,
            &reference.scope_id,
            &commit,
            &graph_digest,
        );
        if identity != reference.reference_id {
            return mismatch("reference_id", &reference.reference_id, &identity);
        }
        Ok(graph)
    }
}

impl From<&GraphExport> for GraphCounts {
    fn from(graph: &GraphExport) -> Self {
        Self {
            sources: graph.sources.len(),
            domains: graph.domains.len(),
            requirements: graph.requirements.len(),
            boundaries: graph.boundaries.len(),
            topics: graph.topics.len(),
            questions: graph.questions.len(),
            resolutions: graph.resolutions.len(),
            rules: graph.rules.len(),
            edges: graph.edges.len(),
        }
    }
}

fn mismatch<T>(
    field: &'static str,
    expected: &str,
    actual: &str,
) -> Result<T, GraphReferenceError> {
    Err(GraphReferenceError::Mismatched {
        field,
        expected: expected.to_string(),
        actual: actual.to_string(),
    })
}

fn validate_correlation(correlation: &ExternalCorrelation) -> Result<(), GraphReferenceError> {
    if correlation.system.trim().is_empty() || correlation.key.trim().is_empty() {
        return Err(GraphReferenceError::Incomplete {
            detail: "external correlation system and key must not be empty".into(),
        });
    }
    Ok(())
}

fn deserialize_correlation<'de, D>(deserializer: D) -> Result<Option<ExternalCorrelation>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    ExternalCorrelation::deserialize(deserializer).map(Some)
}

fn validate_prefixed_hash(
    field: &str,
    value: &str,
    prefix: &str,
    digits: usize,
) -> Result<(), GraphReferenceError> {
    let Some(hash) = value.strip_prefix(prefix) else {
        return Err(GraphReferenceError::Incomplete {
            detail: format!("{field} must start with '{prefix}'"),
        });
    };
    if hash.len() != digits || !hash.bytes().all(is_lower_hex_digit) {
        return Err(GraphReferenceError::Incomplete {
            detail: format!("{field} must contain {digits} hexadecimal characters"),
        });
    }
    Ok(())
}

const fn is_lower_hex_digit(byte: u8) -> bool {
    byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')
}

fn reference_identity(
    repository_id: &str,
    store_path: &str,
    scope: &str,
    commit: &str,
    graph_digest: &str,
) -> String {
    let framed = format!(
        "graph-reference-v1\0{repository_id}\0{store_path}\0{scope}\0{commit}\0{graph_digest}"
    );
    format!("grf1_{}", sha256(framed.as_bytes()))
}

pub(super) fn incomplete(error: impl std::fmt::Display) -> GraphReferenceError {
    GraphReferenceError::Incomplete {
        detail: error.to_string(),
    }
}

fn ensure_graph_schema_version(
    kind: &str,
    version: SchemaVersion,
) -> Result<(), GraphReferenceError> {
    ensure_supported_schema_version(kind, version).map_err(|_| {
        incomplete(format!(
            "{kind} has unsupported schema_version {}; expected {}",
            version.0, SUPPORTED_SCHEMA_VERSION.0
        ))
    })
}
