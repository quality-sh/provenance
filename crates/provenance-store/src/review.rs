//! Recoverable Requirement edits and immutable evidence.
mod classifier;
pub(crate) mod guard;
mod input;
mod journal;
mod relationships;
mod save;
pub use input::{RequirementRelations, SaveRequirement};

fn owner_matches(record: &provenance_core::Requirement, owner: Option<&str>) -> anyhow::Result<()> {
    if record.declared_by.as_deref() != owner {
        return Err(crate::write_error::SourceFailure::wrap(
            crate::write_error::WriteFailure::RecordOwnershipConflict,
            anyhow::anyhow!("declared_by must match the existing owner"),
        ));
    }
    Ok(())
}

pub(crate) mod cache;

mod reads;
pub use reads::{read_evidence, read_history};

mod snapshot;

#[cfg(test)]
mod recovery_tests;

#[cfg(all(test, any(unix, windows)))]
mod path_tests;
