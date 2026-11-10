//! The cases of `engine.rs` through the public contract.
//!
//! Each engine sends its operations through `provenance_contract::invoke_with`
//! over the simulated storage, so the dispatch, the request decoding and the
//! failure framing run on every step. The run keeps the laws of every layer
//! and two laws of this layer: O1, no answer is a write failure (500) unless
//! a planted fault met the operation; O2, the times of the messages that
//! landed increase in the order that they landed.

use hegel::TestCase;
use provenance_core::protocol::failure::{ErasedFailure, OperationError, OperationFailure};
use provenance_core::SDK_PROTOCOL_VERSION;
use provenance_simulation::generate::{self, Case, Effect, Operation, Step, Task, SCOPE};
use provenance_simulation::laws::Violation;
use provenance_simulation::storage::Outcome;
use provenance_simulation::{check_case, run_case, Config, Run};
use provenance_store::operations::catalog::{
    invoke_authorized_typed, ContextResolver, ExecutionNeeds, Invoke, Operation as Catalog,
    OperationFuture, PreparedContext, PreparedScope, RequestedContext,
};
use provenance_store::repository::{Repository, RepositoryIdentity};
use provenance_store::storage::memory::{MemorySectionLock, MemoryStateStorage};
use serde_json::{json, Value};
use std::sync::Arc;

const CONTRACT: Config = Config {
    plant_stale_read: false,
    only_law: None,
    apply: Some(apply_contract),
    layer_laws: Some(contract_laws),
};

#[hegel::test]
#[ignore = "random cases run nightly: cargo nextest run --profile simulation --run-ignored all"]
fn random_cases_keep_the_laws_through_the_contract(tc: TestCase) {
    check_case("contract", &Case::draw(&tc), CONTRACT);
}

/// Prepares every call for the one simulated repository of an engine.
struct Resolver(Repository);

impl ContextResolver for Resolver {
    fn authorize(&self, _: &'static str, _: &RequestedContext) -> Result<(), OperationFailure> {
        Ok(())
    }

    fn prepare(
        &self,
        _: &'static str,
        _: RequestedContext,
        _: ExecutionNeeds,
    ) -> Result<PreparedContext, OperationFailure> {
        Ok(PreparedContext::for_scope(PreparedScope {
            repository: self.0.clone(),
            requested_target: "simulated".into(),
        }))
    }
}

/// Runs each call on the engine's thread, as the host's blocking path does.
#[derive(Clone)]
struct Host(Arc<Resolver>);

impl Invoke for Host {
    fn invoke<O: Catalog>(
        self,
        selected: Option<RequestedContext>,
        request: O::Request,
    ) -> OperationFuture<O::Success, OperationError<O::Failure>> {
        Box::pin(async move {
            let selected = selected.ok_or(OperationError::Common(OperationFailure::Internal))?;
            invoke_authorized_typed::<O>(self.0, selected, request).await
        })
    }
}

/// Polls `future` on this thread. The catalog writes do not wait on a
/// runtime, so the first poll gives the answer.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    struct Unpark(std::thread::Thread);
    impl std::task::Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = std::task::Waker::from(Arc::new(Unpark(std::thread::current())));
    let mut context = std::task::Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        if let std::task::Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
        std::thread::park();
    }
}

/// The start of the error text of a contract refusal with `status`.
fn refusal_prefix(status: u16) -> String {
    format!("contract answer {status}:")
}

/// Sends one call by name through the contract dispatch.
fn invoke(
    repository: &Repository,
    operation: &str,
    request: &Value,
) -> Result<Value, Box<ErasedFailure>> {
    let call = json!({"context": {"repository": "simulated", "scope": SCOPE}, "request": request});
    let host = Host(Arc::new(Resolver(repository.clone())));
    block_on(provenance_contract::invoke_with(
        operation,
        SDK_PROTOCOL_VERSION,
        call,
        host,
    ))
    .map_err(Box::new)
}

