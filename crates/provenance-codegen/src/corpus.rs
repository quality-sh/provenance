//! Actual wire schemas used to evaluate client generators.
use provenance_core::{
    model::VerificationRun,
    protocol::{
        CheckStatementRequest, EvidenceResult, GetResult, GraphNode, QueryResponse, SearchQuery,
        Stamp, StampPolicy, Stamped, TypedSpecInput,
    },
};
use schemars::{
    generate::{Contract, SchemaSettings},
    JsonSchema,
};
use serde_json::{json, Map, Value};

fn schema<T: JsonSchema>(contract: Contract) -> Value {
    let mut settings = SchemaSettings::draft2020_12();
    settings.contract = contract;
    serde_json::to_value(settings.into_generator().into_root_schema_for::<T>()).unwrap()
}

pub fn corpus() -> Value {
    let mut schemas = Map::new();
    macro_rules! add {
        ($name:literal, $type:ty, $direction:ident) => {
            crate::component($name, schema::<$type>(Contract::$direction), &mut schemas);
        };
    }
    add!("CheckStatementInput", CheckStatementRequest, Deserialize);
    add!("ReportOutput", provenance_ste100::Report, Serialize);
    add!("SearchInput", SearchQuery, Deserialize);
    for (name, operation, schema) in [
        (
            "GetOutput",
            "get",
            schema::<QueryResponse<GetResult>>(Contract::Serialize),
        ),
        (
            "EvidenceOutput",
            "evidence",
            schema::<QueryResponse<EvidenceResult>>(Contract::Serialize),
        ),
    ] {
        crate::component(
            name,
            provenance_store::operations::catalog::bind_response_identity(schema, operation),
            &mut schemas,
        );
    }
    add!("GraphNodeOutput", GraphNode, Serialize);
    add!(
        "PlanOutput",
        provenance_store::operations::TypedSpecPlan,
        Serialize
    );
    add!("VerificationRunOutput", VerificationRun, Serialize);
    add!("TypedSpecInput", TypedSpecInput, Deserialize);
    json!({"openapi":"3.1.0","info":{"title":"Production wire fixtures","version":"1"},
        "paths":{},"components":{"schemas":schemas},"x-wire-fixtures":wire_fixtures()})
}

fn wire_fixtures() -> Value {
    let stamp = || Stamp {
        serial: 1,
        digest: "test".into(),
        instance_id: "fixture".into(),
        derivation: 1,
        policy: StampPolicy::AnnotateOnly,
        attested: vec![],
        live: vec![],
    };
    let get = QueryResponse::new(
        "get",
        Stamped {
            result: GetResult {
                found: false,
                node: None,
            },
            stamp: stamp(),
            freshness_error: None,
        },
    );
    let evidence: EvidenceResult = serde_json::from_value(json!({"rule_id":"r","limit":50,"has_more":false,
        "implementation_bindings":[],"verification_bindings":[],"verification_runs":[],"review_required":false,"reviews":[]})).unwrap();
    let evidence = QueryResponse::new(
        "evidence",
        Stamped {
            result: evidence,
            stamp: stamp(),
            freshness_error: None,
        },
    );
    let node: GraphNode = serde_json::from_value(
        json!({"node_type":"domain","schema_version":1,"scope_id":"default",
        "id":"domain_a","name":"A"}),
    )
    .unwrap();
    let plan: provenance_store::operations::TypedSpecPlan = serde_json::from_value(json!({"declared_by":"test","created":0,
        "updated":0,"moved":0,"deleted":0,"conflicts":0,"unchanged":0,"resources":[],"affected_rules":[]})).unwrap();
    let run: VerificationRun = serde_json::from_value(json!({"schema_version":1,"scope_id":"default","id":"run_a",
        "rule_id":"rule_a","binding_id":"binding_a","method":"examples","declared_by":"test","status":"running","started_at":1})).unwrap();
    json!({"GetOutput":get,"EvidenceOutput":evidence,"GraphNodeOutput":node,"PlanOutput":plan,
        "VerificationRunOutput":run,"ReportOutput":provenance_ste100::check_descriptive("Café; stop.")})
}
