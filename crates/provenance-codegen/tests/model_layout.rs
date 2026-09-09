use provenance_codegen::{component, rust_types};
use serde_json::{json, Map, Value};

#[test]
fn growing_catalog_uses_schema_family_indexes() {
    let mut schemas = Map::new();
    for family in 0..16 {
        let definitions: Map<String, Value> = (0..40)
            .map(|model| {
                (
                    format!("Model{model}"),
                    json!({"type":"integer","enum":[family * 40 + model]}),
                )
            })
            .collect();
        component(
            &format!("Family{family}Output"),
            json!({"type":"object","$defs":definitions}),
            &mut schemas,
        );
    }
    let files = rust_types(&json!({"components":{"schemas":schemas}})).unwrap();
    assert!(files["types.rs"].lines().count() < 30);
    for family in 0..16 {
        let path = format!("families/Family{family}Output.rs");
        assert!(files.contains_key(&path), "missing family index {path}");
    }
    for (path, source) in &files {
        assert!(source.lines().count() <= 500, "oversized {path}");
    }
}

#[test]
fn primitive_conversions_stay_with_the_model_that_owns_them() {
    let mut schemas = Map::new();
    component(
        "VersionOutput",
        json!({"type":"integer","enum":[7]}),
        &mut schemas,
    );
    let files = rust_types(&json!({"components":{"schemas":schemas}})).unwrap();
    let model = &files["models/VersionOutput.rs"];
    assert!(model.contains("From<VersionOutput> for i64"));
    assert!(!files.contains_key("models/i64.rs"));
}
