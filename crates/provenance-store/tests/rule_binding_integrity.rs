//! Workspace-wide Rule binding integrity.
//!
//! Each listed Rule id has an implementation binding site that the scanner
//! finds in the workspace.

use provenance_scanner::{source_sites, SourceSiteRole};

#[test]
fn listed_rule_ids_have_discovered_binding_sites() {
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
