use provenance_macros::verifies;
use provenance_porcelain::get::{
    GetInput, GetPort, Impact, PortFuture, ReadError, Record, Traversal, TraversalRequest, View,
};
use provenance_porcelain::Porcelain;
use serde_json::json;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct FixturePort {
    record: Record,
    traversed: Vec<Record>,
    request: Arc<Mutex<Option<TraversalRequest>>>,
    impact: serde_json::Value,
    has_more: bool,
    continuation: Option<String>,
    truncated: bool,
    resolves: Arc<Mutex<usize>>,
    impact_record: Arc<Mutex<Option<Record>>>,
}

impl GetPort for FixturePort {
    fn resolve<'a>(&'a self, _: &'a str) -> PortFuture<'a, Option<Record>> {
        *self.resolves.lock().unwrap() += 1;
        Box::pin(async { Ok(Some(self.record.clone())) })
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
                response_metadata: Some(json!({"stamp": "view"})),
            })
        })
    }

    fn impact<'a>(&'a self, record: &'a Record, limit: usize) -> PortFuture<'a, Impact> {
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
                response_metadata: Some(json!({
                    "stamp": "impact",
                    "freshness_error": "catch-up failed"
                })),
            })
        })
    }
}

fn service() -> Porcelain<FixturePort> {
    Porcelain::new(FixturePort {
        record: Record::new(
            "req_alpha",
            "requirement",
            json!({"id": "req_alpha", "statement": "Keep the interface clear."}),
        ),
        traversed: Vec::new(),
        request: Arc::new(Mutex::new(None)),
        impact: json!(null),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    })
}

fn record(id: &str, kind: &str) -> Record {
    Record::new(id, kind, json!({"id": id}))
}

#[tokio::test]
#[verifies("rule_porcelain_get_selects_child_context", examples)]
async fn children_select_depth_and_returned_kinds() {
    let request = Arc::new(Mutex::new(None));
    let porcelain = Porcelain::new(FixturePort {
        record: record("req_root", "requirement"),
        traversed: vec![
            record("req_child", "requirement"),
            record("rule_leaf", "rule"),
        ],
        request: request.clone(),
        impact: json!(null),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    });
    let mut input = GetInput::new("req_root", View::Children);
    input.max_depth = Some(2);
    input.returned_kinds = vec!["rule".into()];

    let outcome = porcelain.get(input).await.unwrap();

    assert_eq!(outcome.related, vec![record("rule_leaf", "rule")]);
    let sent = request.lock().unwrap().clone().unwrap();
    assert_eq!(sent.max_depth, 2);
}

#[tokio::test]
#[verifies("rule_porcelain_return_filter_keeps_traversal", examples)]
async fn returned_kind_filter_does_not_limit_intermediate_traversal() {
    let request = Arc::new(Mutex::new(None));
    let porcelain = Porcelain::new(FixturePort {
        record: record("req_root", "requirement"),
        traversed: vec![
            record("res_middle", "resolution"),
            record("rule_leaf", "rule"),
        ],
        request: request.clone(),
        impact: json!(null),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    });
    let mut input = GetInput::new("req_root", View::Children);
    input.max_depth = Some(2);
    input.returned_kinds = vec!["rule".into()];

    let outcome = porcelain.get(input).await.unwrap();

    assert_eq!(outcome.related, vec![record("rule_leaf", "rule")]);
    assert_eq!(request.lock().unwrap().as_ref().unwrap().max_depth, 2);
}