/// Runs `task` through the contract; an operation with no contract call runs
/// on the store, as the engine layer does.
fn apply_contract(repository: &Repository, task: &Task) -> anyhow::Result<Effect> {
    let Some(call) = task.call(repository)? else {
        return generate::apply(repository, task);
    };
    match invoke(repository, call.operation, &call.request) {
        Ok(answer) => task.effect(&call, &answer),
        Err(failure) => anyhow::bail!(
            "{} {}",
            refusal_prefix(failure.status_code()),
            serde_json::to_string(&failure)?
        ),
    }
}

const fn broken(law: &'static str, detail: String) -> Result<(), Violation> {
    Err(Violation { law, detail })
}

/// O1 and O2.
fn contract_laws(run: &Run) -> Result<(), Violation> {
    no_write_failure_without_a_fault(run)?;
    message_times_increase(run)
}

/// O1: an answer of 500 comes only from an operation that a refused or
/// unknown publish met, or that stopped.
fn no_write_failure_without_a_fault(run: &Run) -> Result<(), Violation> {
    let prefix = refusal_prefix(500);
    for done in &run.done {
        let Err(text) = &done.result else { continue };
        if !text.starts_with(&prefix) || done.stopped {
            continue;
        }
        let faulted = run.publishes.iter().any(|publish| {
            publish.operation == done.operation
                && publish.engine == done.engine
                && matches!(publish.outcome, Outcome::Refused | Outcome::Unknown)
        });
        if !faulted {
            return broken(
                "O1",
                format!(
                    "operation {} ({:?}) answered a write failure without a fault: {text}",
                    done.operation, done.op
                ),
            );
        }
    }
    Ok(())
}

/// O2: the messages of the successful posts, in the order of their last
/// publish, have increasing times.
fn message_times_increase(run: &Run) -> Result<(), Violation> {
    let repository = Repository::with_storage(
        RepositoryIdentity::new(
            camino::Utf8PathBuf::from("/simulated"),
            provenance_core::ScopeId::new(SCOPE).expect("a scope"),
            provenance_store::settings::Settings::default(),
        ),
        Arc::new(MemoryStateStorage::new(run.entries.clone())),
        Arc::new(MemorySectionLock::default()),
    )
    .map_err(|error| Violation {
        law: "O2",
        detail: format!("the final state did not open: {error:#}"),
    })?;
    let messages = repository
        .view()
        .and_then(|view| view.list_messages(&provenance_core::ScopeId::new(SCOPE)?))
        .map_err(|error| Violation {
            law: "O2",
            detail: format!("the messages did not read: {error:#}"),
        })?;
    let mut landed = Vec::new();
    for done in &run.done {
        let Ok(Effect::Message { id }) = &done.result else {
            continue;
        };
        let last_publish = run.publishes.iter().rposition(|publish| {
            publish.operation == done.operation
                && publish.engine == done.engine
                && publish.outcome == Outcome::Landed
        });
        let message = messages.iter().find(|message| message.id.as_str() == id);
        if let (Some(position), Some(message)) = (last_publish, message) {
            landed.push((position, done.operation, message.created_at));
        }
    }
    landed.sort_unstable();
    for pair in landed.windows(2) {
        let ((_, first, before), (_, second, after)) = (pair[0], pair[1]);
        if after <= before {
            return broken(
                "O2",
                format!(
                    "the message of operation {second} landed after the one of operation {first} with time {after}, not after {before}"
                ),
            );
        }
    }
    Ok(())
}

/// A repository over the memory storage with one scope and nothing else.
fn empty_repository() -> Repository {
    use provenance_core::{Manifest, RepoPathPrefix};
    use provenance_store::storage::StateKey;
    let scope = provenance_core::ScopeId::new(SCOPE).expect("a scope");
    let manifest = Manifest::default_with_scope(scope.clone(), RepoPathPrefix::new("."));
    let entries = std::collections::BTreeMap::from([(
        StateKey::new("manifest.json").expect("a state key"),
        serde_json::to_vec(&manifest).expect("JSON"),
    )]);
    Repository::with_storage(
        RepositoryIdentity::new(
            camino::Utf8PathBuf::from("/simulated"),
            scope,
            provenance_store::settings::Settings::default(),
        ),
        Arc::new(MemoryStateStorage::new(entries)),
        Arc::new(MemorySectionLock::default()),
    )
    .expect("the repository opens")
}

