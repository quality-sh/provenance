//! Keep generated model definitions and their conversions in named files.
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write as _,
};

pub fn render(
    parsed: syn::File,
    document: &Value,
) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let models: BTreeSet<_> = parsed.items.iter().filter_map(declaration_name).collect();
    let families: Vec<_> = document["components"]["schemas"]
        .as_object()
        .ok_or("missing OpenAPI components")?
        .values()
        .filter_map(|schema| schema["x-provenance-model-family"].as_str())
        .collect();
    let mut groups: BTreeMap<String, Vec<syn::Item>> = BTreeMap::new();
    for mut item in parsed.items {
        let name = item_group(&item, &models)?;
        strip_schema_docs(&mut item);
        groups.entry(name).or_default().push(item);
    }
    let mut output = BTreeMap::new();
    let mut indexes: BTreeMap<String, String> = BTreeMap::new();
    for (name, items) in groups {
        let family = families
            .iter()
            .filter(|family| name.starts_with(**family))
            .max_by_key(|family| family.len())
            .copied()
            .unwrap_or("shared");
        let path = format!("models/{name}.rs");
        let file = syn::File {
            shebang: None,
            attrs: vec![],
            items,
        };
        insert(&mut output, path, &prettyplease::unparse(&file))?;
        writeln!(
            indexes.entry(family.to_owned()).or_default(),
            "include!(\"../models/{name}.rs\");"
        )?;
    }
    let mut root = String::new();
    for (family, index) in indexes {
        let path = format!("families/{family}.rs");
        writeln!(root, "include!(\"{path}\");")?;
        insert(&mut output, path, &index)?;
    }
    insert(&mut output, "types.rs".into(), &root)?;
    Ok(output)
}

fn insert(
    output: &mut BTreeMap<String, String>,
    path: String,
    content: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = format!("// Generated from OpenAPI. Do not edit.\n{content}");
    if source.lines().count() > 500 {
        return Err(format!("generated model group exceeds 500 lines: {path}").into());
    }
    output.insert(path, source);
    Ok(())
}

fn declaration_name(item: &syn::Item) -> Option<String> {
    match item {
        syn::Item::Struct(item) => Some(item.ident.to_string()),
        syn::Item::Enum(item) => Some(item.ident.to_string()),
        syn::Item::Type(item) => Some(item.ident.to_string()),
        syn::Item::Mod(item) => Some(item.ident.to_string()),
        _ => None,
    }
}

fn item_group(
    item: &syn::Item,
    models: &BTreeSet<String>,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(name) = declaration_name(item) {
        return Ok(name);
    }
    if let syn::Item::Impl(item) = item {
        let syn::Type::Path(path) = item.self_ty.as_ref() else {
            return Err("unsupported generated impl target".into());
        };
        let name = path.path.segments.last().unwrap().ident.to_string();
        if models.contains(&name) {
            return Ok(name);
        }
        // Reverse conversions belong to the named source model, not to i64 or Vec.
        if let Some((_, trait_path, _)) = &item.trait_ {
            if let Some(segment) = trait_path.segments.last() {
                if segment.ident == "From" {
                    if let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments {
                        if let Some(syn::GenericArgument::Type(syn::Type::Path(source))) =
                            arguments.args.first()
                        {
                            let source = source.path.segments.last().unwrap().ident.to_string();
                            if models.contains(&source) {
                                return Ok(source);
                            }
                        }
                    }
                }
            }
        }
    }
    Err("unsupported generated item ownership".into())
}

fn strip_schema_docs(item: &mut syn::Item) {
    if let syn::Item::Struct(model) = item {
        let booleans = model
            .fields
            .iter()
            .filter(
                |field| matches!(&field.ty, syn::Type::Path(path) if path.path.is_ident("bool")),
            )
            .count();
        if booleans > 3 {
            model.attrs.push(syn::parse_quote!(
                #[allow(clippy::struct_excessive_bools, reason = "preserve the declared wire fields")]
            ));
        }
    }
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
