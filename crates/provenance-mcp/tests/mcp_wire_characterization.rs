//! Characterization of the MCP wire surface, without a Rule binding.
//!
//! These tests pin the JSON values and response shapes that the MCP library
//! writes for our server, so a library upgrade that changes them fails here.
//! They speak raw JSON-RPC over an in-memory stream and do not use the
//! library's client.
#![cfg(feature = "test-fixture")]

use crate::support;

use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use serde_json::{json, Value};
use std::sync::Arc;
use support::{check_port::FixtureCheckPort, first_calls, records::Repository};
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, DuplexStream, Lines};

const TOOL_LIST_FNV1A: u64 = 9_591_195_474_183_200_660;

/// The porcelain tools, in list order. The list has no plumbing tools.
const PORCELAIN_TOOL_NAMES: &[&str] = &[
    "get",
    "search",
    "api",
    "check",
    "trace",
    "change",
    "discussions",
];

/// The request metadata of a 2026-07-28 client, which has no `initialize` handshake.
fn request_meta() -> Value {
    json!({
        "io.modelcontextprotocol/protocolVersion":"2026-07-28",
        "io.modelcontextprotocol/clientCapabilities":{},
        "io.modelcontextprotocol/clientInfo":{"name":"characterization","version":"0"}
    })
}

/// One raw JSON-RPC peer connected to one served host.
struct Wire {
    lines: Lines<BufReader<tokio::io::ReadHalf<DuplexStream>>>,
    writer: tokio::io::WriteHalf<DuplexStream>,
    next_id: u64,
    /// The `_meta` that each request carries; none after an `initialize` handshake.
    meta: Option<Value>,
}

impl Wire {
    /// A 2026-07-28 peer: each request carries the protocol metadata.
    fn start(host: StatementHost) -> Self {
        let mut wire = Self::bare(host);
        wire.meta = Some(request_meta());
        wire
    }

    /// A peer that sends each request without metadata.
    fn bare(host: StatementHost) -> Self {
        let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
        tokio::spawn(async move {
            let running = provenance_mcp::serve(host, server_io).await.unwrap();
            let _ = running.waiting().await;
        });
        let (reader, writer) = tokio::io::split(client_io);
        Self {
            lines: BufReader::new(reader).lines(),
            writer,
            next_id: 1,
            meta: None,
        }
    }

    async fn send(&mut self, message: &Value) {
        let mut line = serde_json::to_vec(message).unwrap();
        line.push(b'\n');
        self.writer.write_all(&line).await.unwrap();
        self.writer.flush().await.unwrap();
    }

    /// Send one request and return the whole response message.
    async fn request(&mut self, method: &str, mut params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        if let Some(meta) = &self.meta {
            params["_meta"] = meta.clone();
        }
        self.send(&json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}))
            .await;
        let line = self.lines.next_line().await.unwrap().unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(response["id"], id, "{response}");
        response
    }

    /// Initialize with one requested protocol version and return the result.
    async fn initialize(&mut self, protocol_version: &str) -> Value {
        let response = self
            .request(
                "initialize",
                json!({
                    "protocolVersion":protocol_version,
                    "capabilities":{},
                    "clientInfo":{"name":"characterization","version":"0"}
                }),
            )
            .await;
        self.send(&json!({"jsonrpc":"2.0","method":"notifications/initialized"}))
            .await;
        response["result"].clone()
    }
}

