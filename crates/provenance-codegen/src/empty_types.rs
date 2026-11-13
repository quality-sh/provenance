//! Write the generated types with no values or no fields without a derived
//! enum with no variants.
//!
//! Typify writes `pub enum X {}` for a `false` schema and derives
//! `Deserialize` for a closed object with no properties. For both, the serde
//! derive makes an enum with no variants, and the `empty_enums` lint refuses
//! it. This pass writes a struct over `Infallible` for the first, and a
//! `Deserialize` impl that calls the client's `empty` module for both. The JSON
//! that each type reads and writes does not change.
use syn::{punctuated::Punctuated, Attribute, Item, Meta, Path, Token};

pub fn rewrite(items: Vec<Item>) -> Vec<Item> {
    let mut output = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Item::Enum(model) if model.variants.is_empty() => {
                let name = model.ident;
                let mut attrs = model.attrs;
                remove_serde(&mut attrs);
                let vis = model.vis;
                output.push(syn::parse_quote!(
                    #(#attrs)*
                    #vis struct #name(::std::convert::Infallible);
                ));
                output.push(syn::parse_quote!(
                    impl ::serde::Serialize for #name {
                        fn serialize<S: ::serde::Serializer>(
                            &self,
                            _: S,
                        ) -> ::std::result::Result<S::Ok, S::Error> {
                            match self.0 {}
                        }
                    }
                ));
                let text = name.to_string();
                output.push(syn::parse_quote!(
                    impl<'de> ::serde::Deserialize<'de> for #name {
                        fn deserialize<D: ::serde::Deserializer<'de>>(
                            deserializer: D,
                        ) -> ::std::result::Result<Self, D::Error> {
                            crate::empty::deserialize_enum(deserializer, #text).map(Self)
                        }
                    }
                ));
            }
            Item::Struct(mut model)
                if matches!(&model.fields, syn::Fields::Named(fields) if fields.named.is_empty())
                    && denies_unknown_fields(&model.attrs) =>
            {
                remove_derive(&mut model.attrs, "Deserialize");
                let name = model.ident.clone();
                let text = name.to_string();
                output.push(Item::Struct(model));
                output.push(syn::parse_quote!(
                    impl<'de> ::serde::Deserialize<'de> for #name {
                        fn deserialize<D: ::serde::Deserializer<'de>>(
                            deserializer: D,
                        ) -> ::std::result::Result<Self, D::Error> {
                            crate::empty::deserialize_struct(deserializer, #text).map(|()| Self {})
                        }
                    }
                ));
            }
            item => output.push(item),
        }
    }
    output
}

fn derived_paths(attribute: &Attribute) -> Option<Punctuated<Path, Token![,]>> {
    if !attribute.path().is_ident("derive") {
        return None;
    }
    attribute
        .parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)
        .ok()
}

fn is_named(path: &Path, name: &str) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == name)
}

fn remove_derive(attrs: &mut [Attribute], name: &str) {
    for attribute in attrs {
        if let Some(paths) = derived_paths(attribute) {
            let kept = paths.into_iter().filter(|path| !is_named(path, name));
            *attribute = syn::parse_quote!(#[derive(#(#kept),*)]);
        }
    }
}

/// Removes the serde derives and the `#[serde(...)]` attributes, which apply
/// only to them.
fn remove_serde(attrs: &mut Vec<Attribute>) {
    attrs.retain(|attribute| !attribute.path().is_ident("serde"));
    remove_derive(attrs, "Serialize");
    remove_derive(attrs, "Deserialize");
}

fn denies_unknown_fields(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attribute| {
        attribute.path().is_ident("serde")
            && attribute
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .is_ok_and(|metas| {
                    metas
                        .iter()
                        .any(|meta| meta.path().is_ident("deny_unknown_fields"))
                })
    })
}
