use provenance_core::protocol::TypedImplementationInput;
use provenance_core::{ImplementationBinding, StableId};

pub(super) fn matches(
    rule_id: &StableId,
    desired: Option<&TypedImplementationInput>,
    current: &[ImplementationBinding],
    owner: &str,
) -> bool {
    let Some(desired) = desired else {
        return true;
    };
    current.iter().any(|binding| {
        !binding.retired
            && binding.rule_id == *rule_id
            && binding.declared_by == owner
            && binding.file == desired.file
            && binding.symbol == desired.symbol
    })
}

pub(super) fn current_value(
    rule_id: &StableId,
    current: &[ImplementationBinding],
) -> serde_json::Value {
    current
        .iter()
        .find(|binding| !binding.retired && binding.rule_id == *rule_id)
        .map_or(
            serde_json::Value::Null,
            |binding| serde_json::json!({ "file": binding.file, "symbol": binding.symbol }),
        )
}