#[tokio::test]
#[verifies("rule_porcelain_get_has_grounding_impact", examples)]
async fn grounding_and_impact_are_identified_named_views() {
    let grounding = Porcelain::new(FixturePort {
        record: record("rule_alpha", "rule"),
        traversed: vec![record("req_alpha", "requirement")],
        request: Arc::new(Mutex::new(None)),
        impact: json!(null),
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
        record: record("req_alpha", "requirement")
            .with_response_metadata(json!({"stamp": "record"})),
        traversed: Vec::new(),
        request: Arc::new(Mutex::new(None)),
        impact: json!({"affected_rules": ["rule_alpha"]}),
        has_more: false,
        continuation: None,
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    })
    .get(GetInput::new("req_alpha", View::Impact))
    .await
    .unwrap();

    assert_eq!(grounding.view, View::Grounding);
    assert_eq!(grounding.related, vec![record("req_alpha", "requirement")]);
    assert_eq!(impact.view, View::Impact);
    assert_eq!(impact.detail.unwrap()["affected_rules"][0], "rule_alpha");
    assert_eq!(impact.record_metadata.unwrap()["stamp"], "record");
    assert_eq!(impact.view_metadata.as_ref().unwrap()["stamp"], "impact");
    assert_eq!(
        impact.view_metadata.unwrap()["freshness_error"],
        "catch-up failed"
    );
}

#[tokio::test]
#[verifies("rule_porcelain_return_filter_keeps_traversal", examples)]
async fn read_rejects_a_returned_kind_outside_the_closed_kind_set() {
    let mut input = GetInput::new("req_root", View::Children);
    input.returned_kinds = vec!["requirement".into(), "dinosaur".into()];

    assert_eq!(service().get(input).await, Err(ReadError::InvalidOptions));
}

#[tokio::test]
#[verifies("rule_porcelain_read_rejects_bad_options", examples)]
async fn read_rejects_options_that_its_view_does_not_support() {
    let mut bare = GetInput::new("req_alpha", View::Record);
    bare.max_depth = Some(2);
    let mut impact = GetInput::new("req_alpha", View::Impact);
    impact.returned_kinds = vec!["rule".into()];

    assert_eq!(service().get(bare).await, Err(ReadError::InvalidOptions));
    assert_eq!(service().get(impact).await, Err(ReadError::InvalidOptions));
}

#[tokio::test]
#[verifies("rule_porcelain_output_reports_bounds", examples)]
async fn incomplete_read_reports_its_bound_and_continuation() {
    let porcelain = Porcelain::new(FixturePort {
        record: record("req_root", "requirement"),
        traversed: Vec::new(),
        request: Arc::new(Mutex::new(None)),
        impact: json!({"affected_rules": ["rule_child"]}),
        has_more: true,
        continuation: Some("next-page".into()),
        truncated: false,
        resolves: Arc::new(Mutex::new(0)),
        impact_record: Arc::new(Mutex::new(None)),
    });
    let mut input = GetInput::new("req_root", View::Impact);
    input.limit = Some(1);

    let bounds = porcelain.get(input).await.unwrap().bounds.unwrap();

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
        record: record("req_alpha", "requirement"),
        traversed: Vec::new(),
        request: Arc::new(Mutex::new(None)),
        impact: json!({"affected_rules": []}),
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
        impact_record.lock().unwrap().as_ref().unwrap().kind,
        "requirement"
    );
}

#[tokio::test]
#[verifies("rule_porcelain_id_needs_no_kind_selector", examples)]
async fn get_resolves_a_record_from_its_id_alone() {
    let outcome = service()
        .get(GetInput::new("req_alpha", View::Record))
        .await
        .unwrap();

    assert_eq!(outcome.record.id, "req_alpha");
    assert_eq!(outcome.record.kind, "requirement");
}

#[tokio::test]
#[verifies("rule_porcelain_get_returns_record", examples)]
async fn bare_get_returns_the_selected_record() {
    let outcome = service()
        .get(GetInput::new("req_alpha", View::Record))
        .await
        .unwrap();

    assert_eq!(outcome.view, View::Record);
    assert_eq!(
        outcome.record.value["statement"],
        "Keep the interface clear."
    );
    assert!(outcome.related.is_empty());
    assert_eq!(outcome.bounds, None);
}

#[allow(dead_code)]
fn assert_read_error_is_public(_: ReadError) {}
