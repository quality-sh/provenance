//! The failures of the read operations through a port, and the steps of the
//! router before it.
//!
//! A binding of a repository over memory storage runs its reads through a
//! port. Each failure that the reads declare (a record that does not exist,
//! the refusals of status 409 with their fields, a refused input, an
//! authorization refusal) must equal the answer of the router over the same
//! state in a folder, in the form of `support/memory_host.rs`, and so must
//! the second page of a list. Before the port is asked, the router
//! authorizes the call, waits for a work slot and runs the preparation of
//! its host access, as for an operation that it runs itself.

use crate::support::memory_host::{bound, compared, in_process, memory_repository, state, Counted};
use crate::support::names::Names;
use crate::support::records::{allow_reviewer, Repository};
use crate::support::router_answer::{Answer, Http};
use camino::Utf8PathBuf;
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::{RequirementStatus, ScopeId, StableId};
use provenance_store::cache::{catch_up_in_publish, materialize_state};
use provenance_store::operations::catalog::{
    ContextResolver, ExecutionNeeds, PreparedContext, RequestedContext,
};
use provenance_store::operations::read_operations::{ReadAnswer, ReadOperations};
use provenance_store::operations::LocalRepository;
use provenance_store::repository::Repository as Opened;
use provenance_store::state_store::CreateRequirementInput;
use provenance_store::storage::memory::MemoryStateStorage;
use provenance_store::storage::{Atomicity, StateStorage, WriteSet};
use provenance_transport::HostAccess;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;

