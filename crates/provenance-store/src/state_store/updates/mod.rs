//! Record edits retain canonical identities and use the existing publication writer.
mod archive;
mod descriptions;
mod inputs;
mod knowledge;
mod resolutions;
mod shaping;

use crate::write_error::WriteFailure;
pub use inputs::*;

fn invalid(message: &str) -> anyhow::Error {
    crate::refusal::refuse!(InvalidUpdate, "{message}")
}

fn owner_matches(saved: Option<&str>, supplied: Option<&str>) -> anyhow::Result<()> {
    if saved != supplied {
        return Err(crate::refusal::refuse!(
            (WriteFailure::record_ownership_conflict(saved)),
            "declared_by must match the existing owner; omit it for a manual record"
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
    crate::refusal::refuse!(MissingReference, "record does not exist")
}
