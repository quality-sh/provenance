use provenance_core::{protocol::failure::OperationFailure, Manifest, RepoPathPrefix, ScopeId};
use provenance_store::{layout::ProvenanceLayout, operations::catalog::*, state_store::StateStore};
use serde_json::{json, Value};
use std::sync::Arc;

pub struct Fixture {
    pub dir: tempfile::TempDir,
    pub store: StateStore,
    pub scope: ScopeId,
}
impl Fixture {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::from_path_buf(dir.path().into()).unwrap();
        let layout = ProvenanceLayout::new(root);
        let scope = ScopeId::new("default").unwrap();
        std::fs::create_dir_all(layout.manifest_path().parent().unwrap()).unwrap();
        std::fs::write(
            layout.manifest_path(),
            serde_json::to_vec(&Manifest::default_with_scope(
                scope.clone(),
                RepoPathPrefix::new("."),
            ))
            .unwrap(),
        )
        .unwrap();
        Self {
            dir,
            store: StateStore::new(layout),
            scope,
        }
    }
    pub async fn call(&self, operation: &str, request: Value) -> Result<Value, Value> {
        struct Resolver(PreparedScope);
        impl ContextResolver for Resolver {
            fn prepare(
                &self,
                _: &'static str,
                _: RequestedContext,
                _: ExecutionNeeds,
            ) -> Result<PreparedContext, OperationFailure> {
                Ok(PreparedContext::for_scope(self.0.clone()))
            }
        }
        invoke_with(
            operation,
            provenance_core::SDK_PROTOCOL_VERSION,
            json!({"context":{"repository":"fixture","scope":"default"},"request":request}),
            Arc::new(Resolver(PreparedScope {
                root: camino::Utf8PathBuf::from_path_buf(self.dir.path().into()).unwrap(),
                scope: self.scope.clone(),
                requested_target: "fixture".into(),
            })),
        )
        .await
        .map_err(|error| error.error)
    }
    pub async fn source(&self) -> Value {
        self.call("create-source", json!({"scope_id":"default","id":"source_one","name":"Policy","source_type":"policy","url":"https://old.example","reference":"section 1","commit_pin":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","supersedes":[]})).await.unwrap()
    }
    pub async fn requirement(&self) {
        self.call("create-requirement", json!({"scope_id":"default","id":"req_one","statement":"The system saves the record.","status":"active","depends_on":[],"supersedes":[]})).await.unwrap();
    }
}
