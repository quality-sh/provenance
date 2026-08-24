use provenance_macros::verifies;

use super::{requirement, rule, source, spec};

fn sample(order: bool) -> super::SpecBuilder {
    let first = requirement("sharing")
        .statement("Users can securely share documentation")
        .from(source("sharing-policy").document("docs/sharing-policy.md"))
        .rules([
            rule("expiry")
                .statement("Share links must expire within 30 days")
                .implemented_at("src/share_links.rs", "create_share_link"),
            rule("audit")
                .statement("Share-link access is audited")
                .id("rule_share_link_audit"),
        ]);
    let second = requirement("Übersicht")
        .statement("Readers get an overview page")
        .rules([rule("overview").statement("The overview lists every share link")]);
    if order {
        spec("share-links").requirements([first, second])
    } else {
        spec("share-links").requirements([second, first])
    }
}

#[test]
#[verifies("rule_rust_authored_documents_are_canonical", property)]
fn declaration_order_never_reaches_the_canonical_document() {
    let forward = sample(true).build().unwrap().materialize("spec://rust");
    let reversed = sample(false).build().unwrap().materialize("spec://rust");
    assert_eq!(
        serde_json::to_string(&forward).unwrap(),
        serde_json::to_string(&reversed).unwrap()
    );
}

#[test]
#[verifies("rule_rust_authored_documents_are_canonical", examples)]
fn canonical_order_is_utf8_byte_order_not_collation() {
    let document = spec("keys")
        .requirements([
            requirement("b-lower").statement("Statement one"),
            requirement("A-upper").statement("Statement two"),
            requirement("ä-umlaut").statement("Statement three"),
        ])
        .build()
        .unwrap();
    let keys = document
        .requirements()
        .iter()
        .map(|requirement| requirement.key.as_str())
        .collect::<Vec<_>>();
    assert_eq!(keys, ["A-upper", "b-lower", "ä-umlaut"]);
}

#[test]
fn rules_sort_by_serialized_address() {
    let document = sample(true).build().unwrap();
    let addresses = document
        .rules()
        .iter()
        .map(|rule| rule.address.as_ref().unwrap().segments().join("/"))
        .collect::<Vec<_>>();
    let mut sorted = addresses.clone();
    sorted.sort();
    assert_eq!(addresses, sorted);
}

#[test]
fn a_multi_owner_rule_takes_the_shared_address() {
    let document = spec("share-links")
        .requirements([
            requirement("sharing")
                .statement("Users can share documentation")
                .rules([rule("audit")
                    .statement("Share-link access is audited")
                    .requirements(["retention"])]),
            requirement("retention").statement("Share records are retained"),
        ])
        .build()
        .unwrap();
    let audit = &document.rules()[0];
    assert_eq!(
        audit.address.as_ref().unwrap().segments(),
        ["share-links", "rule", "audit"]
    );
    assert_eq!(audit.requirements, ["retention", "sharing"]);
}

#[test]
#[verifies("rule_rust_build_text_checks_trim", examples)]
fn build_rejects_text_that_is_empty_after_trimming() {
    let error = spec("share-links")
        .requirements([requirement("sharing")
            .statement("  \t")
            .description(" ")
            .from(source("policy").name(" ").document("   "))
            .rules([rule("expiry").statement("")])])
        .build()
        .unwrap_err();
    assert_eq!(
        error.violations(),
        [
            "Requirement `sharing` statement must not be empty",
            "requirement description must not be empty",
            "source name must not be empty",
            "document reference must not be empty",
            "Rule `expiry` statement must not be empty",
        ]
    );
}

#[test]
fn build_carries_every_structural_violation() {
    let error = spec("share-links")
        .requirements([
            requirement("sharing")
                .statement("Users can share documentation")
                .rules([rule("expiry")
                    .statement("Share links expire")
                    .requirements(["missing"])]),
            requirement("sharing").statement("A duplicate requirement"),
        ])
        .build()
        .unwrap_err();
    assert_eq!(
        error.violations(),
        [
            "duplicate requirement key `sharing`",
            "rule `expiry` references undeclared requirement `missing`",
        ]
    );
}

#[test]
fn build_rejects_a_malformed_explicit_rule_id() {
    let error = spec("share-links")
        .requirements([requirement("sharing")
            .statement("Users can share documentation")
            .rules([rule("expiry").statement("Share links expire").id("Bad Id")])])
        .build()
        .unwrap_err();
    assert_eq!(
        error.violations(),
        ["rule `expiry` id `Bad Id` must use lowercase ASCII letters, digits, '_' or '-'"]
    );
}

#[test]
fn handles_carry_declaration_addresses() {
    let document = sample(true).build().unwrap();
    let handles = document.handles();
    let sharing = handles.requirement("sharing").unwrap();
    assert_eq!(
        sharing.address.segments(),
        ["share-links", "requirement", "sharing"]
    );
    let expiry = sharing.rule("expiry").unwrap();
    assert_eq!(
        expiry.address.segments(),
        ["share-links", "requirement", "sharing", "rule", "expiry"]
    );
    assert!(handles.requirement("absent").is_err());
    assert!(sharing.rule("overview").is_err());
}

#[test]
#[verifies("rule_rust_typed_input_round_trip", examples)]
fn materialize_emits_the_wire_document_with_addresses() {
    let input = sample(true).build().unwrap().materialize("spec://rust");
    assert_eq!(input.schema_version, 1);
    assert_eq!(input.declared_by, "spec://rust");
    assert!(input.rules.iter().all(|rule| rule.address.is_some()));
    let round_trip: crate::protocol::TypedSpecInput =
        serde_json::from_str(&serde_json::to_string(&input).unwrap()).unwrap();
    assert_eq!(
        serde_json::to_string(&round_trip).unwrap(),
        serde_json::to_string(&input).unwrap()
    );
}

/// The kernel source may not reach for files, environment, processes,
/// clocks, or randomness. The scan reads the module sources as checked-in
/// text; the kernel itself stays free of I/O.
#[test]
#[verifies("rule_rust_authoring_kernel_is_pure", conformance)]
fn kernel_modules_import_no_ambient_capability() {
    let root = camino::Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/authoring");
    let mut checked = 0;
    for name in [
        "addresses.rs",
        "builders.rs",
        "checks.rs",
        "document.rs",
        "error.rs",
        "handles.rs",
    ] {
        let text = std::fs::read_to_string(root.join(name)).unwrap();
        for forbidden in [
            "std::fs",
            "std::env",
            "std::process",
            "std::net",
            "std::time",
            "SystemTime",
            "Instant::",
        ] {
            assert!(!text.contains(forbidden), "{name} must not use {forbidden}");
        }
        checked += 1;
    }
    assert_eq!(checked, 6);
}
