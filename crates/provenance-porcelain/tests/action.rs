use provenance_core::protocol::{GraphNode, RecordResolution as CoreResolution};
use provenance_porcelain::action::{Action, ActionError};
use provenance_porcelain::get::{
    GetPort, Impact, PortFuture, RecordResolution, Traversal, TraversalRequest,
};
use provenance_porcelain::Porcelain;
use std::sync::{Arc, Mutex};

struct Port {
    result: CoreResolution,
    resolved: Arc<Mutex<usize>>,
}

impl GetPort for Port {
    fn resolve<'a>(&'a self, _: &'a str) -> PortFuture<'a, RecordResolution> {
        *self.resolved.lock().unwrap() += 1;
        Box::pin(async {
            Ok(RecordResolution {
                result: self.result.clone(),
                metadata: None,
            })
        })
    }

    fn traverse(&self, _: TraversalRequest) -> PortFuture<'_, Traversal> {
        unreachable!("actions do not traverse")
    }

    fn impact<'a>(&'a self, _: &'a GraphNode, _: usize) -> PortFuture<'a, Impact> {
        unreachable!("actions do not calculate impact")
    }
}

fn port(result: CoreResolution) -> (Porcelain<Port>, Arc<Mutex<usize>>) {
    let resolved = Arc::new(Mutex::new(0));
    (
        Porcelain::new(Port {
            result,
            resolved: resolved.clone(),
        }),
        resolved,
    )
}

#[test]
fn discussion_keywords_share_the_target_action_declaration() {
    let words = Action::DISCUSSION.map(Action::as_str);
    assert_eq!(words, ["discussions", "discussion", "discuss", "reply"]);
    for action in Action::DISCUSSION {
        assert_eq!(Action::parse(action.as_str()), Some(action));
        assert!(!Action::RECORD.contains(&action));
    }
}

#[tokio::test]
async fn parent_selection_uses_the_record_resolver() {
    let record: GraphNode = serde_json::from_value(serde_json::json!({
        "node_type":"requirement", "schema_version":2, "scope_id":"default",
        "id":"req_live", "statement":"The target exists.", "status":"active"
    }))
    .unwrap();
    let (porcelain, resolved) = port(CoreResolution::Found(record));
    let parent = porcelain.select_parent("req_live").await.unwrap();
    assert_eq!(parent.node_type, provenance_core::NodeType::Requirement);
    assert_eq!(parent.node_id.as_str(), "req_live");
    assert_eq!(*resolved.lock().unwrap(), 1);
}

#[tokio::test]
async fn create_uses_declared_kind_without_resolving_a_record() {
    let (porcelain, resolved) = port(CoreResolution::Missing);
    let target = porcelain
        .select_target(
            Action::Create,
            "req_new",
            Some(provenance_core::NodeType::Requirement),
        )
        .await
        .unwrap();
    assert_eq!(target.kind, provenance_core::NodeType::Requirement);
    assert_eq!(target.target, "req_new");
    assert_eq!(*resolved.lock().unwrap(), 0);
    assert_eq!(
        porcelain
            .select_target(Action::Create, "req_new", None)
            .await
            .unwrap_err(),
        ActionError::KindSelection
    );
}

#[tokio::test]
async fn existing_action_resolves_kind_and_preserves_missing_identity() {
    let record: GraphNode = serde_json::from_value(serde_json::json!({
        "node_type":"requirement", "schema_version":2, "scope_id":"default",
        "id":"req_live", "statement":"The target exists.", "status":"active"
    }))
    .unwrap();
    let (porcelain, resolved) = port(CoreResolution::Found(record));
    let target = porcelain
        .select_target(Action::Update, "req_live", None)
        .await
        .unwrap();
    assert_eq!(target.kind, provenance_core::NodeType::Requirement);
    assert_eq!(*resolved.lock().unwrap(), 1);
    assert_eq!(
        porcelain
            .select_target(
                Action::Update,
                "req_live",
                Some(provenance_core::NodeType::Rule)
            )
            .await
            .unwrap_err(),
        ActionError::KindSelection
    );

    let (missing, _) = port(CoreResolution::Missing);
    assert_eq!(
        missing
            .select_target(Action::Update, "req_absent", None)
            .await
            .unwrap_err(),
        ActionError::NotFound
    );
    let (ambiguous, _) = port(CoreResolution::Ambiguous);
    assert_eq!(
        ambiguous
            .select_target(Action::Update, "req_duplicate", None)
            .await
            .unwrap_err(),
        ActionError::AmbiguousIdentity
    );
}
