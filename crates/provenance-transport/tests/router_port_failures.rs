//! The failures of the read operations through a port.
//!
//! A repository over memory storage runs its reads through a port. Each
//! failure that the reads declare (a record that does not exist, the
//! refusals of status 409 with their fields, a refused input, an
//! authorization refusal) must equal the answer of the router over the same
//! state in a folder, in the form of `support/memory_host.rs`. Before the
//! port is asked, the router runs the preparation of its host access, as
//! for an operation that it runs itself. An answer of the port that the
//! operation does not declare becomes the `internal` failure.
//!
//! Implementation aid: no Rule in the graph covers the port. The plan
//! proposes `rule_contract_router_serves_any_storage` for the owner.
#![cfg(feature = "test-fixture")]

mod support {
    pub mod memory_host;
    pub mod names;
    pub mod records;
    pub mod router_answer;
}

use camino::Utf8PathBuf;
use provenance_core::protocol::failure::{ErasedFailure, OperationFailure};
use provenance_store::cache::{catch_up_in_publish, materialize_state};
use provenance_store::operations::catalog::{
    ContextResolver, ExecutionNeeds, PreparedContext, RequestedContext,
};
use provenance_store::operations::read_operations::{
    InProcessReadOperations, ReadAnswer, ReadOperations, READ_OPERATIONS,
};
use provenance_store::operations::LocalRepository;
use provenance_store::repository::Repository as Opened;
use provenance_store::storage::memory::MemoryStateStorage;
use provenance_store::storage::{Atomicity, StateStorage, WriteSet};
use provenance_transport::HostAccess;
use serde_json::{json, Value};
use std::sync::Arc;
use support::memory_host::{compared, memory_repository, state, Counted};
use support::names::Names;
use support::records::{allow_reviewer, Repository};
use support::router_answer::{Answer, Http};

