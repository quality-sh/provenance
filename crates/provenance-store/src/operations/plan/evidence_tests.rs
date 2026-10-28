use super::reviews;
use crate::layout::ProvenanceLayout;
use crate::state_store::{StateStore, TypedSpecInput};
use provenance_core::{Manifest, RepoPathPrefix, ScopeId, SUPPORTED_SCHEMA_VERSION};
use serde_json::json;

fn store() -> (tempfile::TempDir, StateStore, ScopeId) {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().to_path_buf()).unwrap();
    let layout = ProvenanceLayout::new(root);
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    let scope = ScopeId::new("default").unwrap();
    let manifest = Manifest::default_with_scope(scope.clone(), RepoPathPrefix::new("."));
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&manifest).unwrap(),
    )
    .unwrap();
    (dir, StateStore::new(layout), scope)
}

fn spec(requirement: &str) -> TypedSpecInput {
    serde_json::from_value(json!({
        "schema_version": SUPPORTED_SCHEMA_VERSION.0,
        "spec": "evidence",
        "declared_by": "spec://typescript/evidence",
        "requirements": [{"key": "sharing", "statement": requirement}],
        "rules": [{"key": "expiry", "requirement": "sharing", "statement": "Clean rule"}]
    }))
    .unwrap()
}

#[test]
fn a_planned_restatement_puts_the_rules_of_the_requirement_up_for_review() {
    let (_dir, store, scope) = store();
    store
        .apply_typed_spec(&scope, spec("Clean requirement"))
        .unwrap();
    let plan = store
        .plan_typed_spec(&scope, spec("Changed requirement"))
        .unwrap();

    let reviews = reviews(&store, &scope, &plan).unwrap();

    assert_eq!(reviews.rules.len(), 1, "the one Rule of the Requirement");
    let evidence = reviews.evidence(&reviews.rules[0]);
    assert!(evidence.review_required());
    let [reason] = evidence.reasons() else {
        panic!("one planned reason: {:?}", evidence.reasons());
    };
    assert_eq!(reason.field, "statement");
    assert_eq!(reason.before, "Clean requirement");
    assert_eq!(reason.after, "Changed requirement");
    assert_eq!(reason.changed_at, None, "a planned change has no time yet");
}

#[test]
fn an_applied_restatement_stays_on_file_as_a_recorded_review() {
    let (_dir, store, scope) = store();
    store
        .apply_typed_spec(&scope, spec("Clean requirement"))
        .unwrap();
    store
        .apply_typed_spec(&scope, spec("Changed requirement"))
        .unwrap();
    let plan = store
        .plan_typed_spec(&scope, spec("Changed requirement"))
        .unwrap();

    let reviews = reviews(&store, &scope, &plan).unwrap();

    assert_eq!(reviews.rules.len(), 1, "the one Rule of the Requirement");
    let evidence = reviews.evidence(&reviews.rules[0]);
    let [reason] = evidence.reasons() else {
        panic!("one recorded reason: {:?}", evidence.reasons());
    };
    assert_eq!(reason.before, "Clean requirement");
    assert_eq!(reason.after, "Changed requirement");
    assert!(
        reason.changed_at.is_some(),
        "a recorded review keeps its time"
    );
}
