//! The one table of families the projection stores.
//!
//! Every family `provenance.db` holds has exactly one row here. The digest
//! assembler, the materialize loaders, and the catch-up machinery all read
//! this table, so a family added here gains storage, stamping, and
//! invalidation together. A family without a row cannot be stored or
//! stamped.

use crate::{layout::ProvenanceLayout, shards, state_store::StateStore};
use camino::Utf8PathBuf;
use provenance_core::ScopeId;

/// One family of records the projection stores.
///
/// The variant list is the rule: nineteen stored families, no more. Sixteen
/// come from the original cache tables; implementation bindings,
/// verification bindings, and requirement reviews joined when the canonical
/// halves of impact, evidence, and resolve-symbol became projection-attested.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionFamily {
    Sources,
    Domains,
    Requirements,
    Boundaries,
    Topics,
    Questions,
    Edges,
    Resolutions,
    Rules,
    Threads,
    Messages,
    Contributions,
    SynthesisPackets,
    ProposalCards,
    AssertionRecords,
    Dispositions,
    ImplementationBindings,
    VerificationBindings,
    RequirementReviews,
}

impl ProjectionFamily {
    pub const ALL: [Self; 19] = [
        Self::Sources,
        Self::Domains,
        Self::Requirements,
        Self::Boundaries,
        Self::Topics,
        Self::Questions,
        Self::Edges,
        Self::Resolutions,
        Self::Rules,
        Self::Threads,
        Self::Messages,
        Self::Contributions,
        Self::SynthesisPackets,
        Self::ProposalCards,
        Self::AssertionRecords,
        Self::Dispositions,
        Self::ImplementationBindings,
        Self::VerificationBindings,
        Self::RequirementReviews,
    ];

