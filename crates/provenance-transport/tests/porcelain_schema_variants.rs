use jsonschema::JSONSchema;
use provenance_core::protocol::{GraphNode, TracedNode};
use provenance_core::NodeType;
use provenance_porcelain::get::{Bounds, GetInput, GetOutcome, Traversal, View, ViewResult};
use rmcp::{model::CallToolRequestParams, ServiceExt as _};
use serde_json::{json, Value};

#[cfg(feature = "test-fixture")]
#[allow(dead_code)]
mod support {
    pub mod records;
    pub mod resource_http;
}

fn target(kind: NodeType) -> String {
    match kind {
        NodeType::Requirement => "req_shared".into(),
        _ => format!("{}_shared", kind.as_str()),
    }
}

fn populate(node: &mut GraphNode) {
    match node {
        GraphNode::Source(record) => record.reference = Some("Section 1".into()),
        GraphNode::Requirement(record) => record.description = Some("A readable record.".into()),
        GraphNode::Resolution(record) => record.context = Some("The graph has records.".into()),
        GraphNode::Rule(record) => record.name = Some("Shared rule".into()),
        GraphNode::Topic(record) => record.claimed_by = Some("reviewer".into()),
        GraphNode::Question(record) => record.answer = Some("The shared record.".into()),
        GraphNode::Domain(record) => record.description = Some("Shared records.".into()),
        GraphNode::Boundary(record) => {
            record.source_ref = Some(provenance_core::SourceReference {
                source_id: provenance_core::StableId::new("source_shared").unwrap(),
                clause: Some("Section 1".into()),
            });
        }
    }
}

fn assert_canonical_record(schema: &JSONSchema, value: &Value, node: &GraphNode) {
    let record = &value["record"];
    assert_eq!(record["id"], node.id().as_str());
    assert_eq!(record["kind"], node.node_type().as_str());
    assert!(record["value"].get("node_type").is_none());
    let mut bare = serde_json::to_value(node).unwrap();
    bare.as_object_mut().unwrap().remove("node_type");
    assert_eq!(record["value"], bare);
    assert!(schema.is_valid(value), "{}: {value}", node.node_type().as_str());
}

fn assert_tagged_related(schema: &JSONSchema, nodes: &[GraphNode]) -> Value {
    let outcome = GetOutcome {
        record: nodes[0].clone(),
        result: ViewResult::Children(Traversal {
            records: nodes
                .iter()
                .cloned()
                .map(|node| TracedNode { depth: 1, node })
                .collect(),
            bounds: Bounds {
                limit: nodes.len(),
                max_depth: Some(1),
                has_more: false,
                continuation: None,
                truncated: false,
            },
            response_metadata: None,
        }),
        record_metadata: None,
    };
    let value = serde_json::to_value(outcome).unwrap();
    assert!(schema.is_valid(&value), "tagged related records: {value}");
    for (related, node) in value["related"].as_array().unwrap().iter().zip(nodes) {
        assert_eq!(related["id"], node.id().as_str());
        assert_eq!(related["kind"], node.node_type().as_str());
        assert_eq!(related["value"], serde_json::to_value(node).unwrap());
        assert_eq!(related["value"]["node_type"], node.node_type().as_str());
    }
    value
}

fn assert_rejected_changes(schema: &JSONSchema, valid: &Value, changes: &[(&str, Value)]) {
    for (label, candidate) in changes {
        assert!(!schema.is_valid(candidate), "accepted {label}: {candidate}");
    }
    assert!(schema.is_valid(valid));
}

