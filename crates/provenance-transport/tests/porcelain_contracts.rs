use jsonschema::JSONSchema;
use provenance_porcelain::check::{
    BindingContext, BindingPolicy, Category, CategoryRun, CheckInput, CheckPort, Finding,
    PortFuture, Refusal, StatementContext,
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[cfg(feature = "test-fixture")]
#[allow(dead_code)]
mod support {
    pub mod records;
    pub mod resource_http;
}

fn validator(schema: &Value) -> JSONSchema {
    JSONSchema::compile(schema).expect("emitted MCP schema is valid")
}

#[cfg(feature = "test-fixture")]
fn assert_get_schema_edges(input: &JSONSchema, output: &JSONSchema, valid: &Value) {
    for invalid in [
        json!({"target":""}),
        json!({"target":"req_shared","view":"invented"}),
        json!({"target":"req_shared","max_depth":0}),
        json!({"target":"req_shared","limit":0}),
        json!({"target":"req_shared","returned_kinds":["invented"]}),
    ] {
        assert!(!input.is_valid(&invalid), "invalid input {invalid}");
    }
    let mut missing_record = valid.clone();
    missing_record.as_object_mut().unwrap().remove("record");
    assert!(!output.is_valid(&missing_record));
    let mut without_metadata = valid.clone();
    without_metadata
        .as_object_mut()
        .unwrap()
        .remove("record_metadata");
    assert!(output.is_valid(&without_metadata));
    let mut null_metadata = valid.clone();
    null_metadata["record_metadata"] = Value::Null;
    assert!(output.is_valid(&null_metadata));
    let mut tagged_record = valid["record"]["value"].clone();
    tagged_record["node_type"] = json!("requirement");
    let record = serde_json::from_value(tagged_record).unwrap();
    let continued = provenance_porcelain::get::GetOutcome {
        record,
        result: provenance_porcelain::get::ViewResult::Children(
            provenance_porcelain::get::Traversal {
                records: Vec::new(),
                bounds: provenance_porcelain::get::Bounds {
                    limit: 1,
                    max_depth: Some(2),
                    has_more: true,
                    continuation: Some("next-page".into()),
                    truncated: true,
                },
                response_metadata: Some(provenance_core::protocol::ResponseMeta {
                    freshness_error: Some("catch-up failed".into()),
                    ..Default::default()
                }),
            },
        ),
        record_metadata: None,
    };
    let continued = serde_json::to_value(continued).unwrap();
    assert!(output.is_valid(&continued));
    assert_eq!(continued["bounds"]["continuation"], "next-page");
    assert_eq!(
        continued["view_metadata"]["freshness_error"],
        "catch-up failed"
    );
    assert!(continued.get("record_metadata").is_none());
    for (field, bad) in [
        (
            "record",
            json!({"id":"req_shared","kind":"requirement","value":42}),
        ),
        ("view", json!("invented")),
        (
            "related",
            json!([{"id":"rule_shared","kind":"rule","value":7,"depth":1}]),
        ),
        ("detail", json!("not impact data")),
        ("bounds", json!({"limit":"many"})),
        ("record_metadata", json!(5)),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = bad;
        assert!(!output.is_valid(&invalid), "invalid {field} was accepted");
    }
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
async fn emitted_get_contract_accepts_each_live_view_and_rejects_wrong_payloads() {
    use provenance_porcelain::get::{GetInput, View};
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let repository = support::records::Repository::new("The shared graph is readable.");
    let host = support::resource_http::host(&repository, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let get = tools.iter().find(|tool| tool.name == "get").unwrap();
    let input = validator(&json!(get.input_schema));
    let output = validator(&json!(get.output_schema.as_ref().unwrap()));

    let mut record_value = None;
    let cases = View::ALL
        .into_iter()
        .map(|view| {
            (
                if view == View::Grounding {
                    "rule_shared"
                } else {
                    "req_shared"
                },
                view,
            )
        })
        .chain(std::iter::once(("rule_shared", View::Record)));
    for (target, view) in cases {
        let arguments = json!({"target":target, "view":view.as_str()});
        assert!(input.is_valid(&arguments), "valid {view:?} input");
        let result = client
            .call_tool(
                CallToolRequestParams::new("get")
                    .with_arguments(arguments.as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        assert_ne!(result.is_error, Some(true), "{view:?}");
        let value = result.structured_content.as_ref().unwrap();
        assert!(
            output.is_valid(value),
            "{view:?}: {:?}",
            output
                .validate(value)
                .err()
                .map(|errors| errors.map(|error| error.to_string()).collect::<Vec<_>>())
        );

        let service = provenance_porcelain::Porcelain::new(
            provenance_transport::porcelain::HostGetPort::new(host.clone()),
        );
        let shared = service.get(GetInput::new(target, view)).await.unwrap();
        assert_eq!(value, &serde_json::to_value(&shared).unwrap());
        assert_eq!(
            result.content[0].as_text().unwrap().text,
            provenance_porcelain::get::render_readable(&shared).unwrap()
        );
        if view == View::Record {
            assert!(value["detail"].is_null());
            assert!(value["bounds"].is_null());
            assert!(value.get("view_metadata").is_none());
            if target == "req_shared" {
                record_value = Some(value.clone());
            }
        } else {
            assert!(value["bounds"].is_object());
            assert!(value["view_metadata"].is_object());
        }
    }

    assert_get_schema_edges(&input, &output, &record_value.unwrap());

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}

fn assert_check_context_edges(output: &JSONSchema) {
    use provenance_porcelain::check::{CategoryReport, CheckOutcome};

    let result = CheckOutcome {
        categories: vec![
            CategoryReport::from_run(CategoryRun::Statements {
                findings: vec![Finding::new("statement finding")],
                context: Some(StatementContext {
                    candidate_commit: "candidate".into(),
                    base_commit: Some("base".into()),
                }),
                refusal: Refusal::Findings,
            }),
            CategoryReport::from_run(CategoryRun::Bindings {
                findings: vec![],
                context: BindingContext {
                    policy: BindingPolicy::Error,
                },
                refusal: Refusal::None,
            }),
            CategoryReport::from_run(CategoryRun::Graph {
                findings: vec![Finding::with_detail("graph finding", json!(7))],
                refusal: Refusal::Findings,
            }),
        ],
    };
    let valid = serde_json::to_value(result).unwrap();
    assert!(output.is_valid(&valid), "populated check contexts: {valid}");
    assert_eq!(valid["categories"][0]["context"]["base_commit"], "base");
    assert_eq!(valid["categories"][1]["context"]["policy"], "error");
    assert_eq!(valid["categories"][2]["findings"][0]["detail"], 7);

    let mut missing_candidate = valid.clone();
    missing_candidate["categories"][0]["context"]
        .as_object_mut()
        .unwrap()
        .remove("candidate_commit");
    let mut wrong_base = valid.clone();
    wrong_base["categories"][0]["context"]["base_commit"] = json!(12);
    let mut unknown_policy = valid.clone();
    unknown_policy["categories"][1]["context"]["policy"] = json!("ignore");
    let mut missing_message = valid;
    missing_message["categories"][2]["findings"][0]
        .as_object_mut()
        .unwrap()
        .remove("message");
    for (label, candidate) in [
        ("missing statement candidate", missing_candidate),
        ("numeric base commit", wrong_base),
        ("unknown binding policy", unknown_policy),
        ("missing finding message", missing_message),
    ] {
        assert!(
            !output.is_valid(&candidate),
            "accepted {label}: {candidate}"
        );
    }
}

struct CategoryFixture {
    unavailable: AtomicBool,
}

impl CheckPort for CategoryFixture {
    fn run<'a>(&'a self, category: Category, _: Option<&'a str>) -> PortFuture<'a> {
        let unavailable = self.unavailable.load(Ordering::SeqCst);
        Box::pin(async move {
            Ok(match category {
                Category::Graph => CategoryRun::Graph {
                    findings: vec![Finding::with_detail(
                        "graph finding",
                        json!({"edge": [1, true]}),
                    )],
                    refusal: Refusal::Findings,
                },
                Category::Statements => CategoryRun::Statements {
                    findings: vec![Finding::new("statement finding")],
                    context: Some(StatementContext {
                        candidate_commit: "candidate".into(),
                        base_commit: None,
                    }),
                    refusal: Refusal::Findings,
                },
                Category::Bindings if unavailable => return Err("bindings are unavailable".into()),
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

#[tokio::test]
async fn emitted_check_contract_accepts_all_categories_contexts_and_unavailable_results() {
    use rmcp::{model::CallToolRequestParams, ServiceExt as _};

    let fixture = Arc::new(CategoryFixture {
        unavailable: AtomicBool::new(false),
    });
    let port: Arc<dyn CheckPort> = fixture.clone();
    let host = provenance_transport::StatementHost::default().with_check_port(port.clone());
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server = tokio::spawn(async move { host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let check = tools.iter().find(|tool| tool.name == "check").unwrap();
    let input = validator(&json!(check.input_schema));
    let output = validator(&json!(check.output_schema.as_ref().unwrap()));
    for category in Category::ALL {
        let request = json!({"categories":[category]});
        assert!(input.is_valid(&request), "{category:?}");
    }
    assert!(!input.is_valid(&json!({"categories":["invented"]})));
    assert!(!input.is_valid(&json!({"extra":true})));
    assert!(!input.is_valid(&json!({"scope":"other"})));

    let arguments = json!({});
    let result = client
        .call_tool(
            CallToolRequestParams::new("check")
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    assert_ne!(result.is_error, Some(true));
    let value = result.structured_content.as_ref().unwrap();
    assert!(
        output.is_valid(value),
        "{:?}",
        output
            .validate(value)
            .err()
            .map(|errors| errors.map(|error| error.to_string()).collect::<Vec<_>>())
    );
    let shared = provenance_porcelain::Porcelain::new(port.clone())
        .check(CheckInput::default())
        .await;
    assert_eq!(value, &serde_json::to_value(&shared).unwrap());
    assert_eq!(
        result.content[0].as_text().unwrap().text,
        provenance_porcelain::check::render_readable(&shared)
    );
    assert_eq!(
        value["categories"].as_array().unwrap().len(),
        Category::ALL.len()
    );
    assert_eq!(
        value["categories"][1]["context"]["base_commit"],
        Value::Null
    );
    assert_eq!(
        value["categories"][0]["findings"][0]["detail"]["edge"],
        json!([1, true])
    );
    assert_eq!(value["categories"][2]["context"]["policy"], "warning");

    for (field, bad) in [
        ("category", json!("invented")),
        ("status", json!("unknown")),
        ("findings", json!([{"message":8}])),
        ("context", json!("not a context")),
    ] {
        let mut invalid = value.clone();
        invalid["categories"][1][field] = bad;
        assert!(!output.is_valid(&invalid), "invalid {field} was accepted");
    }
    assert_check_context_edges(&output);

    fixture.unavailable.store(true, Ordering::SeqCst);
    let arguments = json!({"categories":["bindings"]});
    let unavailable = client
        .call_tool(
            CallToolRequestParams::new("check")
                .with_arguments(arguments.as_object().unwrap().clone()),
        )
        .await
        .unwrap();
    let value = unavailable.structured_content.as_ref().unwrap();
    assert!(output.is_valid(value));
    assert_eq!(value["categories"][0]["status"], "unavailable");
    assert_eq!(
        value["categories"][0]["unavailable_reason"],
        "bindings are unavailable"
    );

    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