/// The step of the host access that refuses one operation.
#[derive(Clone, Copy)]
enum Step {
    Open,
    Authorize(&'static str),
    Prepare(&'static str),
}

/// The host access of one bound repository, with `step` as a refusal.
struct Gate {
    inner: LocalRepository,
    step: Step,
}

impl ContextResolver for Gate {
    fn authorize(
        &self,
        operation: &'static str,
        context: &RequestedContext,
    ) -> Result<(), OperationFailure> {
        if matches!(self.step, Step::Authorize(name) if name == operation) {
            return Err(OperationFailure::AccessDenied);
        }
        self.inner.authorize(operation, context)
    }

    fn prepare(
        &self,
        operation: &'static str,
        context: RequestedContext,
        needs: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        if matches!(self.step, Step::Prepare(name) if name == operation) {
            return Err(OperationFailure::UnknownScope);
        }
        self.inner.prepare(operation, context, needs)
    }
}

impl HostAccess for Gate {
    fn advertises(&self, operation: &str) -> bool {
        HostAccess::advertises(&self.inner, operation)
    }

    fn bound_identity(&self) -> Option<(String, String)> {
        HostAccess::bound_identity(&self.inner)
    }

    fn read_operations(&self) -> Option<Arc<dyn ReadOperations>> {
        HostAccess::read_operations(&self.inner)
    }
}

fn host(repository: Opened, step: Step) -> Http {
    let inner = LocalRepository::bind(repository, "selected").unwrap();
    Http::with_access(Arc::new(Gate { inner, step }))
}

/// A port that gives one answer to each call: a success, or a failure body
/// with its status.
#[derive(Debug, Clone)]
enum Fixed {
    Success(Value),
    Failure(Value, u16),
}

impl ReadOperations for Fixed {
    fn run(&self, operation: &'static str, _: Value) -> ReadAnswer {
        let answer = match self.clone() {
            Self::Success(success) => Ok(success),
            Self::Failure(body, status) => Err(ErasedFailure::declared(operation, body, status)),
        };
        Box::pin(async move { answer })
    }
}

/// A fixture repository in a folder and a repository over memory storage
/// with the same state, each behind a router.
struct Pair {
    folder: Repository,
    memory: Opened,
    storage: Arc<MemoryStateStorage>,
    root: Utf8PathBuf,
    _missing: tempfile::TempDir,
    names: (Names, Names),
}

impl Pair {
    /// The pair, with `policy` as the read freshness policy of both.
    async fn new(policy: Option<&str>) -> Self {
        let folder = Repository::new("The shared graph is readable.");
        folder.all_kinds();
        allow_reviewer(&folder);
        if let Some(policy) = policy {
            let path = folder.layout.provenance_dir().join("settings.json");
            let mut settings: Value = std::fs::read(&path).map_or_else(
                |_| json!({}),
                |bytes| serde_json::from_slice(&bytes).unwrap(),
            );
            settings["read"] = json!({ "freshness_policy": policy });
            std::fs::write(&path, settings.to_string()).unwrap();
        }
        let missing = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::from_path_buf(missing.path().join("project")).unwrap();
        let (memory, storage) = memory_repository(&folder, &root).await;
        Self {
            folder,
            memory,
            storage,
            root,
            _missing: missing,
            names: (Names::default(), Names::default()),
        }
    }

    /// Calls `path` on the router over memory, through `port`, and on the
    /// router over the folder, each with `step`. The two answers must be
    /// the same. Gives the answer over memory.
    async fn same(&mut self, path: &str, port: &Arc<Counted>, step: Step) -> Answer {
        let memory = self.memory.with_read_operations(port.clone());
        let shown = format!("GET {path}");
        let over_memory = host(memory, step)
            .call("GET", path, None, &[], shown.clone())
            .await;
        let over_folder = host(self.folder.open(), step)
            .call("GET", path, None, &[], shown)
            .await;
        let folder_root = self.folder.dir.path().to_str().unwrap().to_owned();
        assert_eq!(
            compared(&mut self.names.0, self.root.as_str(), &over_memory),
            compared(&mut self.names.1, &folder_root, &over_folder),
            "{}",
            over_memory.shown
        );
        over_memory
    }

    /// The in-process port of the memory repository, counted.
    fn in_process(&self) -> Arc<Counted> {
        let reads = InProcessReadOperations::new(self.memory.clone(), "selected").unwrap();
        Counted::new(Arc::new(reads))
    }
}

fn kind(answer: &Answer) -> (u16, Value) {
    (answer.status, answer.json()["error"]["kind"].clone())
}

#[tokio::test]
async fn declared_failures_through_the_port_answer_as_over_a_folder() {
    let mut pair = Pair::new(None).await;
    let port = pair.in_process();
    let missing = pair
        .same("/requirements/req_missing", &port, Step::Open)
        .await;
    assert_eq!(kind(&missing), (404, json!("resource_not_found")));
    let cursor = pair
        .same("/requirements?cursor=bogus", &port, Step::Open)
        .await;
    assert_eq!(kind(&cursor), (409, json!("cursor_invalid")));
    let limit = pair
        .same(
            "/requirements/req_shared/document?limit=0",
            &port,
            Step::Open,
        )
        .await;
    assert_eq!(kind(&limit), (400, json!("invalid_input")));
    let runs = port.runs();
    assert!(runs.contains(&"get-reviewed-resource"), "{runs:?}");
    assert!(runs.contains(&"page-requirements"), "{runs:?}");
}

#[tokio::test]
async fn refusals_of_a_stale_read_through_the_port_answer_as_over_a_folder() {
    let mut pair = Pair::new(Some("refuse_stale")).await;
    let port = pair.in_process();
    materialize_state(&pair.folder.open()).await.unwrap();
    catch_up_in_publish(&pair.memory).await.unwrap();
    let fresh = pair.same("/requirements", &port, Step::Open).await;
    assert_eq!(fresh.status, 200, "{}", fresh.json());

    let (key, mut bytes) = state(&pair.folder)
        .into_iter()
        .find(|(key, _)| key.as_str().starts_with("scopes/default/"))
        .unwrap();
    bytes.extend_from_slice(b"\n");
    std::fs::write(pair.folder.layout.state_dir().join(key.as_str()), &bytes).unwrap();
    pair.storage
        .publish(WriteSet::new(Atomicity::PerKey).put(key, bytes))
        .unwrap();
    let stale = pair.same("/requirements", &port, Step::Open).await;
    assert_eq!(kind(&stale), (409, json!("stale")));
    assert_eq!(port.runs().len(), 2);
}

#[tokio::test]
async fn an_authorization_refusal_answers_before_the_port() {
    let mut pair = Pair::new(None).await;
    let port = pair.in_process();
    let step = Step::Authorize("page-requirements");
    let refused = pair.same("/requirements", &port, step).await;
    assert_eq!(kind(&refused), (403, json!("access_denied")));
    assert_eq!(port.runs(), Vec::<&str>::new());
}

#[tokio::test]
async fn the_router_prepares_the_repository_before_the_port() {
    let mut pair = Pair::new(None).await;
    // This port prepares nothing: it answers each call with an empty page.
    let port = Counted::new(Arc::new(Fixed::Success(json!({"items": []}))));
    let step = Step::Prepare("page-requirements");
    let refused = pair.same("/requirements", &port, step).await;
    assert_eq!(kind(&refused), (404, json!("unknown_scope")));
    assert_eq!(port.runs(), Vec::<&str>::new());
}

#[tokio::test]
async fn an_answer_that_the_operation_does_not_declare_is_internal() {
    let pair = Pair::new(None).await;
    let undeclared = [
        Fixed::Success(json!({"page": "of another operation"})),
        Fixed::Failure(json!({"kind": "stale"}), 409),
        Fixed::Failure(json!({"kind": "no_projection"}), 1000),
        Fixed::Failure(json!({"kind": "no_projection"}), 200),
    ];
    for answer in undeclared {
        let port = Counted::new(Arc::new(answer.clone()));
        let memory = pair.memory.with_read_operations(port.clone());
        let given = Http::over(memory)
            .call(
                "GET",
                "/requirements",
                None,
                &[],
                "GET /requirements".into(),
            )
            .await;
        assert_eq!(kind(&given), (500, json!("internal")), "{answer:?}");
        assert_eq!(given.json()["error"], json!({"kind": "internal"}));
        assert_eq!(port.runs(), vec!["page-requirements"]);
    }
    let declared = Fixed::Failure(json!({"kind": "no_projection"}), 409);
    let declared = Counted::new(Arc::new(declared));
    let memory = pair.memory.with_read_operations(declared);
    let given = host(memory, Step::Open)
        .call(
            "GET",
            "/requirements",
            None,
            &[],
            "GET /requirements".into(),
        )
        .await;
    assert_eq!(kind(&given), (409, json!("no_projection")));
}

/// Implementation aid: no operation of the port writes, so the router never
/// gives a page write to the port.
#[test]
fn no_read_operation_writes() {
    let writes: Vec<&str> = READ_OPERATIONS
        .iter()
        .copied()
        .filter(|operation| {
            !provenance_contract::contains(operation) || provenance_contract::mutates(operation)
        })
        .collect();
    assert_eq!(writes, Vec::<&str>::new());
}