fn fnv1a(text: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn full_host(repository: &Repository) -> StatementHost {
    let access = FixtureAccess::new(
        vec![Target {
            id: "selected".into(),
            root: repository.dir.path().to_path_buf(),
        }],
        vec![("selected".into(), "default".into())],
        "fixture-secret",
        "fixture.test",
    )
    .unwrap()
    .allow_writes();
    StatementHost::with_fixture_access(access).with_check_port(Arc::new(FixtureCheckPort))
}

fn check_host() -> StatementHost {
    StatementHost::default().with_check_port(Arc::new(FixtureCheckPort))
}

/// The instructions of the full host up to its first calls: it shows each command and each
/// record and Discussion write, and it has no notify port and no repository files.
const INSTRUCTIONS: &str = "# Provenance\n\n\
Provenance records requirements, decisions, and the rules that connect them to code.\n\n\
## Records\n\n\
- Requirement: an obligation the system must satisfy. Requirements can refine other \
Requirements.\n\
- Rule: an identified atomic behavioural obligation that refines one or more \
Requirements. A Rule can exist before code or verification.\n\
- Resolution: a recorded decision that removes ambiguity. A Resolution can produce a \
Rule when it establishes a precise obligation.\n\
- Source: identified evidence that graph records can cite. A citation records where an \
obligation came from.\n\
- Topic: a claimable shaping work area attached to a Requirement.\n\
- Question: a concern to resolve, attached to a Topic and a Requirement, with a \
selected resolution method.\n\
- Discussion: one concern identified by a root Message and its replies inside a \
Thread.\n\
- Description: on a Requirement or Rule, how the product behaves in the present tense: \
states, what changes them, what the person sees, one example. History goes on the \
thread; unknowns go in a Question.\n\n\
## Code and evidence\n\n\
An Implementation binding connects a Rule to production code that realizes it.\n\
Verification is evidence that a Rule holds. A Verification binding identifies intended \
evidence; a Verification run records one execution and its result.\n\
Rule lifecycle, decision grounding, implementation, and verification are separate \
facts. An active Rule can precede its implementation.\n\
Unimplemented and Unverified describe absent bindings or evidence; they are not stored \
Rule statuses. A graph read does not scan code or establish a coverage verdict.\n\
A source citation does not count as an Implementation binding. Coverage checks \
evaluate code bindings separately from graph reads.\n\n\
## Actions\n\n\
Use search to find IDs, then get to read the selected record and its related views. \
Create names an explicit record type; actions on an existing record infer its type.\n\
Before an update, read the record with get. Give its `edit.etag` unchanged to the \
update as the if-match value.\n\
Available actions depend on the interface and its access. Each interface lists the \
inputs and actions it supports.\n\n\
- get: Read one repository record by its repository-local ID. Select its record view, \
children view (records below this record), trace view (records above this record, up \
to its Sources), impact view, or review view.\n\
- search: Find records across the permitted kinds in the bound scope, or the Rules \
bound to a source file and symbol.\n\
- api: Call one public API path with an optional method, query, headers, and JSON \
body. The body is the data object, without the HTTP {\"data\": ...} wrapper. Omit the \
path to list the catalog routes with their methods and inputs. A filter that is the \
method and path of one route, such as GET /rules, gives the request and response \
schemas of that route.\n\
- check: Check graph validity, statement quality, and binding coverage. Give a \
statement to check that text with the project dictionary before a write.\n\
- trace: Read the records above this record, up to its Sources. The get trace view \
gives the same records.\n\
- change: Write to the graph. The action argument selects the write: create, update, \
answer, claim, release, or submit on the target record; or discuss or reply for a \
Discussion. A call that lacks a field that the action and record kind need gets the \
schema of their fields.\n\
- create: Create the target ID as an explicit record type.\n\
- update: Update the existing target while preserving omitted fields.\n\
- answer: Answer the target Question.\n\
- claim: Claim the target Topic.\n\
- release: Release the target Topic claim.\n\
- submit: Submit the target Source, Requirement, Resolution, Rule, Topic, or Question \
for review.\n\
- discuss: Start a Discussion under the target record.\n\
- reply: Reply to the addressed Discussion.\n\
- discussions: List addressed Discussions in a scope or under a record.\n\
- discussion: Read the addressed Discussion.\n\n\
For a focused start, ";

/// `result` with its instructions up to the end of the words before the first calls. The
/// first calls are pinned in `mcp_instructions.rs`.
fn without_first_calls(mut result: Value) -> Value {
    let instructions = result["instructions"].as_str().unwrap_or_default();
    result["instructions"] = Value::String(first_calls::without_first_calls(instructions));
    result
}

fn server_info() -> Value {
    json!({"name":"provenance","version":env!("CARGO_PKG_VERSION")})
}

#[tokio::test]
async fn discover_response_keeps_its_fields() {
    let repository = Repository::new("The shared graph is readable.");
    let mut wire = Wire::start(full_host(&repository));
    let response = wire.request("server/discover", json!({})).await;
    assert_eq!(
        without_first_calls(response["result"].clone()),
        json!({
            "_meta":{"io.modelcontextprotocol/serverInfo":server_info()},
            "cacheScope":"private",
            "capabilities":{"tools":{}},
            "instructions":INSTRUCTIONS,
            "resultType":"complete",
            "supportedVersions":["2024-11-05","2025-03-26","2025-06-18","2025-11-25","2026-07-28"],
            "ttlMs":0
        })
    );
}

/// A client of an older protocol initializes; a request for 2026-07-28 or a later
/// version through `initialize` gets the newest version that has the handshake.
#[tokio::test]
async fn initialize_negotiates_the_older_protocols() {
    for (requested, agreed) in [
        ("2025-11-25", "2025-11-25"),
        ("2025-06-18", "2025-06-18"),
        ("2024-11-05", "2024-11-05"),
        ("2026-07-28", "2025-11-25"),
        ("2099-01-01", "2025-11-25"),
    ] {
        let repository = Repository::new("The shared graph is readable.");
        let mut wire = Wire::bare(full_host(&repository));
        let result = wire.initialize(requested).await;
        assert_eq!(
            without_first_calls(result),
            json!({
                "protocolVersion":agreed,
                "capabilities":{"tools":{}},
                "serverInfo":server_info(),
                "instructions":INSTRUCTIONS,
            }),
            "requested {requested}"
        );
    }
}

/// Without `initialize`, each request must carry the protocol metadata.
#[tokio::test]
async fn a_request_without_metadata_or_handshake_is_refused() {
    let mut wire = Wire::bare(check_host());
    let response = wire.request("tools/list", json!({})).await;
    assert_eq!(
        response["error"],
        json!({
            "code":-32602,
            "message":"request _meta is missing or has malformed required fields: \
                io.modelcontextprotocol/protocolVersion, io.modelcontextprotocol/clientCapabilities"
        })
    );
}

#[tokio::test]
async fn tool_list_keeps_its_names_order_and_schemas() {
    let repository = Repository::new("The shared graph is readable.");
    let mut wire = Wire::start(full_host(&repository));
    let response = wire.request("tools/list", json!({})).await;
    let result = &response["result"];
    let names = result["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names, PORCELAIN_TOOL_NAMES);
    assert_eq!(
        (
            &result["cacheScope"],
            &result["resultType"],
            &result["ttlMs"]
        ),
        (&json!("private"), &json!("complete"), &json!(0))
    );
    assert_eq!(
        result["_meta"],
        json!({"io.modelcontextprotocol/serverInfo":server_info()})
    );
    assert_eq!(fnv1a(&result["tools"].to_string()), TOOL_LIST_FNV1A);
}

#[tokio::test]
async fn tool_call_result_keeps_its_shape() {
    let call = json!({"name":"check","arguments":{"graph": true}});
    let mut expected = json!({
        "content":[{"type":"text","text":"graph: passed"}],
        "isError":false,
        "structuredContent":{
            "categories":[{"category":"graph","status":"passed","findings":[]}]
        }
    });
    let mut legacy = Wire::bare(check_host());
    legacy.initialize("2025-06-18").await;
    let response = legacy.request("tools/call", call.clone()).await;
    assert_eq!(response["result"], expected, "2025-06-18");
    let mut wire = Wire::start(check_host());
    let response = wire.request("tools/call", call).await;
    expected["resultType"] = json!("complete");
    expected["_meta"] = json!({"io.modelcontextprotocol/serverInfo":server_info()});
    assert_eq!(response["result"], expected, "2026-07-28");
}

/// A tool that the host does not list gets the error of any unknown tool.
#[tokio::test]
async fn a_tool_that_the_host_does_not_list_is_unknown() {
    let mut wire = Wire::start(check_host());
    let listed = wire.request("tools/list", json!({})).await;
    let names = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].clone())
        .collect::<Vec<_>>();
    assert_eq!(names, [json!("api"), json!("check")]);
    for name in ["change", "get", "discussions", "search", "trace"] {
        let response = wire
            .request(
                "tools/call",
                json!({"name":name,"arguments":{"action":"create"}}),
            )
            .await;
        assert_eq!(
            response["error"],
            json!({"code":-32602,"message":format!("Unknown tool: {name}")}),
            "{name}"
        );
    }
}

