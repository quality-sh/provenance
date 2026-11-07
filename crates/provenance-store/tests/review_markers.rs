#[allow(dead_code)]
mod review_support;

use review_support::*;

/// Implementation aid: the regenerated client has no snapshot field.
#[test]
fn edit_state_omits_snapshot() {
    let (_temp, store) = fixture();
    let state = store.requirement_edit_state(&scope(), &id()).unwrap();
    let value = serde_json::to_value(state).unwrap();
    assert!(value.get("snapshot").is_none());
}