    /// The family's one name: its cache table, its digest-row key, and its
    /// journal label are all this one string.
    pub const fn family_name(self) -> &'static str {
        match self {
            Self::Sources => "sources",
            Self::Domains => "domains",
            Self::Requirements => "requirements",
            Self::Boundaries => "boundaries",
            Self::Topics => "topics",
            Self::Questions => "questions",
            Self::Edges => "edges",
            Self::Resolutions => "resolutions",
            Self::Rules => "rules",
            Self::Threads => "threads",
            Self::Messages => "messages",
            Self::Contributions => "contributions",
            Self::SynthesisPackets => "synthesis_packets",
            Self::ProposalCards => "proposal_cards",
            Self::AssertionRecords => "assertion_records",
            Self::Dispositions => "dispositions",
            Self::ImplementationBindings => "implementation_bindings",
            Self::VerificationBindings => "verification_bindings",
            Self::RequirementReviews => "requirement_reviews",
        }
    }

    /// Whether the family shards per scope. Edges live in one global shard.
    pub const fn is_scoped(self) -> bool {
        !matches!(self, Self::Edges)
    }

    /// The canonical shard file the family's records live in.
    pub(crate) fn shard_path(
        self,
        layout: &ProvenanceLayout,
        scope: Option<&ScopeId>,
    ) -> anyhow::Result<Utf8PathBuf> {
        if self == Self::Edges {
            return Ok(shards::edges_path(layout));
        }
        let scope = required_scope(scope, self)?;
        Ok(match self {
            Self::Sources => shards::sources_path(layout, scope),
            Self::Domains => shards::domains_path(layout, scope),
            Self::Requirements => shards::requirements_path(layout, scope),
            Self::Boundaries => shards::boundaries_path(layout, scope),
            Self::Topics => shards::topics_path(layout, scope),
            Self::Questions => shards::questions_path(layout, scope),
            Self::Resolutions => shards::resolutions_path(layout, scope),
            Self::Rules => shards::rules_path(layout, scope),
            Self::Threads => shards::threads_path(layout, scope),
            Self::Messages => shards::messages_path(layout, scope),
            Self::Contributions => shards::contributions_path(layout, scope),
            Self::SynthesisPackets => shards::synthesis_packets_path(layout, scope),
            Self::ProposalCards => shards::proposal_cards_path(layout, scope),
            Self::AssertionRecords => shards::assertion_records_path(layout, scope),
            Self::Dispositions => shards::dispositions_path(layout, scope),
            Self::ImplementationBindings => shards::implementation_bindings_path(layout, scope),
            Self::VerificationBindings => shards::verification_bindings_path(layout, scope),
            Self::RequirementReviews => shards::requirement_reviews_path(layout, scope),
            Self::Edges => unreachable!("edges answered above"),
        })
    }

    /// The complete set of files the family's reader derivation reads.
    ///
    /// This is the family's byte domain: the hash sweep, the stamp
    /// baselines, and the journal mapping all consume it, so a change in
    /// any contributing file — a month shard, a second edge shard, the
    /// ideation landings overlay, a legacy promotion decision — moves the
    /// family's digest. Fixed members are declared whether or not the file
    /// exists; month and edge shards are discovered exactly where the
    /// readers discover them.
    pub(crate) fn byte_domain(
        self,
        layout: &ProvenanceLayout,
        scope: Option<&ScopeId>,
    ) -> anyhow::Result<Vec<Utf8PathBuf>> {
        if self == Self::Edges {
            return crate::state_store::readers::edge_shard_paths(layout);
        }
        let scope = required_scope(scope, self)?;
        let mut domain = match self {
            Self::Messages => crate::state_store::readers::message_shard_paths(layout, scope)?,
            Self::Contributions
            | Self::SynthesisPackets
            | Self::ProposalCards
            | Self::AssertionRecords => vec![
                self.shard_path(layout, Some(scope))?,
                shards::ideation_landings_path(layout, scope),
            ],
            Self::Dispositions => vec![
                self.shard_path(layout, Some(scope))?,
                shards::ideation_landings_path(layout, scope),
                shards::legacy_promotion_decisions_path(layout, scope),
            ],
            other => vec![other.shard_path(layout, Some(scope))?],
        };
        domain.sort();
        Ok(domain)
    }

    /// The family's records as canonical bytes, sorted by canonical id, with
    /// the record count. Content comes from the canonical shards through the
    /// state store; bytes come from the one canonical writer.
    pub(crate) fn canonical_records(
        self,
        store: &StateStore,
        scope: Option<&ScopeId>,
    ) -> anyhow::Result<(Vec<u8>, u64)> {
        if self == Self::Edges {
            return sorted_bytes(store.list_edges()?, |record| record.id.as_str());
        }
        let scope = required_scope(scope, self)?;
        match self {
            Self::Sources => sorted_bytes(store.list_sources(scope)?, |r| r.id.as_str()),
            Self::Domains => sorted_bytes(store.list_domains(scope)?, |r| r.id.as_str()),
            Self::Requirements => sorted_bytes(store.list_requirements(scope)?, |r| r.id.as_str()),
            Self::Boundaries => sorted_bytes(store.list_boundaries(scope)?, |r| r.id.as_str()),
            Self::Topics => sorted_bytes(store.list_topics(scope)?, |r| r.id.as_str()),
            Self::Questions => sorted_bytes(store.list_questions(scope)?, |r| r.id.as_str()),
            Self::Resolutions => sorted_bytes(store.list_resolutions(scope)?, |r| r.id.as_str()),
            Self::Rules => sorted_bytes(store.list_rules(scope)?, |r| r.id.as_str()),
            Self::Threads => sorted_bytes(store.list_threads(scope)?, |r| r.id.as_str()),
            Self::Messages => sorted_bytes(store.list_messages(scope)?, |r| r.id.as_str()),
            Self::Contributions => {
                sorted_bytes(store.list_contributions(scope)?, |r| r.id.as_str())
            }
            Self::SynthesisPackets => {
                sorted_bytes(store.list_synthesis_packets(scope)?, |r| r.id.as_str())
            }
            Self::ProposalCards => {
                sorted_bytes(store.list_proposal_cards(scope)?, |r| r.id.as_str())
            }
            Self::AssertionRecords => {
                sorted_bytes(store.list_assertion_records(scope)?, |r| r.id.as_str())
            }
            Self::Dispositions => sorted_bytes(store.list_dispositions(scope)?, |r| r.id.as_str()),
            Self::ImplementationBindings => {
                sorted_bytes(store.list_implementation_bindings(scope)?, |r| {
                    r.id.as_str()
                })
            }
            Self::VerificationBindings => {
                sorted_bytes(store.list_verification_bindings(scope)?, |r| r.id.as_str())
            }
            Self::RequirementReviews => {
                sorted_bytes(store.list_requirement_reviews(scope)?, |r| r.id.as_str())
            }
            Self::Edges => unreachable!("edges answered above"),
        }
    }
}

