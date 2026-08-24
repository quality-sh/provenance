//! The facade adds no semantic layer: authoring goes through the
//! kernel, and an unresolved implementation target refuses apply
//! before any state is written.

use provenance_core::{Manifest, RepoPathPrefix, ScopeId};
use provenance_macros::verifies;
use provenance_sdk::{operations, requirement, rule, spec};
use provenance_store::layout::ProvenanceLayout;
use provenance_store::state_store::StateStore;

fn repository() -> (tempfile::TempDir, camino::Utf8PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = camino::Utf8PathBuf::from_path_buf(dir.path().canonicalize().unwrap()).unwrap();
    let layout = ProvenanceLayout::new(root.clone());
    std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
    std::fs::write(
        layout.manifest_path(),
        serde_json::to_string(&Manifest::default_with_scope(
            ScopeId::new("default").unwrap(),
            RepoPathPrefix::new("."),
        ))
        .unwrap(),
    )
    .unwrap();
    (dir, root)
}

#[test]
#[verifies("rule_rust_sdk_facade_delegates_semantics", conformance)]
fn facade_authoring_is_the_kernel_byte_for_byte() {
    let through_facade = spec("share-links")
        .requirements([requirement("sharing")
            .statement("Users can securely share documentation")
            .rules([rule("expiry").statement("Share links expire within 30 days")])])
        .build()
        .unwrap()
        .materialize("spec://rust");
    let through_kernel = provenance_core::authoring::spec("share-links")
        .requirements([provenance_core::authoring::requirement("sharing")
            .statement("Users can securely share documentation")
            .rules([provenance_core::authoring::rule("expiry")
                .statement("Share links expire within 30 days")])])
        .build()
        .unwrap()
        .materialize("spec://rust");

    assert_eq!(
        serde_json::to_string(&through_facade).unwrap(),
        serde_json::to_string(&through_kernel).unwrap()
    );
}

#[test]
#[verifies("rule_rust_store_resolves_implementation_symbols", examples)]
#[verifies("rule_rust_symbol_resolution_refuses_writes", examples)]
fn an_unresolved_implementation_target_refuses_apply_without_writes() {
    let (_dir, root) = repository();
    let document = spec("share-links")
        .requirements([requirement("sharing")
            .statement("Users can securely share documentation")
            .rules([rule("expiry")
                .statement("Share links expire within 30 days")
                .implemented_at("src/absent.rs", "create_share_link")])])
        .build()
        .unwrap();

    let error = operations::apply(
        Some(root.clone()),
        &ScopeId::new("default").unwrap(),
        document.materialize("spec://rust"),
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("implementation target `src/absent.rs` does not exist"),
        "unexpected refusal: {error}"
    );
    let store = StateStore::new(ProvenanceLayout::new(root));
    assert!(store
        .list_rules(&ScopeId::new("default").unwrap())
        .unwrap()
        .is_empty());
}
