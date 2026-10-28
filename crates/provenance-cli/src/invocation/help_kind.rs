//! The record kind that action help shows fields for.
//!
//! Help asks the store for the kinds under its read-only marker.
//! The marker skips the publication lock and recovery, so help never waits
//! for another command and never writes to the repository.

use super::grammar::TargetVerb;
use provenance_core::{NodeType, ScopeId};
use provenance_porcelain::action::Action;
use provenance_store::layout::ProvenanceLayout;

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
    // record kinds through the layout instead.
    let layout = ProvenanceLayout::new(repo.as_str());
    provenance_store::publication::read_only_record_kinds(&layout, &scope, target)
        .ok()
        .and_then(|kinds| match kinds.as_slice() {
            [kind] => Some(*kind),
            _ => None,
        })
}
