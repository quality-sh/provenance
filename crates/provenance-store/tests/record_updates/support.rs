use provenance_core::{
    protocol::failure::{OperationError, OperationFailure},
    Manifest, RepoPathPrefix, ScopeId,
};
use provenance_store::{layout::ProvenanceLayout, operations::catalog::*, state_store::StateStore};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;

/// Runs each call on the caller's thread through the fixture resolver. Integration tests
/// build the library without `cfg(test)`, so the catalog's test-only host is not available.
#[derive(Clone)]
struct Host(Arc<dyn ContextResolver>);

impl Invoke for Host {
    fn invoke<O: Operation>(
        self,
        selected: Option<RequestedContext>,
        request: O::Request,
    ) -> OperationFuture<O::Success, OperationError<O::Failure>> {
        Box::pin(async move {
            let selected = selected.expect("record updates select a scope");
            invoke_authorized_typed::<O>(self.0, selected, request).await
        })
    }
}

pub fn shard_path(fixture: &Fixture, family: &str, file: &str) -> PathBuf {
    fixture
        .dir
        .path()
        .join(format!(".provenance/state/scopes/default/{family}/{file}"))
}

pub fn raw_rows(fixture: &Fixture, family: &str, file: &str) -> Vec<String> {
    std::fs::read_to_string(shard_path(fixture, family, file))
        .unwrap()
        .lines()
        .map(str::to_owned)
        .collect()
}

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
        let store = provenance_store::repository::Repository::open(dir.path(), "default")
            .unwrap()
            .store();
        Self { dir, store, scope }
    }
    pub async fn call(&self, operation: &str, request: Value) -> Result<Value, Value> {
        struct Resolver(PreparedScope);
        impl ContextResolver for Resolver {
            fn authorize(
                &self,
                _: &'static str,
                _: &RequestedContext,
            ) -> Result<(), OperationFailure> {
                Ok(())
            }

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
            Host(Arc::new(Resolver(PreparedScope {
                repository: provenance_store::repository::Repository::open(
                    self.dir.path(),
                    self.scope.as_str(),
                )
                .unwrap(),
                requested_target: "fixture".into(),
            }))),
        )
        .await
        .map_err(|error| error.error)
    }
    pub async fn source(&self) -> Value {
        self.call("create-source", json!({"scope_id":"default","id":"source_one","name":"Policy","source_type":"policy","url":"https://old.example","reference":"section 1","commit_pin":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","supersedes":[]})).await.unwrap()
    }
    pub async fn requirement(&self) {
        self.call("create-requirement-native", json!({"scope_id":"default","id":"req_one","statement":"The system saves the record.","status":"active","depends_on":[],"supersedes":[]})).await.unwrap();
    }
}
