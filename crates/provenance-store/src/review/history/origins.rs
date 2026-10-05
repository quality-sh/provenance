//! Assigns an outcome to its first matching record version.

use super::{git, VersionedRecord};
use camino::Utf8Path;
use provenance_core::{
    threads::{Discussion, DiscussionOrigin},
    NodeType, StableId,
};
use std::collections::BTreeSet;

pub(super) fn assign(
    root: &Utf8Path,
    path: &Utf8Path,
    working: &[Discussion],
    kind: NodeType,
    id: &StableId,
    versions: &mut [VersionedRecord],
) -> anyhow::Result<()> {
    let commits = versions
        .iter()
        .filter_map(|version| version.version.commit.as_deref())
        .collect::<Vec<_>>();
    let mut committed = git::file_at_commits(root, path, &commits)?.into_iter();
    let mut seen = BTreeSet::new();
    let mut pending = Vec::new();
    for version in versions {
        let discussions = if version.version.commit.is_some() {
            committed
                .next()
                .flatten()
                .unwrap_or_default()
                .lines()
                .filter(|line| !line.trim().is_empty())
                .map(serde_json::from_str::<Discussion>)
                .collect::<Result<Vec<_>, _>>()?
        } else {
            working.to_vec()
        };
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
                    pending.push((
                        outcome.revision,
                        DiscussionOrigin {
                            discussion_id: discussion.discussion_id.clone(),
                            thread_id: discussion.thread_id.clone(),
                            message_id: outcome.message_id,
                        },
                    ));
                }
            }
        }
        version.version.origin = pending
            .iter()
            .find(|(revision, _)| *revision == version.version.revision)
            .map(|(_, origin)| origin.clone());
        pending.retain(|(revision, _)| *revision != version.version.revision);
    }
    Ok(())
}
