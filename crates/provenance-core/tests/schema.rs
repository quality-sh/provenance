#![cfg(feature = "schema")]
use provenance_core::protocol::{EvidenceResult, SearchQuery, TraceQuery, TypedSpecInput};
use schemars::{
    generate::{Contract, SchemaSettings},
    JsonSchema,
};
use serde_json::{json, Value};

fn schema<T: JsonSchema>(contract: Contract) -> Value {
    serde_json::to_value(
        SchemaSettings::draft2020_12()
            .with(|s| s.contract = contract)
            .into_generator()
            .into_root_schema_for::<T>(),
    )
    .unwrap()
}
fn valid(schema: &Value, value: &Value) -> bool {
    jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(schema)
        .unwrap()
        .is_valid(value)
}
#[test]
fn query_defaults_and_domain_bounds_are_explicit() {
    let search = schema::<SearchQuery>(Contract::Deserialize);
    assert!(valid(&search, &json!({"text":"x"})));
    assert_eq!(search["properties"]["limit"]["default"], 50);
    for limit in [0, 201] {
        assert!(
            !valid(&search, &json!({"text":"x","limit":limit})),
            "accepted limit {limit}"
        );
    }
    for limit in [1, 200] {
        assert!(valid(&search, &json!({"text":"x","limit":limit})));
    }
    assert!(!valid(&search, &json!({"text":"x","unexpected":true})));
    let trace = schema::<TraceQuery>(Contract::Deserialize);
    assert_eq!(trace["properties"]["max_depth"]["default"], 3);
    for depth in [0, 11] {
        assert!(!valid(&trace, &json!({"id":"r","max_depth":depth})));
    }
}
#[test]
fn evidence_output_requires_nullable_stale_and_emitted_cut_flags() {
    let input = json!({"rule_id":"r", "limit":50,"has_more":false,"implementation_bindings":[],
        "verification_bindings":[],"verification_runs":[],"review_required":false,"reviews":[]});
    let value: EvidenceResult = serde_json::from_value(input.clone()).unwrap();
    let output = serde_json::to_value(value).unwrap();
    assert_eq!(output["stale"], Value::Null);
    assert!(valid(
        &schema::<EvidenceResult>(Contract::Deserialize),
        &input
    ));
    let emitted = schema::<EvidenceResult>(Contract::Serialize);
    assert!(valid(&emitted, &output));
    for field in [
        "stale",
        "implementation_bindings_has_more",
        "verification_bindings_has_more",
        "verification_runs_has_more",
        "reviews_has_more",
    ] {
        let mut missing = output.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(!valid(&emitted, &missing), "missing {field} accepted");
    }
}
#[test]
fn typed_spec_uses_real_input_and_output_omissions() {
    let input = json!({"schema_version":1,"spec":"x","declared_by":"test","rules":[
        {"key":"a","statement":"The system works.","implementation":{"file":"src/a.rs","symbol":"a"}}
    ]});
    let decoded: TypedSpecInput = serde_json::from_value(input.clone()).unwrap();
    assert!(valid(
        &schema::<TypedSpecInput>(Contract::Deserialize),
        &input
    ));
    assert!(valid(
        &schema::<TypedSpecInput>(Contract::Serialize),
        &serde_json::to_value(decoded).unwrap()
    ));
    let mut bad = input;
    bad["extra"] = json!(true);
    assert!(!valid(
        &schema::<TypedSpecInput>(Contract::Deserialize),
        &bad
    ));
}
#[test]
fn smart_identifiers_describe_their_runtime_constraints() {
    use provenance_core::{DeclarationAddress, ScopeId, StableId};
    for contract in [Contract::Serialize, Contract::Deserialize] {
        for s in [
            schema::<StableId>(contract.clone()),
            schema::<ScopeId>(contract.clone()),
        ] {
            assert!(valid(&s, &json!("rule_1")));
            for bad in [json!(""), json!("A"), json!("a/b")] {
                assert!(!valid(&s, &bad), "accepted {bad}");
            }
        }
        let address = schema::<DeclarationAddress>(contract);
        assert!(valid(&address, &json!(["requirement", "rule"])));
        assert!(!valid(&address, &json!([])));
        assert!(!valid(&address, &json!(["  "])));
    }
}
#[test]
fn flattened_response_schema_matches_serialized_envelope() {
    use provenance_core::protocol::{GetResult, QueryResponse, Stamp, StampPolicy, Stamped};
    let response = QueryResponse::new(
        "get",
        Stamped {
            result: GetResult {
                found: false,
                node: None,
            },
            stamp: Stamp {
                serial: 1,
                digest: "test".into(),
                instance_id: "fixture".into(),
                derivation: 1,
                policy: StampPolicy::AnnotateOnly,
                attested: vec![],
                live: vec![],
            },
            freshness_error: None,
        },
    );
    let value = serde_json::to_value(response).unwrap();
    assert_eq!(value["found"], false);
    assert!(value.get("result").is_none());
    assert!(valid(
        &schema::<QueryResponse<GetResult>>(Contract::Serialize),
        &value
    ));
}
#[test]
fn tagged_graph_node_schema_matches_real_serde_variant() {
    use provenance_core::protocol::GraphNode;
    let value = json!({"node_type":"domain","schema_version":1,"scope_id":"default",
        "id":"domain_a","name":"A"});
    let decoded: GraphNode = serde_json::from_value(value.clone()).unwrap();
    assert!(valid(&schema::<GraphNode>(Contract::Deserialize), &value));
    assert!(valid(
        &schema::<GraphNode>(Contract::Serialize),
        &serde_json::to_value(decoded).unwrap()
    ));
    let mut bad = value;
    bad["node_type"] = json!("invented");
    assert!(!valid(&schema::<GraphNode>(Contract::Deserialize), &bad));
}
