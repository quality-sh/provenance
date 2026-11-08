//! Each listed tool schema is a JSON Schema 2020-12 document for the model that calls
//! the tool: each format is one that 2020-12 defines, a number range is a minimum and
//! a maximum, and no description shows the Rust code behind the schema.
#![cfg(feature = "test-fixture")]

#[allow(dead_code)]
mod support {
    #[path = "../../../provenance-transport/tests/support/check_port.rs"]
    pub mod check_port;
    pub mod fixture_host;
    pub mod mcp;
    #[path = "../../../provenance-transport/tests/support/records.rs"]
    pub mod records;
    pub mod schema_check;
}

use serde_json::{json, Value};
use support::{mcp::Session, records::Repository};

/// The formats of JSON Schema 2020-12 (Validation, section 7.3).
const FORMATS: &[&str] = &[
    "date-time",
    "date",
    "time",
    "duration",
    "email",
    "idn-email",
    "hostname",
    "idn-hostname",
    "ipv4",
    "ipv6",
    "uri",
    "uri-reference",
    "iri",
    "iri-reference",
    "uuid",
    "uri-template",
    "json-pointer",
    "relative-json-pointer",
    "regex",
];

/// Words that show Rust code to the model instead of the meaning of the field.
const RUST_WORDS: &[&str] = &[
    "`String`",
    "`new`",
    "`None`",
    "`Some",
    "[`",
    "inner",
    "private",
    "deserializ",
    "variant",
    "Option<",
    "Vec<",
    "::",
];

/// Each value of `keyword` in the schema objects below `value`, with its JSON path.
/// The walk does not go into instance values (`enum`, `const`, `default`, `examples`).
fn keyword_values<'a>(
    path: &str,
    value: &'a Value,
    keyword: &str,
    found: &mut Vec<(String, &'a Value)>,
) {
    match value {
        Value::Object(object) => {
            if let Some(held) = object.get(keyword) {
                found.push((path.to_owned(), held));
            }
            for (name, child) in object {
                if !["enum", "const", "default", "examples"].contains(&name.as_str()) {
                    keyword_values(&format!("{path}/{name}"), child, keyword, found);
                }
            }
        }
        Value::Array(values) => {
            for (index, child) in values.iter().enumerate() {
                keyword_values(&format!("{path}/{index}"), child, keyword, found);
            }
        }
        _ => {}
    }
}

/// Each listed schema, and each data schema that a `change` refusal gives, by name.
async fn schemas(session: &Session) -> Vec<(String, Value)> {
    let mut schemas = Vec::new();
    for tool in session.client.list_all_tools().await.unwrap() {
        schemas.push((format!("{} input", tool.name), json!(tool.input_schema)));
        schemas.push((format!("{} output", tool.name), json!(tool.output_schema)));
    }
    for kind in [
        "source",
        "requirement",
        "resolution",
        "rule",
        "topic",
        "question",
        "domain",
    ] {
        let refused = support::mcp::call(
            &session.client,
            "change",
            json!({"action": "create", "target": format!("{kind}_new"), "type": kind}),
        )
        .await;
        let schema = refused.structured_content.unwrap()["error"]["schema"].clone();
        assert!(schema.is_object(), "{kind}: {schema}");
        schemas.push((format!("change create {kind} data"), schema));
    }
    schemas
}

#[tokio::test]
async fn each_schema_format_is_a_json_schema_2020_12_format() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let mut problems = Vec::new();
    for (name, schema) in schemas(&session).await {
        let mut formats = Vec::new();
        keyword_values("", &schema, "format", &mut formats);
        for (path, format) in formats {
            if !format
                .as_str()
                .is_some_and(|format| FORMATS.contains(&format))
            {
                problems.push(format!("{name} {path}: {format}"));
            }
        }
        support::schema_check::compile(&schema);
    }
    assert_eq!(problems, Vec::<String>::new());
    session.close().await;
}

#[tokio::test]
async fn an_integer_range_is_a_minimum_and_a_maximum() {
    let repository = Repository::new("The shared graph is readable.");
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let (discussions, _) = support::schema_check::schemas(&session.client, "discussions").await;
    assert_eq!(
        discussions["properties"]["limit"],
        json!({"type": ["integer", "null"], "minimum": 1, "maximum": 200,
               "description": "Actions: discussions, discussion."})
    );
    let (change, _) = support::schema_check::schemas(&session.client, "change").await;
    assert_eq!(
        change["properties"]["expected_version"],
        json!({"type": "integer", "minimum": 0, "description": "Actions: reply."})
    );
    session.close().await;
}

#[tokio::test]
async fn no_schema_description_shows_rust_code() {
    let repository = Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let session = Session::start(support::fixture_host::full(&repository)).await;
    let mut problems = Vec::new();
    for (name, schema) in schemas(&session).await {
        let mut descriptions = Vec::new();
        keyword_values("", &schema, "description", &mut descriptions);
        for (path, description) in descriptions {
            let text = description.as_str().unwrap_or_default();
            if let Some(word) = RUST_WORDS.iter().find(|word| text.contains(*word)) {
                problems.push(format!("{name} {path}: {word} in {text:?}"));
            }
        }
    }
    assert_eq!(problems, Vec::<String>::new());
    session.close().await;
}

/// The schema check fails on a tool that tools/list does not have.
#[tokio::test]
#[should_panic(expected = "tools/list lacks no_such_tool")]
async fn the_schema_check_fails_on_a_tool_that_is_not_listed() {
    let repository = Repository::new("The shared graph is readable.");
    let session = Session::start(support::fixture_host::full(&repository)).await;
    support::schema_check::call(&session.client, "no_such_tool", json!({})).await;
}
