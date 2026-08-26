//! The macro projection over the string-keyed kernel handles.

use provenance_macros::verifies;
use provenance_sdk::{provenance_spec, requirement, rule};

#[test]
fn implemented_by_has_no_public_path_joining_helper() {
    let crate_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let interface = std::fs::read_to_string(crate_root.join("src/lib.rs")).unwrap();
    let implementation = std::fs::read_to_string(crate_root.join("src/macros.rs")).unwrap();

    assert!(!interface.contains("implemented_by_package_path"));
    assert!(!implementation.contains("pub fn implemented_by_package_path"));
}

provenance_spec!(share_links => "share-links" {
    requirement("sharing")
        .statement("Users can securely share documentation")
        .rules([rule("expiry").statement("Share links expire within 30 days")]),
});

#[test]
fn the_projected_spec_builds_and_projects_handles() {
    let document = share_links().unwrap();
    assert_eq!(document.spec(), "share-links");
    let handles = document.handles();
    let expiry = handles
        .requirement("sharing")
        .unwrap()
        .rule("expiry")
        .unwrap();
    assert_eq!(
        expiry.address.segments(),
        ["share-links", "requirement", "sharing", "rule", "expiry"]
    );
}

#[test]
#[verifies("rule_nested_rust_implementation_path", examples)]
fn implemented_by_records_the_site() {
    let document = provenance_sdk::spec("share-links")
        .requirements([requirement("sharing")
            .statement("Users can securely share documentation")
            .rules([provenance_sdk::implemented_by!(
                rule("expiry").statement("Share links expire within 30 days"),
                "src/verify.rs",
                verify
            )])])
        .build()
        .unwrap();
    let implementation = document.rules()[0].implementation.as_ref().unwrap();
    assert_eq!(
        implementation.file,
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/verify.rs")
    );
    assert_eq!(implementation.symbol, "verify");
}