/// The step of the host access that refuses one operation.
#[derive(Clone, Copy)]
pub enum Step {
    Open,
    Authorize(&'static str),
    Prepare(&'static str),
}

/// The host access of one binding, with `step` as a refusal.
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
            return Err(OperationFailure::AccessDenied { message: None });
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

/// A host over `binding`, with `step` as a refusal.
pub fn host(binding: LocalRepository, step: Step) -> Http {
    Http::with_access(Arc::new(Gate {
        inner: binding,
        step,
    }))
}

/// A fixture repository in a folder and a repository over memory storage
/// with the same state, each behind a router.
pub struct Pair {
    pub folder: Repository,
    pub memory: Opened,
    storage: Arc<MemoryStateStorage>,
    _missing: tempfile::TempDir,
    names: (Names, Names),
}

impl Pair {
    /// The pair, with `policy` as the read freshness policy of both.
    pub async fn new(policy: Option<&str>) -> Self {
        Self::with(policy, &[]).await
    }

    /// The pair, with `policy` as the read freshness policy of both, and a
    /// Requirement with each id of `requirements` beside the records of
    /// each kind.
    async fn with(policy: Option<&str>, requirements: &[&str]) -> Self {
        let folder = Repository::new("The shared graph is readable.");
        folder.all_kinds();
        allow_reviewer(&folder);
        for id in requirements {
            folder
                .open()
                .store()
                .create_requirement(requirement(id))
                .unwrap();
        }
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
            _missing: missing,
            names: (Names::default(), Names::default()),
        }
    }

    /// Calls `paths.0` on the router over memory, through `port`, and
    /// `paths.1` on the router over the folder, each with `step`. The two
    /// answers must be the same. Gives the answer over memory, then the
    /// answer over the folder.
    async fn both(
        &mut self,
        paths: (&str, &str),
        port: &Arc<Counted>,
        step: Step,
    ) -> (Answer, Answer) {
        let binding = bound(self.memory.clone(), port.clone());
        let over_memory = host(binding, step)
            .call("GET", paths.0, None, &[], format!("GET {}", paths.0))
            .await;
        let folder = LocalRepository::bind(self.folder.open(), "selected").unwrap();
        let over_folder = host(folder, step)
            .call("GET", paths.1, None, &[], format!("GET {}", paths.1))
            .await;
        assert_eq!(
            compared(&mut self.names.0, &over_memory),
            compared(&mut self.names.1, &over_folder),
            "{}",
            over_memory.shown
        );
        (over_memory, over_folder)
    }

    /// [`Pair::both`] with one path. Gives the answer over memory.
    pub async fn same(&mut self, path: &str, port: &Arc<Counted>, step: Step) -> Answer {
        self.both((path, path), port, step).await.0
    }

    /// The in-process port of the memory repository, counted.
    pub fn in_process(&self) -> Arc<Counted> {
        in_process(&self.memory)
    }
}

fn requirement(id: &str) -> CreateRequirementInput {
    CreateRequirementInput {
        scope_id: ScopeId::new("default").unwrap(),
        id: StableId::new(id).unwrap(),
        statement: "The second page is readable.".into(),
        description: None,
        status: RequirementStatus::Active,
        refines: Vec::new(),
        depends_on: Vec::new(),
        supersedes: Vec::new(),
        spawned_by: None,
        origin_thread: None,
        origin_message: None,
    }
}

pub fn kind(answer: &Answer) -> (u16, Value) {
    (answer.status, answer.json()["error"]["kind"].clone())
}

#[tokio::test]
async fn declared_failures_through_the_port_answer_as_over_a_folder() {
    let _alone = crate::one_at_a_time().await;
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

/// The cursor of a page is signed with the key of its read database. The
/// second page through the port, read with the cursor of the first, answers
/// as the second page over the folder.
#[tokio::test]
async fn a_second_page_through_the_port_answers_as_over_a_folder() {
    let _alone = crate::one_at_a_time().await;
    let mut pair = Pair::with(None, &["req_second"]).await;
    let port = pair.in_process();
    let first = "/requirements?limit=1";
    let (memory, folder) = pair.both((first, first), &port, Step::Open).await;
    let next = |answer: &Answer| {
        let cursor = answer.json()["meta"]["next_cursor"].clone();
        format!("{first}&cursor={}", cursor.as_str().unwrap())
    };
    let (memory, folder) = (next(&memory), next(&folder));
    let (second, _) = pair.both((&memory, &folder), &port, Step::Open).await;
    assert_eq!(second.status, 200, "{}", second.json());
    let items = second.json()["data"]["items"].clone();
    assert_eq!(items.as_array().map(Vec::len), Some(1), "{items}");
    assert_eq!(port.runs(), ["page-requirements", "page-requirements"]);
}

#[tokio::test]
async fn refusals_of_a_stale_read_through_the_port_answer_as_over_a_folder() {
    let _alone = crate::one_at_a_time().await;
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
    let _alone = crate::one_at_a_time().await;
    let mut pair = Pair::new(None).await;
    let port = pair.in_process();
    let step = Step::Authorize("page-requirements");
    let refused = pair.same("/requirements", &port, step).await;
    assert_eq!(kind(&refused), (403, json!("access_denied")));
    assert_eq!(port.runs(), Vec::<&str>::new());
}

#[tokio::test]
async fn the_router_prepares_the_repository_before_the_port() {
    let _alone = crate::one_at_a_time().await;
    let mut pair = Pair::new(None).await;
    // This port prepares nothing: it answers each call with an empty page.
    let port = Counted::new(Arc::new(crate::answers::Fixed::Success(
        json!({"items": []}),
    )));
    let step = Step::Prepare("page-requirements");
    let refused = pair.same("/requirements", &port, step).await;
    assert_eq!(kind(&refused), (404, json!("unknown_scope")));
    assert_eq!(port.runs(), Vec::<&str>::new());
}

/// A port whose calls wait until the test opens it, then answer as the
/// port below.
#[derive(Debug)]
struct Waiting {
    below: Arc<Counted>,
    open: Arc<Semaphore>,
}

impl ReadOperations for Waiting {
    fn run(&self, operation: &'static str, request: Value) -> ReadAnswer {
        let open = Arc::clone(&self.open);
        let answer = self.below.run(operation, request);
        Box::pin(async move {
            let _open = open.acquire().await;
            answer.await
        })
    }
}

/// When each work slot of the host holds a call that the port runs, the
/// next call waits for a slot until the request deadline, then answers
/// `host_busy` (503). The port is not asked for it.
#[tokio::test]
async fn a_full_work_slot_queue_answers_host_busy_before_the_port() {
    const SLOTS: usize = 8;
    let _alone = crate::one_at_a_time().await;
    let pair = Pair::new(None).await;
    let counted = pair.in_process();
    let open = Arc::new(Semaphore::new(0));
    let port = Arc::new(Waiting {
        below: counted.clone(),
        open: open.clone(),
    });
    let http = Arc::new(Http::with_access(Arc::new(bound(
        pair.memory.clone(),
        port,
    ))));
    let call = |http: Arc<Http>| async move {
        http.call(
            "GET",
            "/requirements",
            None,
            &[],
            "GET /requirements".into(),
        )
        .await
    };
    let held: Vec<_> = (0..SLOTS)
        .map(|_| tokio::spawn(call(http.clone())))
        .collect();
    tokio::time::timeout(Duration::from_secs(60), async {
        while counted.runs().len() < SLOTS {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("each work slot holds a call of the port");

    let busy = call(http.clone()).await;
    assert_eq!(kind(&busy), (503, json!("host_busy")), "{}", busy.json());
    assert_eq!(counted.runs().len(), SLOTS);
    open.add_permits(SLOTS);
    for held in held {
        assert_eq!(held.await.unwrap().status, 200);
    }
}
