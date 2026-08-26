//! The macro projection over the string-keyed kernel handles.

use provenance_macros::verifies;
use provenance_sdk::{provenance_spec, requirement, rule};

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
