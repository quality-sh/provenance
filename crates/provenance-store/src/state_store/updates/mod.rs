//! Record edits retain canonical identities and use the existing publication writer.
mod descriptions;
mod inputs;
mod knowledge;
mod resolutions;
mod shaping;

use crate::write_error::{SourceFailure, WriteFailure};
pub use inputs::*;
use provenance_core::model::relations::RelationOwner;
use provenance_core::ScopeId;

fn invalid(message: &str) -> anyhow::Error {
    SourceFailure::wrap(WriteFailure::InvalidUpdate, anyhow::anyhow!("{message}"))
}

fn owner_matches(saved: Option<&str>, supplied: Option<&str>) -> anyhow::Result<()> {
    if saved != supplied {
        return Err(SourceFailure::wrap(
            WriteFailure::RecordOwnershipConflict,
            anyhow::anyhow!(
                "declared_by must match the existing owner; omit it for a manual record"
            ),
        ));
    }
    Ok(())
}

fn required_text(value: &str) -> anyhow::Result<()> {
    if value.trim().is_empty() {
        return Err(invalid("required text must not be blank"));
    }
    Ok(())
}

fn optional<T>(target: &mut Option<T>, value: Option<T>, clear: bool) -> anyhow::Result<()> {
    if clear && value.is_some() {
        return Err(invalid(
            "a field cannot be set and cleared in the same update",
        ));
    }
    if clear {
        *target = None;
    } else if let Some(value) = value {
        *target = Some(value);
    }
    Ok(())
}

fn set<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

fn missing() -> anyhow::Error {
    SourceFailure::wrap(
        WriteFailure::MissingReference,
        anyhow::anyhow!("record does not exist"),
    )
}

fn validate_final_relations<T: RelationOwner>(
    store: &crate::state_store::StateStore,
    scope: &ScopeId,
    record: &T,
) -> anyhow::Result<()> {
    for (name, target) in record.references() {
        let declaration = T::relations()
            .iter()
            .find(|declaration| declaration.name == name)
            .expect("a record reference has a relation declaration");
        store.ensure_node_exists(scope, declaration.target, target, name)?;
    }
    store.validate_graph_scope(scope)
}
