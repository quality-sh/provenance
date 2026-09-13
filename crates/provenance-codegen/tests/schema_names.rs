use serde_json::{json, Map};

#[test]
fn different_roots_and_directions_cannot_overwrite_references() {
    let schema = json!({"type":"object","properties":{"item":{"$ref":"#/$defs/Shared"}},
        "$defs":{"Shared":{"type":"string"}}});
    let mut components = Map::new();
    provenance_codegen::component("FirstInput", schema.clone(), &mut components);
    provenance_codegen::component("FirstOutput", schema.clone(), &mut components);
    provenance_codegen::component("SecondOutput", schema, &mut components);
    for prefix in ["FirstInput", "FirstOutput", "SecondOutput"] {
        assert_eq!(
            components[prefix]["properties"]["item"]["$ref"],
            format!("#/components/schemas/{prefix}Shared")
        );
        assert_eq!(components[&format!("{prefix}Shared")]["type"], "string");
    }
}

#[test]
fn production_corpus_references_resolve_without_name_collisions() {
    fn check(value: &serde_json::Value, root: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(reference) = map.get("$ref") {
                    let reference = reference.as_str().unwrap();
                    assert!(
                        root.pointer(reference.strip_prefix('#').unwrap()).is_some(),
                        "{reference}"
                    );
                }
                for value in map.values() {
                    check(value, root);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    check(value, root);
                }
            }
            _ => {}
        }
    }
    let corpus = provenance_codegen::corpus();
    check(&corpus, &corpus);
    assert!(corpus
        .pointer("/components/schemas/GetOutput/properties/found")
        .is_some());
    assert!(corpus
        .pointer("/components/schemas/EvidenceOutput/properties/stale")
        .is_some());
}

#[test]
#[should_panic(expected = "schema component name collision")]
fn colliding_components_stop_generation() {
    let mut components = Map::new();
    provenance_codegen::component("FirstInput", json!({"type":"string"}), &mut components);
    provenance_codegen::component("FirstInput", json!({"type":"integer"}), &mut components);
}

#[test]
fn colliding_rust_identifiers_stop_generation() {
    let document = json!({"components":{"schemas":{
        "FooBar":{"type":"object","properties":{"a":{"type":"string"}}},
        "foo_bar":{"type":"object","properties":{"b":{"type":"integer"}}}
    }}});
    assert!(provenance_codegen::rust_types(&document)
        .unwrap_err()
        .to_string()
        .contains("name collision"));
}
