//! The review state of one document, kept for each projection revision.
//!
//! Each document page needs the review state of the whole document: the
//! totals count every member, the terminal filter hides each rejected new
//! record, and the cursor binds the state's digest. The state reads
//! canonical shards, so the stamp does not cover it. After a catch-up, or
//! after `refuse_stale` found no change, one projection revision stands for
//! one content of the canonical units, so the cursor pages of one revision
//! share one state. A read under another freshness policy computes the
//! state again.

use crate::operations::reader::{Live, ReadContext};
use crate::review::DocumentReviewState;
use camino::Utf8PathBuf;
use provenance_core::{NodeType, ScopeId, StableId};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};

/// The number of document states the process keeps, most recent first.
const KEPT: usize = 8;

#[derive(PartialEq, Eq)]
struct Key {
    root: Utf8PathBuf,
    scope: ScopeId,
    instance_id: String,
    serial: i64,
    digest: String,
    records: Vec<(NodeType, StableId)>,
}

static STATES: Mutex<VecDeque<(Key, Arc<DocumentReviewState>)>> = Mutex::new(VecDeque::new());

/// The review state of the document's records at the read's revision.
pub(super) fn document_review_state(
    ctx: &ReadContext,
    records: Vec<(NodeType, StableId)>,
) -> anyhow::Result<Arc<DocumentReviewState>> {
    let store = ctx.live(Live::Canonical).store();
    let scope = ctx.snapshot().scope();
    if !ctx.matches_canonical() {
        return Ok(Arc::new(store.document_review_state(scope, &records)?));
    }
    let key = Key {
        root: ctx.repository().layout().root().to_owned(),
        scope: scope.clone(),
        instance_id: ctx.snapshot().instance_id().to_owned(),
        serial: ctx.snapshot().serial(),
        digest: ctx.snapshot().digest().to_owned(),
        records,
    };
    if let Some(state) = kept(&key) {
        return Ok(state);
    }
    let state = Arc::new(store.document_review_state(scope, &key.records)?);
    keep(key, Arc::clone(&state));
    Ok(state)
}

fn kept(key: &Key) -> Option<Arc<DocumentReviewState>> {
    let mut states = STATES.lock().unwrap_or_else(PoisonError::into_inner);
    let position = states.iter().position(|(kept, _)| kept == key)?;
    let entry = states.remove(position)?;
    let state = Arc::clone(&entry.1);
    states.push_front(entry);
    drop(states);
    Some(state)
}

fn keep(key: Key, state: Arc<DocumentReviewState>) {
    let mut states = STATES.lock().unwrap_or_else(PoisonError::into_inner);
    states.retain(|(kept, _)| *kept != key);
    states.push_front((key, state));
    states.truncate(KEPT);
}
