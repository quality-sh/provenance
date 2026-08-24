//! Workspace-wide rule-binding integrity after the store relocation (V10).
//!
//! The CLI SDK operations moved into provenance-store; every #[rule] id
//! that moved must still have a binding site the scanner discovers.

use provenance_macros::verifies;
use provenance_scanner::{source_sites, SourceSiteRole};

#[test]
#[verifies("rule_rust_binding_scan_follows_store_relocation", examples)]
fn moved_rule_ids_keep_discovered_binding_sites() {
    let workspace = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize_utf8()
        .unwrap();
    let scans = provenance_scanner::scan_path(&workspace).unwrap();
    for id in [
        "rule_sdk_project_discovery",
        "rule_ste_sdk_statement_request_schema",
        "rule_ste_sdk_statement_report",
        "rule_ste_sdk_statement_repository_independence",
        "rule_rust_authoring_kernel_is_pure",
        "rule_rust_authored_documents_are_canonical",
        "rule_rust_build_text_checks_trim",
        "rule_rust_store_owns_persistent_identity",
    ] {
        assert!(
            source_sites(&scans).any(|site| {
                site.rule_id() == id && site.role() == SourceSiteRole::Implementation
            }),
            "rule id `{id}` has no discovered implementation binding site"
        );
    }
}
