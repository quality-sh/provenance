use provenance_macros::verifies;
use provenance_porcelain::get::{
    GetInput, GetPort, Impact, PortFuture, ReadError, RecordResolution, Traversal,
    TraversalRequest, View, ViewResult,
};
use provenance_porcelain::Porcelain;
use provenance_core::protocol::{GraphNode, ImpactResult, ResponseMeta, TracedNode};
use provenance_core::protocol::RecordResolution as CoreRecordResolution;
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct FixturePort {
    record: GraphNode,
    traversed: Vec<TracedNode>,
    request: Arc<Mutex<Option<TraversalRequest>>>,
    impact: ImpactResult,
    has_more: bool,
    continuation: Option<String>,
    truncated: bool,
    resolves: Arc<Mutex<usize>>,
    impact_record: Arc<Mutex<Option<GraphNode>>>,
}

impl GetPort for FixturePort {
    fn resolve<'a>(&'a self, _: &'a str) -> PortFuture<'a, RecordResolution> {
        *self.resolves.lock().unwrap() += 1;
        Box::pin(async {
            Ok(RecordResolution {
                result: CoreRecordResolution::Found(self.record.clone()),
                metadata: Some(ResponseMeta {
                    freshness_error: Some("record catch-up failed".into()),
                    ..ResponseMeta::default()
                }),
            })
        })
    }

    fn traverse(&self, request: TraversalRequest) -> PortFuture<'_, Traversal> {
        *self.request.lock().unwrap() = Some(request.clone());
        Box::pin(async move {
            Ok(Traversal {
                records: self.traversed.clone(),
                bounds: provenance_porcelain::get::Bounds {
                    limit: request.limit,
                    max_depth: Some(request.max_depth),
                    has_more: self.has_more,
                    continuation: self.continuation.clone(),
                    truncated: self.truncated,
                },
                response_metadata: Some(ResponseMeta::default()),
            })
        })
    }

    fn impact<'a>(&'a self, record: &'a GraphNode, limit: usize) -> PortFuture<'a, Impact> {
        *self.impact_record.lock().unwrap() = Some(record.clone());
        Box::pin(async move {
            Ok(Impact {
                detail: self.impact.clone(),
                bounds: provenance_porcelain::get::Bounds {
                    limit,
                    max_depth: None,
                    has_more: self.has_more,
                    continuation: self.continuation.clone(),
                    truncated: self.truncated,
                },
                response_metadata: Some(ResponseMeta {
                    freshness_error: Some("catch-up failed".into()),
                    ..ResponseMeta::default()
                }),
            })
        })
    }
}

fn service() -> Porcelain<FixturePort> {
    Porcelain::new(FixturePort {
        record: node("req_alpha", "requirement"),
        traversed: Vec::new(),
        request: Arc::new(Mutex::new(None)),
        impact: impact("req_alpha", 50),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    })
}

fn node(id: &str, kind: &str) -> GraphNode {
    let value = match kind {
        "requirement" => json!({
            "node_type": "requirement", "schema_version": 2, "scope_id": "default",
            "id": id, "statement": "Keep the interface clear.", "status": "active"
        }),
        "resolution" => json!({
            "node_type": "resolution", "schema_version": 2, "scope_id": "default",
            "id": id, "title": "Decision", "position": "Use the typed graph.",
            "rationale": "One graph contract is enough.", "status": "draft",
            "inputs": [], "requirement_ids": [], "review_on": null
        }),
        "rule" => json!({
            "node_type": "rule", "schema_version": 2, "scope_id": "default",
            "id": id, "statement": "Keep the typed graph.", "status": "active",
            "severity": "high", "requirement_ids": []
        }),
        _ => panic!("unsupported fixture kind"),
    };
    serde_json::from_value(value).unwrap()
}

fn traced(id: &str, kind: &str, depth: usize) -> TracedNode {
    TracedNode {
        depth,
        node: node(id, kind),
    }
}

fn impact(id: &str, limit: usize) -> ImpactResult {
    ImpactResult {
        id: id.into(),
        limit,
        has_more: false,
        affected_rules: Vec::new(),
        scan_cut: false,
    }
}

#[tokio::test]
#[verifies("rule_porcelain_get_selects_child_context", examples)]
async fn children_select_depth_and_returned_kinds() {
    let request = Arc::new(Mutex::new(None));
    let porcelain = Porcelain::new(FixturePort {
        record: node("req_root", "requirement"),
        traversed: vec![traced("req_child", "requirement", 1), traced("rule_leaf", "rule", 2)],
        request: request.clone(),
        impact: impact("req_root", 50),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    });
    let mut input = GetInput::new("req_root", View::Children);
    input.max_depth = Some(2);
    input.returned_kinds = vec![provenance_core::NodeType::Rule];

    let outcome = porcelain.get(input).await.unwrap();

    let ViewResult::Children(traversal) = outcome.result else { panic!("children view") };
    assert_eq!(traversal.records.len(), 1);
    assert_eq!(traversal.records[0].node.id().as_str(), "rule_leaf");
    let sent = request.lock().unwrap().clone().unwrap();
    assert_eq!(sent.max_depth, 2);
}