#[tokio::test]
async fn typed_refusal_and_unknown_tool_keep_their_shape() {
    let mut wire = Wire::start(check_host());
    let refusal = wire
        .request(
            "tools/call",
            json!({"name":"check","arguments":{"statement":"The pump stops.","categories":[]}}),
        )
        .await;
    let envelope = json!({
        "error":{"field":"categories","kind":"invalid_input","reason":"unknown_field",
                 "message":"check does not take categories; it takes: base, baseline, bindings, \
                            docs, gaps, graph, output, output_format, path, scope, statement, \
                            statements, strict"},
        "meta":{}
    });
    assert_eq!(
        refusal,
        json!({
            "jsonrpc":"2.0",
            "id":1,
            "result":{
                "content":[{"type":"text","text":envelope.to_string()}],
                "isError":true,
                "resultType":"complete",
                "structuredContent":envelope,
                "_meta":{"io.modelcontextprotocol/serverInfo":server_info()}
            }
        })
    );
    let unknown = wire
        .request("tools/call", json!({"name":"no-such-tool","arguments":{}}))
        .await;
    assert_eq!(
        unknown,
        json!({
            "jsonrpc":"2.0",
            "id":2,
            "error":{"code":-32602,"message":"Unknown tool: no-such-tool"}
        })
    );
}

