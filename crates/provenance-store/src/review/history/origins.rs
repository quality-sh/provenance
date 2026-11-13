//! Assigns an outcome to the first record version that contains it. A
//! committed Discussion line that does not parse holds no outcome.

use super::versions::FileVersions;
use super::VersionedRecord;
use provenance_core::{
    threads::{Discussion, DiscussionOrigin},
    NodeType, StableId,
};
use std::collections::BTreeSet;

/// `path` names the Discussion file; `working` holds the Discussions of the
/// saved working copy.
pub(super) async fn assign(
    history: &dyn FileVersions,
    path: &str,
    working: &[Discussion],
    kind: NodeType,
    id: &StableId,
    versions: &mut [VersionedRecord],
) -> anyhow::Result<()> {
    let mut seen = BTreeSet::new();
    for version in versions {
        let discussions = match &version.version.commit {
            Some(commit) => history
                .text(path, commit)
                .await?
                .unwrap_or_default()
                .lines()
                .filter_map(|line| serde_json::from_str::<Discussion>(line).ok())
                .collect::<Vec<_>>(),
            None => working.to_vec(),
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
                    version.version.origin = Some(DiscussionOrigin {
                        discussion_id: discussion.discussion_id.clone(),
                        thread_id: discussion.thread_id.clone(),
                        message_id: outcome.message_id,
                    });
                }
            }
        }
    }
    Ok(())
}
