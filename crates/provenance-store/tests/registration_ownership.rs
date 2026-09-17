use std::path::Path;

fn source(path: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
}

fn inference_hits<'a>(text: &str, forbidden: &'a [&str]) -> Vec<&'a str> {
    forbidden
        .iter()
        .copied()
        .filter(|pattern| text.contains(pattern))
        .collect()
}

#[test]
fn the_registration_boundary_gate_sees_a_planted_bypass() {
    let planted = r#"let response = match *backing { "search" => "nodes", _ => "result" };"#;
    assert_eq!(
        inference_hits(planted, &["match *backing", "object.remove(\"result\")"]),
        ["match *backing"]
    );
}

#[test]
fn route_construction_has_no_name_or_shape_inference() {
    let routes = format!(
        "{}{}",
        source("src/operations/catalog/routes.rs"),
        source("src/operations/catalog/routes/resource.rs")
    );
    let resources = source("src/operations/catalog/routes/resources.rs");
    let subresources = source("src/operations/catalog/routes/subresources.rs");
    let actions = source("src/operations/catalog/routes/actions.rs");
    let schema = source("src/operations/catalog/schema.rs");

    for (file, text, forbidden) in [
        (
            "routes.rs",
            routes.as_str(),
            "raw.request_schema[\"properties\"].get(\"scope_id\")",
        ),
        (
            "routes.rs",
            routes.as_str(),
            "backing.starts_with(\"list-\")",
        ),
        ("routes.rs", routes.as_str(), "match *backing"),
        (
            "resources.rs",
            resources.as_str(),
            ".find(|definition| definition.name == name)",
        ),
        ("subresources.rs", subresources.as_str(), "match name"),
        ("subresources.rs", subresources.as_str(), "name.starts_with"),
        ("subresources.rs", subresources.as_str(), "suffix.is_empty"),
        (
            "actions.rs",
            actions.as_str(),
            "matches!(\n            name,",
        ),
        (
            "schema.rs",
            schema.as_str(),
            "[\"items\", \"entries\", \"nodes\", \"sites\", \"rules\"]",
        ),
    ] {
        assert!(
            !text.contains(forbidden),
            "{file} still infers route behavior from {forbidden:?}"
        );
    }
}

#[test]
fn transport_has_no_generic_shape_or_etag_hunt() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../provenance-transport/src");
    let request = std::fs::read_to_string(root.join("routing/request.rs")).unwrap();
    let response = std::fs::read_to_string(root.join("routing/response.rs")).unwrap();
    let routing = std::fs::read_to_string(root.join("routing.rs")).unwrap();

    assert!(!request.contains("apply_null_clears"));
    assert!(!request.contains("BodyBinding::DiscussionStart"));
    assert!(!response.contains("object.get(\"found\")"));
    assert!(!response.contains("object.remove(\"result\")"));
    assert!(!response.contains(".unwrap_or(value)"));
    assert!(!routing.contains("pointer(\"/data/edit/etag\")"));
    assert!(!routing.contains("pointer(\"/data/discussion/version\")"));
}
