//! Help for the catalog-derived CLI dialect.
use super::address::{self, Address, Segment};
use provenance_store::operations::catalog::{
    CliDefaultValue, Definition, Parameter, QueryRoute,
};
use serde_json::Value;
use std::fmt::Write as _;

mod schema;

pub(super) fn print_collection(collection: &str) {
    let mut registrations = address::registrations(collection);
    registrations.sort_unstable_by_key(|address| command(address));

    println!("Catalog commands for {collection}:\n");
    println!("Usage:");
    for address in registrations {
        println!("  {} [options]", command(address));
        println!("      {}", description(collection, address));
        let options = concise_options(address);
        if !options.is_empty() {
            println!("      Options: {}", options.join(", "));
        }
    }
    println!("\nRun `provenance {collection} <command> --help` for operation inputs.");
    question_guidance(collection);
}

pub(super) fn print_operation(collection: &str, address: &Address) {
    println!("Catalog commands for {collection}:\n");
    println!("{}\n", description(collection, address));
    println!("Usage:\n  {} [options]", command(address));

    let definition = address.definition;
    if let Some(request) = definition.request_schema() {
        print_body_fields(definition, request);
        print_input_forms();
    }
    print_parameters(address);
    println!("\nHelp is selected only when --help is the operation's sole option.");
    println!("A declared flag value can be the literal string --help.");
    question_guidance(collection);
}

fn command(address: &Address) -> String {
    let words = address
        .words
        .iter()
        .map(|segment| match segment {
            Segment::Literal(word) => (*word).to_owned(),
            Segment::Parameter(name) => format!("<{name}>"),
        })
        .collect::<Vec<_>>();
    if words.is_empty() {
        format!("provenance {}", address.collection)
    } else {
        format!("provenance {} {}", address.collection, words.join(" "))
    }
}

fn description(collection: &str, address: &Address) -> String {
    address.query.map_or_else(
        || address.definition.description.to_owned(),
        |query| format!("Run the {query} query on the selected {collection} resource."),
    )
}

fn concise_options(address: &Address) -> Vec<String> {
    let mut options = selected_parameters(address)
        .iter()
        .filter(|parameter| parameter.location != "path")
        .map(parameter_usage)
        .collect::<Vec<_>>();
    if address.definition.request_schema().is_some() {
        options.push("body flags or --stdin".into());
    }
    options
}

fn print_body_fields(definition: &Definition, request: &Value) {
    let fields = schema::body_fields(request);
    if fields.is_empty() {
        return;
    }
    println!("\nBody fields:");
    for field in fields {
        let flag = cli_name(field.name);
        let cli_default = cli_default(definition, field.name);
        let schema_default = schema::default(field.schema);
        let required = field.required && cli_default.is_none() && schema_default.is_none();
        if let Some(item) = schema::array_item_label(request, field.schema) {
            println!(
                "  --{flag} <{item}> (repeatable){}",
                qualifiers(required, cli_default.as_deref(), schema_default)
            );
            println!(
                "  --{flag}-json <json>{}",
                qualifiers(required, cli_default.as_deref(), schema_default)
            );
        } else if schema::accepts_plain(request, field.schema) {
            let kind = schema::type_label(request, field.schema);
            println!(
                "  --{flag} <{kind}>{}",
                qualifiers(required, cli_default.as_deref(), schema_default)
            );
        } else {
            println!(
                "  --{flag}-json <json>{}",
                qualifiers(required, cli_default.as_deref(), schema_default)
            );
        }
    }
    print_aliases(definition, request);
}

fn print_aliases(definition: &Definition, request: &Value) {
    let aliases = &definition.registration.request.argument_aliases;
    if aliases.is_empty() {
        return;
    }
    println!("\nAliases:");
    for alias in aliases {
        let Some(field) = schema::body_fields(request)
            .into_iter()
            .find(|field| field.name == alias.field)
        else {
            continue;
        };
        let kind = if alias.wrap_array {
            schema::array_item_label(request, field.schema).unwrap_or_else(|| "json".into())
        } else {
            schema::type_label(request, field.schema)
        };
        let repeatable = if alias.wrap_array {
            " (repeatable)"
        } else {
            ""
        };
        println!(
            "  --{} <{kind}>{repeatable} [alias for --{}]",
            cli_name(alias.argument),
            cli_name(alias.field)
        );
    }
}

fn print_input_forms() {
    println!("\nStructured input:");
    println!("  --<field>-json <json>  Set one canonical field to a complete JSON value.");
    println!("  --stdin                Read one complete data object from standard input.");
    println!("  Standard input can combine with flags for different fields.");
}

fn print_parameters(address: &Address) {
    let parameters = selected_parameters(address)
        .into_iter()
        .filter(|parameter| parameter.location != "path")
        .collect::<Vec<_>>();
    if parameters.is_empty() {
        return;
    }
    println!("\nOptions:");
    for parameter in parameters {
        let flag = cli_name(parameter.name);
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
        println!("  --{flag} <{kind}>{}", bracketed(&details));
    }
}

fn selected_parameters(address: &Address) -> Vec<Parameter> {
    let definition = address.definition;
    let mut parameters = selected_query(definition, address.query).map_or_else(
        || definition.registration.request.parameters.clone(),
        |query| query.parameters.clone(),
    );
    parameters.extend(
        definition
            .parameters()
            .into_iter()
            .filter(|parameter| parameter.location == "header"),
    );
    parameters
}

fn selected_query<'a>(
    definition: &'a Definition,
    selected: Option<&str>,
) -> Option<&'a QueryRoute> {
    let selected = selected?;
    definition
        .registration
        .queries
        .iter()
        .find(|query| query.name == selected)
}

fn parameter_usage(parameter: &Parameter) -> String {
    format!(
        "--{} <{}>",
        cli_name(parameter.name),
        schema::type_label(&parameter.schema, &parameter.schema)
    )
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

fn qualifiers(
    required: bool,
    cli_default: Option<&str>,
    schema_default: Option<&Value>,
) -> String {
    let mut details = Vec::new();
    if required {
        details.push("required".into());
    }
    if let Some(default) = cli_default {
        details.push(format!("CLI default: {default}"));
    } else if let Some(default) = schema_default {
        details.push(format!("schema default: {default}"));
    }
    bracketed(&details)
}

fn bracketed(details: &[String]) -> String {
    let mut rendered = String::new();
    for detail in details {
        write!(rendered, " [{detail}]").expect("writing to a String cannot fail");
    }
    rendered
}

fn cli_name(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}

fn question_guidance(collection: &str) {
    if collection == "questions" {
        println!("\nA question should be resolvable in one agent session;");
        println!("otherwise it is fog or needs decomposition.");
    }
}
