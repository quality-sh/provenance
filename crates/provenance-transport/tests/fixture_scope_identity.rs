//! Repository-wide record identity holds for every fixture repository.
#![cfg(feature = "test-fixture")]

use provenance_core::ScopeId;
use provenance_macros::verifies;
use provenance_transport::fixture::records::Repository;

#[test]
#[verifies("rule_porcelain_id_unique_in_repository", examples)]
fn add_scope_keeps_record_ids_unique_across_the_repository() {
    let repository = Repository::new("The shared graph is readable.");
    repository.add_scope("other", "The other graph is readable.");

    let store = provenance_store::state_store::StateStore::new(repository.layout.clone());
    let manifest = store.manifest().unwrap();
    store
        .validate_canonical_ids_unique(&manifest.scopes)
        .unwrap();

    let rules = std::fs::read_to_string(provenance_store::shards::rules_path(
        &repository.layout,
        &ScopeId::new("other").unwrap(),
    ))
    .unwrap();
    assert!(
        rules.contains("\"id\":\"rule_other\""),
        "the added scope must seed its own records: {rules}"
    );
}
