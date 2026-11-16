//! The first steps of the server instructions are calls that the listed tools accept.
#![cfg(feature = "test-fixture")]

use crate::support::{fixture_host, mcp::Session, records::Repository};
use serde_json::{Map, Value};

/// The words before the first calls of the instructions.
const FIRST_STEPS: &str = "For a focused start, ";

/// The record that the fixture repository holds.
const RECORD: &str = "req_shared";

/// The calls of the first steps of `instructions`: the tool name before each ` with {`,
/// and the input object after it.
fn first_step_calls(instructions: &str) -> Vec<(String, String)> {
    let (_, steps) = instructions
        .split_once(FIRST_STEPS)
        .expect("the instructions give first steps");
    let mut calls = Vec::new();
    let mut rest = steps;
    while let Some((before, after)) = rest.split_once(" with {") {
        let tool = before.split_whitespace().last().expect("a tool name");
        let (input, after) = after.split_once('}').expect("the end of the input");
        calls.push((tool.to_owned(), format!("{{{input}}}")));
        rest = after;
    }
    calls
}

/// The input of a call with an example value for each placeholder: a count for `<N>`,
/// the fixture record for `<id>`, and a word of that record for another placeholder.
fn with_examples(input: &str) -> Map<String, Value> {
    let mut text = input.replace("<N>", "1");
    while let Some(start) = text.find("\"<") {
        let end = start + text[start..].find(">\"").expect("the end of a placeholder") + 2;
        let example = if &text[start..end] == "\"<id>\"" {
            RECORD
        } else {
            "shared"
        };
        text.replace_range(start..end, &format!("\"{example}\""));
    }
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{text} is not JSON: {error}"))
}

/// Each first step of the instructions, with an example value for each placeholder, passes
/// the input check of its tool, and the tool answers it.
#[tokio::test]
#[provenance_macros::verifies("rule_guidance_first_steps_are_accepted_calls", examples)]
async fn each_first_step_is_a_call_that_its_tool_accepts() {
    let repository = Repository::new("The shared graph is readable.");
    let session = Session::start(fixture_host::full(&repository)).await;
    let instructions = session
        .client
        .peer_info()
        .and_then(|info| info.instructions.clone())
        .expect("server instructions");
    let calls = first_step_calls(&instructions);
    assert!(calls.len() > 1, "{instructions}");
    for (tool, input) in calls {
        let arguments = with_examples(&input);
        let answer = crate::support::mcp::call(&session.client, &tool, arguments.into()).await;
        assert_ne!(
            answer.is_error,
            Some(true),
            "the {tool} tool refuses the first step {input}: {:?}\n{instructions}",
            answer.structured_content
        );
    }
    session.close().await;
}
