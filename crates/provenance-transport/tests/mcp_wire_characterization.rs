//! Characterization of the MCP wire surface, without a Rule binding.
//!
//! These tests pin the bytes that the MCP library writes for our server, so a
//! library upgrade that changes them fails here. They speak raw JSON-RPC over
//! an in-memory stream and do not use the library's client.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod records;
}

use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryRun, CheckPort, PortFuture, Refusal,
};
use provenance_transport::{
    fixture::{FixtureAccess, Target},
    StatementHost,
};
use serde_json::{json, Value};
use std::sync::Arc;
use support::records::Repository;
use tokio::io::{AsyncBufReadExt as _, AsyncWriteExt as _, BufReader, DuplexStream, Lines};

const TOOL_LIST_FNV1A: u64 = 9_728_329_283_418_856_131;

/// The tools that follow the catalog tools, in list order.
const PORCELAIN_TOOL_NAMES: &[&str] = &[
    "get",
    "search",
    "api",
    "check",
    "create",
    "update",
    "answer",
    "claim",
    "release",
    "submit",
    "discussions",
    "discussion",
    "discuss",
    "reply",
];

struct FixtureCheckPort;

impl CheckPort for FixtureCheckPort {
    fn run<'a>(&'a self, category: Category, _: Option<&'a str>) -> PortFuture<'a> {
        Box::pin(async move {
            Ok(match category {
                Category::Graph => CategoryRun::Graph {
                    findings: Vec::new(),
                    refusal: Refusal::None,
                },
                Category::Statements => CategoryRun::Statements {
                    findings: Vec::new(),
                    context: None,
                    refusal: Refusal::None,
                },
                Category::Bindings => CategoryRun::Bindings {
                    findings: Vec::new(),
                    context: BindingContext {
                        policy: BindingPolicy::Warning,
                    },
                    refusal: Refusal::None,
                },
            })
        })
    }
}

/// One raw JSON-RPC peer connected to one served host.
struct Wire {
    lines: Lines<BufReader<tokio::io::ReadHalf<DuplexStream>>>,
    writer: tokio::io::WriteHalf<DuplexStream>,
    next_id: u64,
}

impl Wire {
    fn start(host: StatementHost) -> Self {
        let (client_io, server_io) = tokio::io::duplex(1024 * 1024);
        tokio::spawn(async move {
            let running = host.serve_mcp(server_io).await.unwrap();
            let _ = running.waiting().await;
        });
        let (reader, writer) = tokio::io::split(client_io);
        Self {
            lines: BufReader::new(reader).lines(),
            writer,
            next_id: 1,
        }
    }

    async fn send(&mut self, message: &Value) {
        let mut line = serde_json::to_vec(message).unwrap();
        line.push(b'\n');
        self.writer.write_all(&line).await.unwrap();
        self.writer.flush().await.unwrap();
    }

    /// Send one request and return the whole response message.
    async fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
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

#[tokio::test]
async fn initialize_response_keeps_its_fields() {
    for requested in ["2025-06-18", "2024-11-05"] {
        let mut wire = Wire::start(StatementHost::default());
        let mut result = wire.initialize(requested).await;
        // The library reports its own package version here.
        let version = result["serverInfo"]
            .as_object_mut()
            .unwrap()
            .remove("version")
            .unwrap();
        assert_ne!(version.as_str().unwrap(), "");
        assert_eq!(
            result,
            json!({
                "protocolVersion":requested,
                "capabilities":{"tools":{}},
                "serverInfo":{"name":"rmcp"},
                "instructions":provenance_porcelain::guidance::guidance().guidance,
            }),
            "requested {requested}"
        );
    }
}

#[tokio::test]
async fn tool_list_keeps_its_names_order_and_schemas() {
    let repository = Repository::new("The shared graph is readable.");
    let mut wire = Wire::start(full_host(&repository));
    wire.initialize("2025-06-18").await;
    let response = wire.request("tools/list", json!({})).await;
    let result = &response["result"];
    let names = result["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(names.len(), 190);
    let (catalog, porcelain) = names.split_at(names.len() - PORCELAIN_TOOL_NAMES.len());
    let definitions = provenance_contract::definitions()
        .iter()
        .map(|definition| definition.name)
        .collect::<Vec<_>>();
    assert_eq!(catalog, definitions);
    assert_eq!(porcelain, PORCELAIN_TOOL_NAMES);
    assert_eq!(fnv1a(&result.to_string()), TOOL_LIST_FNV1A);
}

#[tokio::test]
async fn tool_call_result_keeps_its_shape() {
    let mut wire = Wire::start(StatementHost::default());
    wire.initialize("2025-06-18").await;
    let response = wire
        .request(
            "tools/call",
            json!({
                "name":"check-statement",
                "arguments":{"data":{"statement":"The system stores records."}}
            }),
        )
        .await;
    let mut result = response["result"].clone();
    let text = result["content"][0]["text"].as_str().unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(text).unwrap(),
        result["structuredContent"]
    );
    result["content"][0]["text"] = json!("<structured content>");
    // The analyzer reports the package version of this build.
    let analyzer = result["structuredContent"]["data"]
        .as_object_mut()
        .unwrap()
        .remove("analyzer_version")
        .unwrap();
    assert!(analyzer.is_string());
    assert_eq!(
        result,
        json!({
            "content":[{"type":"text","text":"<structured content>"}],
            "isError":false,
            "structuredContent":{
                "data":{"findings":[],"issue":9,"standard":"ASD-STE100"},
                "meta":{}
            }
        })
    );
}

#[tokio::test]
async fn typed_refusal_and_unknown_tool_keep_their_shape() {
    let mut wire = Wire::start(StatementHost::default());
    wire.initialize("2025-06-18").await;
    let refusal = wire
        .request(
            "tools/call",
            json!({"name":"check-statement","arguments":{"data":{}}}),
        )
        .await;
    let envelope = json!({
        "error":{"field":"request.statement","kind":"invalid_input","reason":"required"},
        "meta":{}
    });
    assert_eq!(
        refusal,
        json!({
            "jsonrpc":"2.0",
            "id":2,
            "result":{
                "content":[{"type":"text","text":envelope.to_string()}],
                "isError":true,
                "structuredContent":envelope
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
            "id":3,
            "error":{"code":-32601,"message":"Unknown tool"}
        })
    );
}