/// Probe of the plan: a catalog write runs through `invoke_with` on a
/// repository over a storage port.
#[test]
fn a_create_source_runs_through_the_contract_on_a_storage_port() {
    let repository = empty_repository();
    let source = invoke(
        &repository,
        "create-source",
        &json!({
            "scope_id": SCOPE, "id": "source_one", "name": "Policy", "source_type": "policy",
            "url": "https://example.com/policy", "reference": "section 1",
            "commit_pin": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "supersedes": [],
        }),
    )
    .unwrap_or_else(|failure| panic!("{}", serde_json::to_string(&failure).unwrap()));
    assert_eq!(source["id"], "source_one");
    let view = repository.view().unwrap();
    let scope = provenance_core::ScopeId::new(SCOPE).unwrap();
    assert_eq!(view.list_sources(&scope).unwrap().len(), 1);
}

/// Probe of the plan: a stale `update-requirement` through the contract is a
/// conflict (409), and the update before it stays.
#[test]
fn a_stale_update_through_the_contract_is_a_conflict() {
    let update = |engine, stale| Step::Write {
        engine,
        op: Operation::UpdateRequirement { n: 0, stale },
    };
    let case = Case {
        engines: 2,
        steps: vec![update(0, false), update(1, true)],
    };
    let run = run_case(&case, CONTRACT);
    let results: Vec<_> = run.done.iter().map(|done| &done.result).collect();
    assert!(
        matches!(results[0], Ok(Effect::Described { .. })),
        "{results:?}"
    );
    assert!(
        matches!(results[1], Err(text) if text.starts_with(&refusal_prefix(409))),
        "{results:?}"
    );
    provenance_simulation::laws::check(&run).unwrap();
    contract_laws(&run).unwrap();
}

/// Every operation of the generator answers through the contract. Each
/// write gets the turns it needs before the next one starts.
#[test]
fn each_operation_runs_through_the_contract() {
    let ops = [
        Operation::CreateRequirement { n: 0 },
        Operation::PostMessage,
        Operation::SubmitReview,
        Operation::WithdrawReview { n: 0 },
        Operation::ClaimTopic,
        Operation::ReleaseTopic,
        Operation::UpdateRequirement { n: 1, stale: false },
        Operation::BindTwice {
            first: 0,
            second: 1,
        },
    ];
    let steps = ops
        .iter()
        .flat_map(|op| {
            std::iter::once(Step::Write {
                engine: 0,
                op: op.clone(),
            })
            .chain(std::iter::repeat_n(Step::Turn(0), 200))
        })
        .collect();
    let case = Case { engines: 2, steps };
    let run = run_case(&case, CONTRACT);
    let ran: Vec<_> = run.done.iter().map(|done| &done.op).collect();
    assert_eq!(ran, ops.iter().collect::<Vec<_>>());
    for done in &run.done {
        assert!(
            matches!(&done.result, Ok(effect) if *effect != Effect::Skipped),
            "{:?} gave {:?}",
            done.op,
            done.result
        );
    }
    check_case("contract", &case, CONTRACT);
}

/// O1 names an operation that answered a write failure without a fault.
#[test]
fn a_write_failure_without_a_fault_breaks_o1() {
    let mut run = run_case(
        &Case {
            engines: 2,
            steps: Vec::new(),
        },
        CONTRACT,
    );
    run.done.push(provenance_simulation::Done {
        engine: Some(0),
        operation: 1,
        op: Operation::PostMessage,
        target: None,
        result: Err(format!("{} {{}}", refusal_prefix(500))),
        stopped: false,
    });
    assert_eq!(contract_laws(&run).unwrap_err().law, "O1");
}
