use crate::{ScopeId, StableId, VerificationBinding, VerificationMethod};

#[test]
fn verification_methods_round_trip_as_the_six_method_words() {
    let cases = [
        (VerificationMethod::Exhaustion, "exhaustion"),
        (VerificationMethod::Property, "property"),
        (VerificationMethod::Examples, "examples"),
        (VerificationMethod::Conformance, "conformance"),
        (VerificationMethod::Construction, "construction"),
        (VerificationMethod::Proof, "proof"),
    ];

    for (method, word) in cases {
        assert_eq!(serde_json::to_string(&method).unwrap(), format!("\"{word}\""));
        assert_eq!(
            serde_json::from_str::<VerificationMethod>(&format!("\"{word}\"")).unwrap(),
            method
        );
        assert_eq!(word.parse::<VerificationMethod>().unwrap(), method);
        assert_eq!(method.to_string(), word);
    }
}

#[test]
fn verification_binding_serializes_its_explicit_key_and_code_facts() {
    let binding = VerificationBinding {
        schema_version: crate::SUPPORTED_SCHEMA_VERSION,
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new("verification_binding_one").unwrap(),
        rule_id: StableId::new("rule_expiry").unwrap(),
        key: "share-link-expiry".into(),
        method: VerificationMethod::Examples,
        declared_by: "test://typescript".into(),
        retired: false,
        file: "tests/share-links.test.ts".into(),
        symbol: Some("share links expire".into()),
    };

    let value = serde_json::to_value(&binding).unwrap();
    assert_eq!(value["key"], "share-link-expiry");
    assert_eq!(value["method"], "examples");
    assert_eq!(value["file"], "tests/share-links.test.ts");
}
