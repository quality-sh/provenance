//! Assigns an outcome to the first record version that contains it. A
//! committed Discussion line that does not parse holds no outcome.

use super::texts::each_text;
use super::versions::FileVersions;
use super::VersionedRecord;
use provenance_core::{
    threads::{Discussion, DiscussionOrigin},
    NodeType, StableId,
};
use std::collections::BTreeSet;

/// One outcome: its Discussion, message and revision.
pub(super) type Outcome = (String, String, String);

/// Assigns the outcomes of the committed Discussion file `path` to the
/// committed `versions`, oldest first. `seen` gets each outcome of a version.
pub(super) async fn committed(
    history: &dyn FileVersions,
    path: &str,
    kind: NodeType,
    id: &StableId,
    versions: &mut [VersionedRecord],
    seen: &mut BTreeSet<Outcome>,
) -> anyhow::Result<()> {
    let commits = versions
        .iter()
        .filter_map(|version| version.version.commit.clone())
        .collect::<Vec<_>>();
    let commits = commits.iter().map(String::as_str).collect::<Vec<_>>();
    let mut versions = versions.iter_mut();
    each_text(history, path, &commits, |text| {
        let discussions = text
            .unwrap_or_default()
            .lines()
            .filter_map(|line| serde_json::from_str::<Discussion>(line).ok())
            .collect();
        if let Some(version) = versions.next() {
            assign(discussions, kind, id, seen, version);
        }
    })
    .await
}

/// Assigns the outcomes of the Discussions of the saved working copy to its
/// version, except an outcome that a committed version holds.
pub(super) fn working(
    discussions: Vec<Discussion>,
    kind: NodeType,
    id: &StableId,
    seen: &BTreeSet<Outcome>,
    version: &mut VersionedRecord,
) {
    assign(discussions, kind, id, &mut seen.clone(), version);
}

fn assign(
    discussions: Vec<Discussion>,
    kind: NodeType,
    id: &StableId,
    seen: &mut BTreeSet<Outcome>,
    version: &mut VersionedRecord,
) {
    for discussion in discussions {
        for outcome in discussion.outcomes {
            if outcome.record_kind != kind || outcome.record_id != *id {
                continue;
            }
            let key = (
                discussion.discussion_id.as_str().to_owned(),
                outcome.message_id.as_str().to_owned(),
                outcome.revision.as_str().to_owned(),
            );
            if seen.insert(key) {
                version.version.origin = Some(DiscussionOrigin {
                    discussion_id: discussion.discussion_id.clone(),
                    thread_id: discussion.thread_id.clone(),
                    message_id: outcome.message_id,
                });
            }
        }
    }
}
