//! The directories below a scope directory that each family's records come
//! from, so a catch-up reads again only the families a write can change.
//!
//! A family reader can read more than its own shard: the ideation readers
//! add the records of the landings shard, the proposal cards take their
//! state from the assertions and the dispositions, and a Discussion is
//! checked against its parent record. Each family names every directory
//! that its reader reads. A test records the reads of each reader and
//! refuses a read outside the named directories.

use crate::cache::ProjectionFamily;
use provenance_core::ScopeId;
use std::collections::BTreeSet;

/// The directories of the six kinds whose records a Discussion can have as
/// its parent.
const PARENTS: [&str; 6] = [
    "sources",
    "requirements",
    "resolutions",
    "rules",
    "topics",
    "questions",
];

pub const fn input_directories(family: ProjectionFamily) -> &'static [&'static str] {
    match family {
        ProjectionFamily::Sources => &["sources"],
        ProjectionFamily::Requirements | ProjectionFamily::RequirementReviews => &["requirements"],
        ProjectionFamily::Topics => &["topics"],
        ProjectionFamily::Questions => &["questions"],
        ProjectionFamily::Resolutions => &["resolutions"],
        ProjectionFamily::Rules => &["rules"],
        ProjectionFamily::Threads | ProjectionFamily::Messages => &["threads"],
        ProjectionFamily::Contributions
        | ProjectionFamily::SynthesisPackets
        | ProjectionFamily::ProposalCards
        | ProjectionFamily::AssertionRecords
        | ProjectionFamily::Dispositions
        | ProjectionFamily::Withdrawals => &["ideation"],
        ProjectionFamily::ImplementationBindings => &["implementations"],
        ProjectionFamily::VerificationBindings => &["verifications"],
        ProjectionFamily::Discussions => &[
            "threads", PARENTS[0], PARENTS[1], PARENTS[2], PARENTS[3], PARENTS[4], PARENTS[5],
        ],
    }
}

/// The families whose records can differ after the files at `changed`,
/// paths below the state directory, changed. `None` means every family: a
/// changed file outside every named directory can belong to any of them.
pub fn affected(scope: &ScopeId, changed: &BTreeSet<String>) -> Option<Vec<ProjectionFamily>> {
    let prefix = format!("scopes/{}/", scope.as_str());
    let mut directories = BTreeSet::new();
    for path in changed {
        let (directory, _) = path.strip_prefix(&prefix)?.split_once('/')?;
        directories.insert(directory);
    }
    let claimed = |directory: &&str| {
        ProjectionFamily::ALL
            .into_iter()
            .any(|family| input_directories(family).contains(directory))
    };
    if !directories.iter().all(claimed) {
        return None;
    }
    Some(
        ProjectionFamily::ALL
            .into_iter()
            .filter(|family| {
                input_directories(*family)
                    .iter()
                    .any(|directory| directories.contains(directory))
            })
            .collect(),
    )
}
