//! The directories below a scope directory that the reader of each family
//! reads. A catch-up parses again only the families that a write can
//! change.
//!
//! A family reader can read more than its own shard. The proposal cards
//! take their state from the assertions and the dispositions. A Discussion is checked
//! against its Thread, its Messages and its parent record. Each family names
//! the shards that its reader reads, and the directories come from the
//! shard paths in [`crate::shards`]. The reader gets a store fenced to
//! these directories, so a read outside them fails the parse.

use crate::cache::ProjectionFamily;
use crate::shards;
use provenance_core::ScopeId;
use std::collections::BTreeSet;

/// The families whose shards the reader of `family` reads. The match names
/// each family, so a new family must name its shards.
fn inputs(family: ProjectionFamily) -> Vec<ProjectionFamily> {
    use ProjectionFamily as F;
    match family {
        F::ProposalCards => vec![F::ProposalCards, F::AssertionRecords, F::Dispositions],
        F::Discussions => vec![
            F::Discussions,
            F::Threads,
            F::Messages,
            F::Sources,
            F::Requirements,
            F::Resolutions,
            F::Rules,
            F::Topics,
            F::Questions,
        ],
        F::Contributions
        | F::SynthesisPackets
        | F::AssertionRecords
        | F::Dispositions
        | F::Sources
        | F::Requirements
        | F::RequirementReviews
        | F::Topics
        | F::Questions
        | F::Resolutions
        | F::Rules
        | F::Threads
        | F::Messages
        | F::Withdrawals
        | F::ImplementationBindings
        | F::VerificationBindings => vec![family],
    }
}

/// The directories below a scope directory that the reader of `family`
/// reads.
pub(super) fn input_directories(family: ProjectionFamily) -> Vec<&'static str> {
    let directories: BTreeSet<_> = inputs(family)
        .into_iter()
        .map(|input| shards::shard_directory(input.shard_suffix()))
        .collect();
    directories.into_iter().collect()
}

/// The families whose records can differ after the files at `changed`,
/// paths below the state directory, changed. `None` means every family: a
/// changed file outside every named directory can belong to any of them.
pub(super) fn affected(
    scope: &ScopeId,
    changed: &BTreeSet<String>,
) -> Option<Vec<ProjectionFamily>> {
    let prefix = format!("scopes/{}/", scope.as_str());
    let mut directories = BTreeSet::new();
    for path in changed {
        let (directory, _) = path.strip_prefix(&prefix)?.split_once('/')?;
        directories.insert(directory);
    }
    let read: Vec<_> = ProjectionFamily::ALL
        .into_iter()
        .map(|family| (family, input_directories(family)))
        .collect();
    let claimed = |directory: &&str| read.iter().any(|(_, inputs)| inputs.contains(directory));
    if !directories.iter().all(claimed) {
        return None;
    }
    Some(
        read.into_iter()
            .filter(|(_, inputs)| {
                inputs
                    .iter()
                    .any(|directory| directories.contains(directory))
            })
            .map(|(family, _)| family)
            .collect(),
    )
}