fn required_scope(scope: Option<&ScopeId>, family: ProjectionFamily) -> anyhow::Result<&ScopeId> {
    scope.ok_or_else(|| {
        anyhow::anyhow!(
            "family `{}` shards per scope and needs one",
            family.family_name()
        )
    })
}

fn sorted_bytes<T: serde::Serialize>(
    mut records: Vec<T>,
    id: impl Fn(&T) -> &str,
) -> anyhow::Result<(Vec<u8>, u64)> {
    records.sort_by(|left, right| id(left).cmp(id(right)));
    let count = records.len() as u64;
    Ok((crate::canonical_digest::canonical_bytes(&records)?, count))
}

/// Maps a written shard path back to its declared family, by exact path
/// equality against the family table — never by directory component, so
/// sibling files like `requirements/req.jsonl` and `requirements/review.jsonl`
/// land in their own families, and bindings and every ideation family are
/// covered because the table covers them.
pub fn families_for_shard_path(
    layout: &ProvenanceLayout,
    path: &camino::Utf8Path,
) -> Vec<(ProjectionFamily, Option<ScopeId>)> {
    if path
        .parent()
        .is_some_and(|parent| parent == layout.edges_dir())
        && path.extension() == Some("jsonl")
    {
        return vec![(ProjectionFamily::Edges, None)];
    }
    let Some(scope) = path
        .strip_prefix(layout.scopes_dir())
        .ok()
        .and_then(|relative| relative.components().next())
        .and_then(|component| ScopeId::new(component.as_str()).ok())
    else {
        return Vec::new();
    };
    let matched: Vec<_> = ProjectionFamily::ALL
        .into_iter()
        .filter(|family| family.is_scoped())
        .filter(|family| {
            family
                .byte_domain(layout, Some(&scope))
                .is_ok_and(|domain| domain.iter().any(|member| member == path))
        })
        .map(|family| (family, Some(scope.clone())))
        .collect();
    if !matched.is_empty() {
        return matched;
    }
    // A canonical write under a scope that no declared domain claims is
    // suspect: a domain this table forgot would otherwise lose its hint
    // silently. Broadcasting costs one wasted re-derivation; missing a real
    // family would cost a false freshness hint.
    ProjectionFamily::ALL
        .into_iter()
        .filter(|family| family.is_scoped())
        .map(|family| (family, Some(scope.clone())))
        .collect()
}

/// The byte domain's files as one framed byte stream.
///
/// Each present file contributes its name, a separator, its length, and its
/// bytes, in sorted path order, so records moving between files of one
/// domain still move the digest. An absent file contributes nothing.
pub fn domain_bytes(paths: &[Utf8PathBuf]) -> anyhow::Result<Vec<u8>> {
    let mut framed = Vec::new();
    for path in paths {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        let name = path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("domain member without a file name: {path}"))?;
        framed.extend_from_slice(name.as_bytes());
        framed.push(0);
        framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
        framed.extend_from_slice(&bytes);
    }
    Ok(framed)
}

/// Summed size and newest mtime across the domain's present files.
///
/// Diagnostic metadata only; never a comparison input.
pub fn domain_metadata(paths: &[Utf8PathBuf]) -> anyhow::Result<(i64, i64)> {
    let mut size_bytes = 0i64;
    let mut mtime_ns = 0i64;
    for path in paths {
        match std::fs::metadata(path) {
            Ok(metadata) => {
                size_bytes += i64::try_from(metadata.len())?;
                let mtime = metadata
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| {
                        i64::try_from(duration.as_nanos()).unwrap_or(i64::MAX)
                    });
                mtime_ns = mtime_ns.max(mtime);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok((size_bytes, mtime_ns))
}
