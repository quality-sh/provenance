//! `#[derive(Relations)]`: the reference-field declaration of one record kind.
//!
//! Each `StableId`-typed field carries one `#[relation(...)]` or is exempted
//! with `#[relation(none)]`; the field named `id` is the owner key and needs
//! neither. The derive emits `Kind::RELATIONS` and `impl RelationOwner`.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    Data, DeriveInput, Error, Fields, GenericArgument, Ident, LitStr, PathArguments, Result, Type,
};

/// How a field's type holds its references.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    /// `StableId`
    Single,
    /// `Option<StableId>`
    OptionalSingle,
    /// `Vec<StableId>`
    List,
    /// `Option<T>`, a reference through `via`
    OptionalVia,
    /// `Vec<T>`, references through `via`
    ListVia,
    /// Anything else
    Other,
}

struct Declaration {
    target: Option<Ident>,
    flow: Option<Ident>,
    required: bool,
    name: Option<LitStr>,
    via: Option<Ident>,
    none: bool,
}

struct Row {
    field: Ident,
    name: String,
    target: Ident,
    flow: Ident,
    required: bool,
    list: bool,
    via: Option<Ident>,
    shape: Shape,
}

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let owner = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(Error::new_spanned(input, "Relations derives on a struct"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Error::new_spanned(
            input,
            "Relations derives on named fields",
        ));
    };
    let mut rows = Vec::new();
    for field in &fields.named {
        let ident = field.ident.clone().expect("named field");
        let shape = shape_of(&field.ty);
        let attribute = field
            .attrs
            .iter()
            .find(|attribute| attribute.path().is_ident("relation"));
        let Some(attribute) = attribute else {
            if ident != "id" && matches!(shape, Shape::Single | Shape::OptionalSingle | Shape::List)
            {
                return Err(Error::new_spanned(
                    field,
                    format!(
                        "field `{ident}` holds a StableId but carries no #[relation]; every \
                         reference field declares one (use #[relation(none)] to exempt it)"
                    ),
                ));
            }
            continue;
        };
        let declaration = parse_declaration(attribute)?;
        if declaration.none {
            continue;
        }
        rows.push(row(field, ident, shape, declaration)?);
    }
    Ok(emit(owner, &rows))
}

