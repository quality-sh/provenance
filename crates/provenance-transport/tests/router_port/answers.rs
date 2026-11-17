//! The check of each answer that a port gives.
//!
//! The router sends an answer of the port only when the operation gives it:
//! a success that the closed success schema accepts, a read failure, or a
//! common failure that a read can give after the router prepared the
//! repository, each with the one status of its kind. Any other answer
//! becomes the `internal` failure (500).

use crate::failures::{host, kind, Pair, Step};
use crate::support::memory_host::{bound, in_process, Counted};
use crate::support::router_answer::Answer;
use provenance_core::protocol::failure::{ErasedFailure, InvalidInputReason, OperationFailure};
use provenance_core::protocol::read_failure::MATERIALIZE;
use provenance_store::operations::read_operations::{ReadAnswer, ReadOperations, READ_OPERATIONS};
use provenance_store::repository::Repository as Opened;
use serde_json::{json, Value};
use std::sync::Arc;

/// A port that gives one answer to each call: a success, or a failure body
/// with its status.
#[derive(Debug, Clone)]
pub enum Fixed {
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

/// The answer of `GET /requirements` of a router whose binding of `memory`
/// has `answer` as its port. The port must be asked once.
async fn through(memory: &Opened, answer: &Fixed) -> Answer {
    let port = Counted::new(Arc::new(answer.clone()));
    let given = host(bound(memory.clone(), port.clone()), Step::Open)
        .call(
            "GET",
            "/requirements",
            None,
            &[],
            "GET /requirements".into(),
        )
        .await;
    assert_eq!(port.runs(), vec!["page-requirements"], "{answer:?}");
    given
}

/// A success of `page-requirements` that the in-process port gives.
async fn real_success(memory: &Opened) -> Value {
    let call = json!({
        "context": { "repository": "selected", "scope": "default" },
        "request": {},
    });
    in_process(memory)
        .run("page-requirements", call)
        .await
        .unwrap()
}

fn with(mut value: Value, pointer: &str, member: &str) -> Value {
    value
        .pointer_mut(pointer)
        .and_then(Value::as_object_mut)
        .unwrap()
        .insert(member.to_owned(), json!("added by the port"));
    value
}

fn common(failure: &OperationFailure) -> Value {
    serde_json::to_value(failure).unwrap()
}

#[tokio::test]
async fn an_answer_that_the_operation_does_not_give_is_internal() {
    let _alone = crate::one_at_a_time().await;
    let pair = Pair::new(None).await;
    let success = real_success(&pair.memory).await;
    let invalid = common(&OperationFailure::InvalidInput {
        field: None,
        reason: InvalidInputReason::InvalidValue,
    });
    let undeclared = [
        Fixed::Success(json!({"page": "of another operation"})),
        Fixed::Success(with(success.clone(), "", "extra")),
        Fixed::Success(with(success.clone(), "/stamp", "debug")),
        Fixed::Failure(json!({"kind": "stale"}), 409),
        Fixed::Failure(with(no_projection(), "", "extra"), 409),
        Fixed::Failure(no_projection(), 1000),
        Fixed::Failure(no_projection(), 200),
        Fixed::Failure(no_projection(), 503),
        Fixed::Failure(invalid, 500),
    ];
    for answer in undeclared {
        let given = through(&pair.memory, &answer).await;
        assert_eq!(kind(&given), (500, json!("internal")), "{answer:?}");
        assert_eq!(given.json()["error"], json!({"kind": "internal"}));
    }
}

/// The failure body of a read with no stored revision and no cause.
fn no_projection() -> Value {
    json!({"kind": "no_projection", "hint": MATERIALIZE})
}

#[tokio::test]
async fn an_answer_that_the_operation_gives_passes_unchanged() {
    let _alone = crate::one_at_a_time().await;
    let pair = Pair::new(None).await;
    let success = real_success(&pair.memory).await;
    let given = through(&pair.memory, &Fixed::Success(success.clone())).await;
    assert_eq!(given.status, 200, "{}", given.json());
    assert_eq!(given.json()["data"]["items"], success["result"]["items"]);

    let declared = through(&pair.memory, &Fixed::Failure(no_projection(), 409)).await;
    assert_eq!(kind(&declared), (409, json!("no_projection")));
}

/// A failure of a common kind that a read can give after the router prepared the repository
/// passes with the status of its kind, as the host gives it.
#[tokio::test]
async fn a_common_failure_from_the_port_passes_with_its_status() {
    let _alone = crate::one_at_a_time().await;
    let pair = Pair::new(None).await;
    let refusals = [
        OperationFailure::InvalidInput {
            field: Some("request.limit".into()),
            reason: InvalidInputReason::InvalidValue,
        },
        OperationFailure::InvalidInput {
            field: Some("f".repeat(128)),
            reason: InvalidInputReason::InvalidValue,
        },
        OperationFailure::InvalidInput {
            field: None,
            reason: InvalidInputReason::Required,
        },
        OperationFailure::access_denied(),
        OperationFailure::UnknownTarget,
        OperationFailure::UnknownScope,
        OperationFailure::UnavailableNeeds,
        OperationFailure::HostBusy,
        OperationFailure::Internal,
    ];
    for failure in refusals {
        let body = common(&failure);
        let answer = Fixed::Failure(body.clone(), failure.status_code());
        let given = through(&pair.memory, &answer).await;
        assert_eq!(given.status, failure.status_code(), "{answer:?}");
        assert_eq!(given.json()["error"], body, "{answer:?}");
    }
}

/// A common failure that only the router or the transport gives before the port is asked, or
/// one with free text that no read gives, is `internal` (500) from a port, with its own status.
#[tokio::test]
async fn a_common_failure_that_a_read_cannot_give_is_internal() {
    let _alone = crate::one_at_a_time().await;
    let pair = Pair::new(None).await;
    let refusals = [
        OperationFailure::Unauthenticated,
        OperationFailure::MethodNotAllowed,
        OperationFailure::UnknownOperation,
        OperationFailure::ProtocolMismatch {
            requested: 2,
            supported: 1,
        },
        OperationFailure::AccessDenied {
            message: Some("any text of the port".into()),
        },
        OperationFailure::InvalidInput {
            field: Some("f".repeat(129)),
            reason: InvalidInputReason::InvalidValue,
        },
        OperationFailure::InvalidInput {
            field: Some("request limit".into()),
            reason: InvalidInputReason::InvalidValue,
        },
    ];
    for failure in refusals {
        let answer = Fixed::Failure(common(&failure), failure.status_code());
        let given = through(&pair.memory, &answer).await;
        assert_eq!(kind(&given), (500, json!("internal")), "{answer:?}");
        assert_eq!(given.json()["error"], json!({"kind": "internal"}));
    }
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
