//! The record kind that action help shows fields for.
//!
//! Help reads the canonical record files under the store's read-only marker.
//! The marker skips the publication lock and recovery, so help never waits
//! for another command and never writes to the repository.

use super::grammar::TargetVerb;
use provenance_core::{NodeType, ScopeId};
use provenance_porcelain::action::Action;
use provenance_store::layout::ProvenanceLayout;
use provenance_store::state_store::StateStore;

/// The kind of an existing target, or `None` when the read cannot give one
/// kind. Help then lists the fields of every record type.
#[provenance_macros::rule("rule_record_help_takes_no_lock")]
pub(super) fn resolve_record_kind(
    selection: &clap::ArgMatches,
    action: Option<TargetVerb>,
) -> Option<NodeType> {
    let Some(TargetVerb::Action(action)) = action else {
        return None;
    };
    if action == Action::Create
        || !provenance_store::operations::catalog::TargetAction::RECORD.contains(&action)
    {
        return None;
    }
    let target = selection.get_one::<String>("target")?;
    let repo = selection.get_one::<String>("repo")?;
    let scope = ScopeId::new(selection.get_one::<String>("scope")?).ok()?;
    // Opening a repository takes the publication lock, so help reads the
    // record files through the layout instead.
    let layout = ProvenanceLayout::new(repo.as_str());
    let store = StateStore::new(layout.clone());
    provenance_store::publication::with_read_only_validation(&layout, || {
        let mut kinds = Vec::new();
        for kind in NodeType::ALL {
            if holds(&store, &scope, kind, target)? {
                kinds.push(kind);
            }
        }
        Ok(kinds)
    })
    .ok()
    .and_then(|kinds| match kinds.as_slice() {
        [kind] => Some(*kind),
        _ => None,
    })
}

fn holds(store: &StateStore, scope: &ScopeId, kind: NodeType, id: &str) -> anyhow::Result<bool> {
    macro_rules! any_id {
        ($list:ident) => {
            store
                .$list(scope)?
                .iter()
                .any(|record| record.id.as_str() == id)
        };
    }
    Ok(match kind {
        NodeType::Source => any_id!(list_sources),
        NodeType::Requirement => any_id!(list_requirements),
        NodeType::Resolution => any_id!(list_resolutions),
        NodeType::Rule => any_id!(list_rules),
        NodeType::Topic => any_id!(list_topics),
        NodeType::Question => any_id!(list_questions),
        NodeType::Domain => any_id!(list_domains),
        NodeType::Boundary => any_id!(list_boundaries),
    })
}
