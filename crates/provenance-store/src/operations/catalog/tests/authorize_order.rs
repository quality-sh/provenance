use super::super::{
    invoke_authorized_typed, ContextResolver, ExecutionNeeds, PreparedContext, RequestedContext,
    ResolveRecord,
};
use provenance_core::{
    protocol::{
        failure::{OperationError, OperationFailure},
        repository::RepositoryContext,
        ResolveRecordQuery,
    },
    NodeType, SDK_PROTOCOL_VERSION,
};
use std::sync::{Arc, Mutex};

/// Records each resolver call in order; the identity check passes and preparation refuses.
struct Recorder(Mutex<Vec<&'static str>>);

impl ContextResolver for Recorder {
    fn authorize(&self, _: &'static str, _: &RequestedContext) -> Result<(), OperationFailure> {
        self.0.lock().unwrap().push("authorize");
        Ok(())
    }

    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        self.0.lock().unwrap().push("prepare");
        Err(OperationFailure::UnknownScope)
    }
}

#[tokio::test]
async fn authorize_runs_before_prepare_and_prepare_still_refuses() {
    let resolver = Arc::new(Recorder(Mutex::new(Vec::new())));
    let error = invoke_authorized_typed::<ResolveRecord>(
        resolver.clone(),
        RequestedContext::Scoped(RepositoryContext {
            repository: "fixture".into(),
            scope: "default".into(),
            freshness: None,
        }),
        ResolveRecordQuery {
            protocol_version: Some(SDK_PROTOCOL_VERSION),
            id: "req_one".into(),
            allowed_node_types: NodeType::ALL.to_vec(),
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(
        error,
        OperationError::Common(OperationFailure::UnknownScope)
    ));
    assert_eq!(*resolver.0.lock().unwrap(), ["authorize", "prepare"]);
}