/// The error object that each tool returns for an input field it does not accept,
/// then the error object for an oversized input.
fn refusals() -> Vec<(&'static str, Value)> {
    let unknown = |tool: &str, takes: &str| {
        json!({"field":"unknown_field","kind":"invalid_input","reason":"unknown_field",
               "message":format!("{tool} does not take unknown_field; it takes: {takes}")})
    };
    let discussion = json!({
        "field":"unknown_field","kind":"invalid_input","reason":"unknown_field",
        "message":"the discussions action does not take unknown_field; it takes: cursor, limit, status, target"
    });
    let change = json!({
        "field":"action","kind":"invalid_input","reason":"required",
        "message":"give an action: create, update, answer, claim, release, submit, discuss, reply"
    });
    vec![
        (
            "get",
            unknown("get", "depth, kind, limit, review_link, target, view"),
        ),
        (
            "search",
            unknown(
                "search",
                "cursor, file, include_terminal, kind, limit, line, symbol, text",
            ),
        ),
        (
            "api",
            unknown("api", "filter, header, input, limit, method, path, query"),
        ),
        (
            "check",
            unknown(
                "check",
                "base, baseline, bindings, docs, gaps, graph, output, output_format, path, \
                 scope, statement, statements, strict",
            ),
        ),
        ("trace", unknown("trace", "depth, kind, limit, target")),
        ("change", change),
        ("discussions", discussion),
        (
            "too_large",
            json!({"field":null,"kind":"invalid_input","reason":"too_large"}),
        ),
    ]
}

#[tokio::test]
async fn each_tool_refusal_keeps_its_text() {
    let repository = Repository::new("The shared graph is readable.");
    let mut wire = Wire::start(full_host(&repository));
    let oversized = "x".repeat(provenance_transport::MAX_BODY_BYTES + 1);
    for (label, error) in refusals() {
        let call = if label == "too_large" {
            json!({"name":"get","arguments":{"id":oversized}})
        } else {
            json!({"name":label,"arguments":{"unknown_field":1}})
        };
        let response = wire.request("tools/call", call).await;
        let envelope = json!({"error":error,"meta":{}});
        assert_eq!(
            response["result"],
            json!({
                "content":[{"type":"text","text":envelope.to_string()}],
                "isError":true,
                "resultType":"complete",
                "structuredContent":envelope,
                "_meta":{"io.modelcontextprotocol/serverInfo":server_info()}
            }),
            "{label}"
        );
    }
    assert_eq!(
        refusals().len(),
        PORCELAIN_TOOL_NAMES.len() + 1,
        "one refusal per tool and one oversized input"
    );
}
