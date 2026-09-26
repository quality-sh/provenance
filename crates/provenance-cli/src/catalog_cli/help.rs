//! Help rendered from the registered catalog contracts.
//!
//! One block per registered address: the real nested usage line, the
//! operation description, the body fields with their types, enums, aliases
//! and defaults, the selected query parameters, and the request controls.
use super::address::{self, Segment};
use super::fields::{self, Source};
use provenance_store::operations::catalog::{CliDefaultValue, Definition, Parameter};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt::Write as _;

mod schema;

/// Render the help block that lists every registered operation.
pub(super) fn collection(collection: &str) -> String {
    let mut registrations = address::registrations(collection);
    registrations.sort_unstable_by_key(|address| usage(collection, address));
    let mut help = format!("Catalog commands for {collection}:");
    for address in registrations {
        operation(&mut help, collection, address);
    }
    help
}

fn operation(help: &mut String, collection: &str, address: &address::Address) {
    let definition = address.definition;
    writeln!(help, "\n  {}", usage(collection, address)).expect("writing to a String cannot fail");
    writeln!(help, "      {}", description(collection, address))
        .expect("writing to a String cannot fail");
    let declared = fields::declared(definition).expect("catalog fields compile");
    print_body_fields(help, definition, &declared);
    print_options(help, address);
}

fn usage(collection: &str, address: &address::Address) -> String {
    let words = address
        .words
        .iter()
        .map(|segment| match segment {
            Segment::Literal(word) => (*word).to_owned(),
            Segment::Parameter(name) => format!("<{name}>"),
        })
        .collect::<Vec<_>>();
    format!("provenance {collection} {}", words.join(" "))
}

fn description(collection: &str, address: &address::Address) -> String {
    address.query.map_or_else(
        || address.definition.description.to_owned(),
        |query| format!("Run the {query} query on the selected {collection} resource."),
    )
}

fn print_body_fields(help: &mut String, definition: &Definition, declared: &[fields::Field]) {
    let Some(request) = definition.request_schema() else {
        return;
    };
    let aliases = &definition.registration.request.argument_aliases;
    let alias_names = aliases
        .iter()
        .map(|alias| fields::cli_name(alias.argument))
        .collect::<BTreeSet<_>>();
    let canonical = declared
        .iter()
        .filter(|field| !alias_names.contains(&field.name))
        .collect::<Vec<_>>();
    let body = canonical
        .iter()
        .filter(|field| matches!(field.source, Source::Body { .. }))
        .collect::<Vec<_>>();
    if body.is_empty() {
        return;
    }
    writeln!(help, "\n      Body fields:").expect("writing to a String cannot fail");
    let mut json_fields = BTreeSet::new();
    for field in body {
        let Source::Body {
            wire_field,
            value_schema,
            wrap_array,
        } = &field.source
        else {
            continue;
        };
        let details = field_details(request, definition, wire_field, value_schema);
        if let Some((kind, repeatable)) = plain_usage(request, value_schema, *wrap_array) {
            writeln!(
                help,
                "        --{} <{kind}>{repeatable}{}",
                field.name,
                bracketed(&details)
            )
            .expect("writing to a String cannot fail");
        }
        if json_fields.insert(wire_field.clone()) {
            writeln!(
                help,
                "        --{} <json>{}",
                fields::json_flag(wire_field),
                bracketed(&details)
            )
            .expect("writing to a String cannot fail");
        }
    }
    print_aliases(help, request, definition, declared);
    writeln!(
        help,
        "\n      --<field>-json carries one whole JSON value; --stdin fills only the body fields that no flag assigned."
    )
    .expect("writing to a String cannot fail");
}

fn print_aliases(
    help: &mut String,
    request: &Value,
    definition: &Definition,
    declared: &[fields::Field],
) {
    for alias in &definition.registration.request.argument_aliases {
        let Some(field) = declared
            .iter()
            .find(|field| field.name == fields::cli_name(alias.argument))
        else {
            continue;
        };
        let Source::Body {
            value_schema,
            wrap_array,
            ..
        } = &field.source
        else {
            continue;
        };
        let kind = schema::type_label(request, value_schema);
        let repeatable = if *wrap_array { " (repeatable)" } else { "" };
        writeln!(
            help,
            "        --{} <{kind}>{repeatable} [alias for --{}]",
            field.name,
            fields::cli_name(alias.field)
        )
        .expect("writing to a String cannot fail");
    }
}

fn print_options(help: &mut String, address: &address::Address) {
    let definition = address.definition;
    let mut seen = BTreeSet::new();
    let parameters = variant_parameters(definition, address.query)
        .into_iter()
        .filter(|parameter| {
            parameter.location != "path" && parameter.name != "query" && seen.insert(parameter.name)
        })
        .collect::<Vec<_>>();
    if parameters.is_empty() {
        return;
    }
    writeln!(help, "\n      Options:").expect("writing to a String cannot fail");
    for parameter in parameters {
        let flag = fields::cli_name(parameter.name);
        let kind = schema::type_label(&parameter.schema, &parameter.schema);
        let mut details = Vec::new();
        if parameter.location == "header" && parameter.name == "Idempotency-Key" {
            details.push("generated if omitted".into());
        } else if parameter.required {
            details.push("required".into());
        }
        if let Some(default) = schema::default(&parameter.schema) {
            details.push(format!("schema default: {default}"));
        }
        if let Some(constraints) = schema::constraints(&parameter.schema) {
            details.push(constraints);
        }
        writeln!(help, "        --{flag} <{kind}>{}", bracketed(&details))
            .expect("writing to a String cannot fail");
    }
}

/// The parameter set of the selected query variant, from the typed contracts.
fn variant_parameters(definition: &Definition, selected: Option<&str>) -> Vec<Parameter> {
    definition
        .query_variants()
        .into_iter()
        .find(|variant| variant.selector == selected)
        .map_or_else(|| definition.parameters(), |variant| variant.parameters)
}

fn plain_usage(
    request: &Value,
    value_schema: &Value,
    wrap_array: bool,
) -> Option<(String, &'static str)> {
    if wrap_array {
        return Some((schema::type_label(request, value_schema), " (repeatable)"));
    }
    if let Some(item) = schema::array_item_label(request, value_schema) {
        return Some((item, " (repeatable)"));
    }
    if schema::object_only(request, value_schema) {
        return None;
    }
    Some((schema::type_label(request, value_schema), ""))
}

fn field_details(
    request: &Value,
    definition: &Definition,
    wire_field: &str,
    value_schema: &Value,
) -> Vec<String> {
    let mut details = Vec::new();
    let schema_default = schema::default(value_schema).map(Value::to_owned);
    let required = schema::required(request, wire_field)
        && cli_default(definition, wire_field).is_none()
        && schema_default.is_none();
    if required {
        details.push("required".into());
    }
    if let Some(default) = cli_default(definition, wire_field) {
        details.push(format!("CLI default: {default}"));
    } else if let Some(default) = schema_default {
        details.push(format!("schema default: {default}"));
    }
    details
}

fn cli_default(definition: &Definition, field: &str) -> Option<String> {
    definition
        .registration
        .cli
        .defaults
        .iter()
        .find(|default| default.field == field)
        .map(|default| match default.value {
            CliDefaultValue::String(value) => Value::String(value.to_owned()).to_string(),
            CliDefaultValue::EmptyArray => "[]".into(),
        })
}

fn bracketed(details: &[String]) -> String {
    let mut rendered = String::new();
    for detail in details {
        write!(rendered, " [{detail}]").expect("writing to a String cannot fail");
    }
    rendered
}