fn parse_declaration(attribute: &syn::Attribute) -> Result<Declaration> {
    let mut declaration = Declaration {
        target: None,
        flow: None,
        required: false,
        name: None,
        via: None,
        none: false,
    };
    attribute.parse_nested_meta(|meta| {
        if meta.path.is_ident("none") {
            declaration.none = true;
        } else if meta.path.is_ident("target") {
            declaration.target = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("flow") {
            declaration.flow = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("required") {
            declaration.required = true;
        } else if meta.path.is_ident("name") {
            declaration.name = Some(meta.value()?.parse()?);
        } else if meta.path.is_ident("via") {
            declaration.via = Some(meta.value()?.parse()?);
        } else {
            let key = meta
                .path
                .get_ident()
                .map_or_else(|| "?".to_string(), ToString::to_string);
            return Err(meta.error(format!(
                "unknown relation key `{key}`; expected target, flow, required, name, via, or none"
            )));
        }
        Ok(())
    })?;
    if declaration.none
        && (declaration.target.is_some()
            || declaration.flow.is_some()
            || declaration.required
            || declaration.name.is_some()
            || declaration.via.is_some())
    {
        return Err(Error::new_spanned(
            attribute,
            "#[relation(none)] takes no other key",
        ));
    }
    Ok(declaration)
}

fn row(field: &syn::Field, ident: Ident, shape: Shape, declaration: Declaration) -> Result<Row> {
    let Some(target) = declaration.target else {
        return Err(Error::new_spanned(
            field,
            format!("relation on `{ident}` names no target"),
        ));
    };
    let Some(flow) = declaration.flow else {
        return Err(Error::new_spanned(
            field,
            format!("relation on `{ident}` names no flow"),
        ));
    };
    if !["target_upstream", "target_downstream", "none"].contains(&flow.to_string().as_str()) {
        return Err(Error::new_spanned(
            &flow,
            "flow is target_upstream, target_downstream, or none",
        ));
    }
    let shape_matches_via = matches!(
        (shape, declaration.via.is_some()),
        (Shape::Single | Shape::OptionalSingle | Shape::List, false)
            | (Shape::OptionalVia | Shape::ListVia, true)
    );
    if !shape_matches_via {
        return Err(Error::new_spanned(
            field,
            format!(
                "relation field `{ident}` must be StableId, Option<StableId>, or \
                 Vec<StableId>, or an Option or Vec of a struct with a via field"
            ),
        ));
    }
    let list = matches!(shape, Shape::List | Shape::ListVia);
    if declaration.required && !list {
        return Err(Error::new_spanned(
            field,
            format!(
                "`required` on `{ident}` is for lists; a bare StableId is required by its type"
            ),
        ));
    }
    let required = declaration.required || shape == Shape::Single;
    let name = declaration
        .name
        .map_or_else(|| ident.to_string(), |literal| literal.value());
    Ok(Row {
        field: ident,
        name,
        target,
        flow: Ident::new(&flow_variant(&flow), Span::call_site()),
        required,
        list,
        via: declaration.via,
        shape,
    })
}

fn flow_variant(flow: &Ident) -> String {
    match flow.to_string().as_str() {
        "target_upstream" => "TargetUpstream".to_string(),
        "target_downstream" => "TargetDownstream".to_string(),
        _ => "None".to_string(),
    }
}

fn shape_of(ty: &Type) -> Shape {
    if is_stable_id(ty) {
        return Shape::Single;
    }
    let Some((wrapper, inner)) = wrapped(ty) else {
        return Shape::Other;
    };
    match (wrapper.as_str(), is_stable_id(inner)) {
        ("Option", true) => Shape::OptionalSingle,
        ("Vec", true) => Shape::List,
        ("Option", false) => Shape::OptionalVia,
        ("Vec", false) => Shape::ListVia,
        _ => Shape::Other,
    }
}

fn is_stable_id(ty: &Type) -> bool {
    matches!(ty, Type::Path(path) if path.path.segments.last().is_some_and(|segment| {
        segment.ident == "StableId" && segment.arguments.is_none()
    }))
}

fn wrapped(ty: &Type) -> Option<(String, &Type)> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if arguments.args.len() != 1 {
        return None;
    }
    let GenericArgument::Type(inner) = arguments.args.first()? else {
        return None;
    };
    Some((segment.ident.to_string(), inner))
}

fn emit(owner: &Ident, rows: &[Row]) -> TokenStream {
    let core = quote!(::provenance_core::model);
    let count = rows.len();
    let table = rows.iter().map(|row| {
        let (name, target, flow, list, required) =
            (&row.name, &row.target, &row.flow, row.list, row.required);
        quote! {
            #core::relations::RelationDecl {
                owner: #core::NodeType::#owner,
                name: #name,
                target: #core::NodeType::#target,
                list: #list,
                required: #required,
                flow: #core::relations::RelationFlow::#flow,
            }
        }
    });
    let walks = rows.iter().map(|row| {
        let (name, field) = (&row.name, &row.field);
        match (row.shape, &row.via) {
            (Shape::Single, _) => quote!(references.push((#name, &self.#field));),
            (Shape::OptionalSingle, _) => quote! {
                if let ::core::option::Option::Some(value) = &self.#field {
                    references.push((#name, value));
                }
            },
            (Shape::List, _) => quote! {
                for value in &self.#field {
                    references.push((#name, value));
                }
            },
            (Shape::OptionalVia, Some(via)) => quote! {
                if let ::core::option::Option::Some(entry) = &self.#field {
                    references.push((#name, &entry.#via));
                }
            },
            (Shape::ListVia, Some(via)) => quote! {
                for entry in &self.#field {
                    references.push((#name, &entry.#via));
                }
            },
            _ => unreachable!("row shapes are checked before emission"),
        }
    });
    quote! {
        impl #owner {
            pub const RELATIONS: [#core::relations::RelationDecl; #count] = [#(#table),*];
        }

        impl #core::relations::RelationOwner for #owner {
            const OWNER: #core::NodeType = #core::NodeType::#owner;

            fn relations() -> &'static [#core::relations::RelationDecl] {
                &Self::RELATIONS
            }

            fn id(&self) -> &#core::StableId {
                &self.id
            }

            fn references(&self) -> ::std::vec::Vec<(&'static str, &#core::StableId)> {
                let mut references = ::std::vec::Vec::new();
                #(#walks)*
                references
            }
        }
    }
}
