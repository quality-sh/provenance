use super::assert_kind_round_trips;
use crate::model::{
    ImplementationBinding, RequirementReview, ScopeId, StableId, VerificationBinding,
    VerificationMethod,
};
use crate::SUPPORTED_SCHEMA_VERSION;

fn sid(value: &str) -> StableId {
    StableId::new(value).unwrap()
}

fn scope() -> ScopeId {
    ScopeId::new("default").unwrap()
}

fn filled_implementation_binding() -> ImplementationBinding {
    ImplementationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("bind_impl_a"),
        rule_id: sid("rule_overtime_001"),
        declared_by: "spec://pay".into(),
        retired: true,
        file: "src/pay.rs".into(),
        symbol: "pay".into(),
    }
}

fn bare_implementation_binding() -> ImplementationBinding {
    ImplementationBinding {
        retired: false,
        ..filled_implementation_binding()
    }
}

fn filled_verification_binding() -> VerificationBinding {
    VerificationBinding {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("bind_ver_a"),
        rule_id: sid("rule_overtime_001"),
        key: "pay_examples".into(),
        method: VerificationMethod::Examples,
        declared_by: "spec://pay".into(),
        retired: true,
        file: "tests/pay.rs".into(),
        symbol: Some("pay_examples".into()),
    }
}

fn bare_verification_binding() -> VerificationBinding {
    VerificationBinding {
        retired: false,
        symbol: None,
        ..filled_verification_binding()
    }
}

fn filled_review() -> RequirementReview {
    RequirementReview {
        schema_version: SUPPORTED_SCHEMA_VERSION,
        scope_id: scope(),
        id: sid("review_a"),
        rule_id: sid("rule_overtime_001"),
        requirement_id: sid("req_overtime"),
        field: "statement".into(),
        before: "Overtime".into(),
        after: "Overtime pay".into(),
        changed_at: 1_700_000_000,
        cleared_at: Some(1_700_000_500),
        cleared_by_run: Some(sid("run_cleared")),
    }
}

fn bare_review() -> RequirementReview {
    RequirementReview {
        cleared_at: None,
        cleared_by_run: None,
        ..filled_review()
    }
}

#[test]
fn an_implementation_binding_round_trips_through_its_row() {
    assert_kind_round_trips(
        &filled_implementation_binding(),
        &bare_implementation_binding(),
    );
}

#[test]
fn a_verification_binding_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_verification_binding(), &bare_verification_binding());
}

#[test]
fn a_requirement_review_round_trips_through_its_row() {
    assert_kind_round_trips(&filled_review(), &bare_review());
}
