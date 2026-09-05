//! `#[derive(ProjectionRow)]`: one projection table column per field.
//!
//! The struct names its table with `#[table("name")]`. Each field is one
//! column named as the field, in declaration order. A field reads back by
//! its spelled type: a `Vec`, an `Option<Vec>`, or a field marked
//! `#[column(json)]` parses the JSON text it holds; a `bool` reads 0 or 1;
//! every other field reads the JSON scalar in the column. The derive emits
//! `impl ProjectionRow for Kind` over the helpers in
//! `provenance_core::model::projection_row`.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Data, DeriveInput, Error, Fields, GenericArgument, Ident, LitStr, PathArguments, Result, Type,
};

/// The name of the search column the store derives beside the record's
/// own columns on a kind table.
const SEARCH_COLUMN: &str = "search_text";

/// How one column reads back into its field.
#[derive(Clone, Copy)]
enum Read {
    /// The JSON scalar the column holds.
    Scalar,
    /// JSON text to parse.
    Json,
    /// 0 or 1.
    Flag,
}

struct Column {
    field: Ident,
    read: Read,
}

pub fn expand(input: &DeriveInput) -> Result<TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(Error::new_spanned(
            input,
            "ProjectionRow derives on a struct",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(Error::new_spanned(
            &data.fields,
            "ProjectionRow derives on a struct with named fields; a tuple struct has no column \
             names",
        ));
    };
    let table = table_name(input)?;
    let columns = fields
        .named
        .iter()
        .map(column)
        .collect::<Result<Vec<_>>>()?;
    Ok(emit(&input.ident, &table, &columns))
}

fn table_name(input: &DeriveInput) -> Result<LitStr> {
    let mut names = input
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("table"));
    let Some(attribute) = names.next() else {
        return Err(Error::new_spanned(
            input,
            "ProjectionRow needs the table name: #[table(\"name\")]",
        ));
    };
    if names.next().is_some() {
        return Err(Error::new_spanned(input, "one #[table] names the table"));
    }
    attribute.parse_args::<LitStr>()
}

fn column(field: &syn::Field) -> Result<Column> {
    let ident = field.ident.clone().expect("named field");
    if ident == SEARCH_COLUMN {
        return Err(Error::new_spanned(
            field,
            format!("field `{SEARCH_COLUMN}` takes the name of the derived search column; rename the field"),
        ));
    }
    let mut read = read_of(&field.ty);
    for attribute in &field.attrs {
        if attribute.path().is_ident("column") && parse_column_attribute(attribute)? {
            read = Read::Json;
        }
    }
    Ok(Column { field: ident, read })
}

/// `#[column(json)]` is the one key; it returns true.
fn parse_column_attribute(attribute: &syn::Attribute) -> Result<bool> {
    let mut json = false;
    attribute.parse_nested_meta(|meta| {
        if meta.path.is_ident("json") {
            json = true;
            Ok(())
        } else {
            Err(meta.error("unknown column key; the one key is `json`"))
        }
    })?;
    Ok(json)
}

/// The spelled type decides the read: `Vec<_>` and `Option<Vec<_>>` hold
/// JSON text, `bool` and `Option<bool>` hold a flag, the rest a scalar.
fn read_of(ty: &Type) -> Read {
    let Some((name, inner)) = last_segment(ty) else {
        return Read::Scalar;
    };
    match (name.as_str(), inner) {
        ("Vec", _) => Read::Json,
        ("bool", _) => Read::Flag,
        ("Option", Some(inner)) => match last_segment(inner) {
            Some((name, _)) if name == "Vec" => Read::Json,
            Some((name, _)) if name == "bool" => Read::Flag,
            _ => Read::Scalar,
        },
        _ => Read::Scalar,
    }
}

/// The last path segment's name and its one type argument, if any.
fn last_segment(ty: &Type) -> Option<(String, Option<&Type>)> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    let inner = match &segment.arguments {
        PathArguments::AngleBracketed(arguments) if arguments.args.len() == 1 => {
            match arguments.args.first()? {
                GenericArgument::Type(inner) => Some(inner),
                _ => None,
            }
        }
        _ => None,
    };
    Some((segment.ident.to_string(), inner))
}

fn emit(owner: &Ident, table: &LitStr, columns: &[Column]) -> TokenStream {
    let row = quote!(::provenance_core::model::projection_row);
    let names = columns.iter().map(|column| column.field.to_string());
    let encodes = columns.iter().map(|column| {
        let field = &column.field;
        quote!(#row::encode(&self.#field)?)
    });
    let decodes = columns.iter().enumerate().map(|(index, column)| {
        let name = column.field.to_string();
        let reader = match column.read {
            Read::Scalar => quote!(#row::scalar),
            Read::Json => quote!(#row::json),
            Read::Flag => quote!(#row::flag),
        };
        quote!((#name, #reader(&row[#index])?))
    });
    quote! {
        impl #row::ProjectionRow for #owner {
            const TABLE: &'static str = #table;
            const COLUMNS: &'static [&'static str] = &[#(#names),*];

            fn row(&self) -> #row::RowResult<::std::vec::Vec<#row::ColumnValue>> {
                ::core::result::Result::Ok(::std::vec![#(#encodes),*])
            }

            fn from_row(row: &[#row::ColumnValue]) -> #row::RowResult<Self> {
                let row = #row::columns::<Self>(row)?;
                #row::record(::std::vec![#(#decodes),*])
            }
        }
    }
}
