use serde_json::Value;

fn operation<'a>(document: &'a Value, path: &str) -> &'a Value {
    &document["paths"][path]["patch"]
}

#[test]
fn generated_public_patch_types_hide_native_clear_fields() {
    let (openapi, _) = provenance_codegen::documents();
    for path in ["/sources/{id}", "/contributions/{id}"] {
        let reference = operation(&openapi, path)["requestBody"]["content"]["application/json"]
            ["schema"]["$ref"]
            .as_str()
            .unwrap();
        let schema = openapi
            .pointer(reference.strip_prefix('#').unwrap())
            .unwrap();
        assert!(schema["properties"]["data"]["properties"]
            .get("clear_fields")
            .is_none());
    }

    let generated = provenance_codegen::rust_types(&openapi).unwrap();
    let text = generated.values().cloned().collect::<String>();
    assert!(!text.contains("clear_fields"));
}
