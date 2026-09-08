//! Adapt `OpenAPI` 3.1 component schemas for the pinned Rust generator.
use serde_json::{json, Value};
use std::{collections::BTreeMap, fmt::Write as _};

fn draft7(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.remove("$schema");
            if let Some(constant) = map.remove("const") {
                map.insert("enum".into(), json!([constant]));
            }
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                if let Some(name) = reference.strip_prefix("#/components/schemas/") {
                    *reference = format!("#/definitions/{name}");
                }
            }
            for value in map.values_mut() {
                draft7(value);
            }
        }
        Value::Array(array) => {
            for value in array {
                draft7(value);
            }
        }
        _ => {}
    }
}

pub fn rust_types(
    document: &Value,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut definitions = document["components"]["schemas"].clone();
    let mut identifiers = std::collections::BTreeSet::new();
    for name in definitions
        .as_object()
        .ok_or("missing OpenAPI components")?
        .keys()
    {
        let normalized: String = name
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .map(|c| c.to_ascii_lowercase())
            .collect();
        if normalized.is_empty() || !identifiers.insert(normalized) {
            return Err(format!("converted Rust schema name collision: {name}").into());
        }
    }
    crate::unions::normalize(&mut definitions, document);
    draft7(&mut definitions);
    let root = serde_json::from_value::<schemars08::schema::RootSchema>(
        json!({"definitions":definitions}),
    )?;
    let mut types = typify::TypeSpace::new(&typify::TypeSpaceSettings::default());
    types.add_root_schema(root)?;
    let parsed = syn::parse2::<syn::File>(types.to_stream())?;
    let mut groups: BTreeMap<String, Vec<syn::Item>> = BTreeMap::new();
    for mut item in parsed.items {
        let name = item_group(&item)?;
        // Typify embeds the full JSON schema in each doc comment. The
        // OpenAPI document retains that information without repeating it.
        strip_schema_docs(&mut item);
        groups
            .entry(format!("models/{name}.rs"))
            .or_default()
            .push(item);
    }
    let mut output = BTreeMap::new();
    for (path, items) in groups {
        let file = syn::File {
            shebang: None,
            attrs: vec![],
            items,
        };
        let text = format!(
            "// Generated from OpenAPI. Do not edit.\n{}",
            prettyplease::unparse(&file)
        );
        if text.lines().count() > 500 {
            return Err(format!("generated model group exceeds 500 lines: {path}").into());
        }
        output.insert(path, text);
    }
    let mut includes = String::new();
    for name in output.keys() {
        writeln!(includes, "include!(\"{name}\");").expect("writing to String succeeds");
    }
    output.insert(
        "types.rs".into(),
        format!("// Generated from OpenAPI. Do not edit.\n{includes}"),
    );
    Ok(output)
}

fn item_group(item: &syn::Item) -> Result<String, Box<dyn std::error::Error>> {
    let group = match item {
        syn::Item::Struct(item) => item.ident.to_string(),
        syn::Item::Enum(item) => item.ident.to_string(),
        syn::Item::Type(item) => item.ident.to_string(),
        syn::Item::Mod(item) => item.ident.to_string(),
        syn::Item::Impl(item) => {
            let syn::Type::Path(path) = item.self_ty.as_ref() else {
                return Err("unsupported generated impl target".into());
            };
            path.path.segments.last().unwrap().ident.to_string()
        }
        _ => return Err("unsupported generated top-level item".into()),
    };
    Ok(group)
}

fn strip_schema_docs(item: &mut syn::Item) {
    let attrs = match item {
        syn::Item::Struct(item) => &mut item.attrs,
        syn::Item::Enum(item) => &mut item.attrs,
        syn::Item::Type(item) => &mut item.attrs,
        syn::Item::Impl(item) => &mut item.attrs,
        syn::Item::Mod(item) => &mut item.attrs,
        _ => return,
    };
    attrs.retain(|attribute| !attribute.path().is_ident("doc"));
}