fn assert_invalid_record_payloads(schema: &JSONSchema, valid: &Value) {
    let mut missing_id = valid.clone();
    missing_id["record"]["value"]
        .as_object_mut()
        .unwrap()
        .remove("id");
    let mut wrong_statement = valid.clone();
    wrong_statement["record"]["value"]["statement"] = json!(7);
    let mut missing_statement = valid.clone();
    missing_statement["record"]["value"]
        .as_object_mut()
        .unwrap()
        .remove("statement");
    let mut invalid_status = valid.clone();
    invalid_status["record"]["value"]["status"] = json!("invented");
    assert_rejected_changes(
        schema,
        valid,
        &[
            ("missing canonical ID", missing_id),
            ("numeric statement", wrong_statement),
            ("missing statement", missing_statement),
            ("unknown status", invalid_status),
        ],
    );
}

fn assert_invalid_related_payloads(schema: &JSONSchema, valid: &Value) {
    let mut missing_tag = valid.clone();
    missing_tag["related"][0]["value"]
        .as_object_mut()
        .unwrap()
        .remove("node_type");
    let mut invalid_tag = valid.clone();
    invalid_tag["related"][0]["value"]["node_type"] = json!("invented");
    let mut wrong_name = valid.clone();
    wrong_name["related"][0]["value"]["name"] = json!(7);
    let mut invalid_kind = valid.clone();
    invalid_kind["related"][0]["kind"] = json!("invented");
    let mut zero_depth = valid.clone();
    zero_depth["related"][0]["depth"] = json!(0);
    assert_rejected_changes(
        schema,
        valid,
        &[
            ("missing related tag", missing_tag),
            ("unknown related tag", invalid_tag),
            ("numeric source name", wrong_name),
            ("unknown related kind", invalid_kind),
            ("zero related depth", zero_depth),
        ],
    );
}

#[cfg(feature = "test-fixture")]
#[tokio::test]
async fn emitted_get_schema_accepts_each_canonical_record_and_tagged_related_kind() {
    let repository = support::records::Repository::new("The shared graph is readable.");
    repository.all_kinds();
    let host = support::resource_http::host(&repository, false);
    let (client_io, server_io) = tokio::io::duplex(256 * 1024);
    let server_host = host.clone();
    let server = tokio::spawn(async move { server_host.serve_mcp(server_io).await.unwrap() });
    let client = ().serve(client_io).await.unwrap();
    let tools = client.list_all_tools().await.unwrap();
    let get = tools.iter().find(|tool| tool.name == "get").unwrap();
    let emitted = json!(get.output_schema.as_ref().unwrap());
    assert_eq!(emitted["$schema"], "https://json-schema.org/draft/2020-12/schema");
    let schema = JSONSchema::compile(&emitted).expect("emitted schema and refs resolve");
    let service = provenance_porcelain::Porcelain::new(
        provenance_transport::porcelain::HostGetPort::new(host),
    );
    let mut nodes = Vec::new();
    let mut requirement = None;
    for kind in NodeType::ALL {
        let id = target(kind);
        let arguments = json!({"target":id,"view":"record"});
        let response = client
            .call_tool(
                CallToolRequestParams::new("get")
                    .with_arguments(arguments.as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        assert_ne!(response.is_error, Some(true), "{kind:?}");
        let value = response.structured_content.as_ref().unwrap();
        let shared = service.get(GetInput::new(&id, View::Record)).await.unwrap();
        assert_eq!(value, &serde_json::to_value(&shared).unwrap());
        assert_canonical_record(&schema, value, &shared.record);
        if kind == NodeType::Requirement {
            requirement = Some(value.clone());
        }
        nodes.push(shared.record);
    }
    assert_invalid_record_payloads(&schema, &requirement.unwrap());
    for node in &mut nodes {
        populate(node);
        let outcome = GetOutcome {
            record: node.clone(),
            result: ViewResult::Record,
            record_metadata: None,
        };
        assert_canonical_record(&schema, &serde_json::to_value(outcome).unwrap(), node);
    }
    let related = assert_tagged_related(&schema, &nodes);
    assert_invalid_related_payloads(&schema, &related);
    client.cancel().await.unwrap();
    server.await.unwrap().cancel().await.unwrap();
}
