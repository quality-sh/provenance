//! Assigns an outcome to the first record version that contains it. A
//! committed Discussion line that does not parse holds no outcome.

use super::index::OutcomeRow;
use super::VersionedRecord;
use provenance_core::{
    threads::{Discussion, DiscussionOrigin},
    NodeType, StableId,
};
use std::collections::{BTreeMap, BTreeSet};

/// One outcome: its Discussion, message and revision.
pub(super) type Outcome = (String, String, String);

/// The outcomes for each record of `kind` in the text of a committed
/// Discussion file, by record id, in file order.
pub(super) fn outcomes(text: Option<&str>, kind: NodeType) -> BTreeMap<String, Vec<OutcomeRow>> {
    let mut found = BTreeMap::<String, Vec<OutcomeRow>>::new();
    for line in text.unwrap_or_default().lines() {
        let Ok(discussion) = serde_json::from_str::<Discussion>(line) else {
            continue;
        };
        for outcome in discussion.outcomes {
            if outcome.record_kind != kind {
                continue;
            }
            found
                .entry(outcome.record_id.as_str().to_owned())
                .or_default()
                .push(OutcomeRow {
                    discussion_id: discussion.discussion_id.clone(),
                    thread_id: discussion.thread_id.clone(),
                    message_id: outcome.message_id,
                    revision: outcome.revision.as_str().to_owned(),
                });
        }
    }
    found
}

/// Assigns `outcomes`, the outcomes of a committed version, to `version`,
/// except an outcome that an earlier version holds. `seen` gets each
/// outcome of the version.
pub(super) fn committed(
    outcomes: &[OutcomeRow],
    seen: &mut BTreeSet<Outcome>,
    version: &mut VersionedRecord,
) {
    for outcome in outcomes {
        let key = (
            outcome.discussion_id.as_str().to_owned(),
            outcome.message_id.as_str().to_owned(),
            outcome.revision.clone(),
        );
        if seen.insert(key) {
            version.version.origin = Some(DiscussionOrigin {
                discussion_id: outcome.discussion_id.clone(),
                thread_id: outcome.thread_id.clone(),
                message_id: outcome.message_id.clone(),
            });
        }
    }
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