#[tokio::test]
#[verifies("rule_porcelain_return_filter_keeps_traversal", examples)]
async fn returned_kind_filter_does_not_limit_intermediate_traversal() {
    let request = Arc::new(Mutex::new(None));
    let porcelain = Porcelain::new(FixturePort {
        record: node("req_root", "requirement"),
        traversed: vec![traced("res_middle", "resolution", 1), traced("rule_leaf", "rule", 2)],
        request: request.clone(),
        impact: impact("req_root", 50),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    });
    let mut input = GetInput::new("req_root", View::Children);
    input.max_depth = Some(2);
    input.returned_kinds = vec![provenance_core::NodeType::Rule];

    let outcome = porcelain.get(input).await.unwrap();

    let ViewResult::Children(traversal) = outcome.result else { panic!("children view") };
    assert_eq!(traversal.records[0].node.id().as_str(), "rule_leaf");
    assert_eq!(request.lock().unwrap().as_ref().unwrap().max_depth, 2);
}

#[tokio::test]
#[verifies("rule_porcelain_get_has_grounding_impact", examples)]
async fn grounding_and_impact_are_identified_named_views() {
    let grounding = Porcelain::new(FixturePort {
        record: node("rule_alpha", "rule"),
        traversed: vec![traced("req_alpha", "requirement", 1)],
        request: Arc::new(Mutex::new(None)),
        impact: impact("rule_alpha", 50),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    })
    .get(GetInput::new("rule_alpha", View::Grounding))
    .await
    .unwrap();
    let impact = Porcelain::new(FixturePort {
        record: node("req_alpha", "requirement"),
        traversed: Vec::new(),
        request: Arc::new(Mutex::new(None)),
        impact: impact("req_alpha", 50),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    })
    .get(GetInput::new("req_alpha", View::Impact))
    .await
    .unwrap();

    let ViewResult::Grounding(traversal) = grounding.result else { panic!("grounding view") };
    assert_eq!(traversal.records[0].node.id().as_str(), "req_alpha");
    let ViewResult::Impact(impact) = impact.result else { panic!("impact view") };
    assert_eq!(impact.detail.id, "req_alpha");
    assert_eq!(impact.response_metadata.unwrap().freshness_error.as_deref(), Some("catch-up failed"));
}

#[tokio::test]
#[verifies("rule_porcelain_read_rejects_bad_options", examples)]
async fn read_rejects_options_that_its_view_does_not_support() {
    let mut bare = GetInput::new("req_alpha", View::Record);
    bare.max_depth = Some(2);
    let mut impact = GetInput::new("req_alpha", View::Impact);
    impact.returned_kinds = vec![provenance_core::NodeType::Rule];

    assert!(matches!(
        service().get(bare).await,
        Err(ReadError::InvalidOptions)
    ));
    assert!(matches!(
        service().get(impact).await,
        Err(ReadError::InvalidOptions)
    ));
}

#[tokio::test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
async fn incomplete_read_reports_its_bound_and_continuation() {
    let porcelain = Porcelain::new(FixturePort {
        record: node("req_root", "requirement"),
        traversed: Vec::new(),
        request: Arc::new(Mutex::new(None)),
        impact: impact("req_root", 1),
        has_more: true,
        continuation: Some("next-page".into()),
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    });
    let mut input = GetInput::new("req_root", View::Impact);
    input.limit = Some(1);

    let outcome = porcelain.get(input).await.unwrap();
    let ViewResult::Impact(impact) = outcome.result else { panic!("impact view") };
    let bounds = impact.bounds;

    assert_eq!(bounds.limit, 1);
    assert_eq!(bounds.max_depth, None);
    assert!(bounds.has_more);
    assert_eq!(bounds.continuation.as_deref(), Some("next-page"));
}

#[tokio::test]
async fn impact_reuses_the_resolved_identity() {
    let resolves = Arc::new(Mutex::new(0));
    let impact_record = Arc::new(Mutex::new(None));
    let porcelain = Porcelain::new(FixturePort {
        record: node("req_alpha", "requirement"),
        traversed: Vec::new(),
        request: Arc::new(Mutex::new(None)),
        impact: impact("req_alpha", 50),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: resolves.clone(),
        impact_record: impact_record.clone(),
    });

    porcelain
        .get(GetInput::new("req_alpha", View::Impact))
        .await
        .unwrap();

    assert_eq!(*resolves.lock().unwrap(), 1);
    assert_eq!(
        impact_record.lock().unwrap().as_ref().unwrap().node_type(),
        provenance_core::NodeType::Requirement
    );
}

#[tokio::test]
#[verifies("rule_porcelain_id_needs_no_kind_selector", examples)]
async fn get_resolves_a_record_from_its_id_alone() {
    let outcome = service()
        .get(GetInput::new("req_alpha", View::Record))
        .await
        .unwrap();

    assert_eq!(outcome.record.id().as_str(), "req_alpha");
    assert_eq!(outcome.record.node_type(), provenance_core::NodeType::Requirement);
}

#[tokio::test]
#[verifies("rule_porcelain_get_returns_record", examples)]
async fn bare_get_returns_the_selected_record() {
    let outcome = service()
        .get(GetInput::new("req_alpha", View::Record))
        .await
        .unwrap();

    assert!(matches!(outcome.result, ViewResult::Record));
    let encoded = serde_json::to_value(outcome).unwrap();
    assert_eq!(encoded["record"]["value"]["statement"], "Keep the interface clear.");
    assert_eq!(encoded["related"], json!([]));
    assert_eq!(encoded["bounds"], json!(null));
}

#[allow(dead_code)]
fn assert_read_error_is_public(_: ReadError) {}
