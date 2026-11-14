//! The check of each answer that a port gives.
//!
//! The router sends an answer of the port only when the operation gives it:
//! a success that the closed success schema accepts, or a common failure or
//! a read failure with the one status of its kind. Any other answer becomes
//! the `internal` failure (500).

use crate::failures::{host, kind, Pair, Step};
use crate::support::memory_host::{bound, in_process, Counted};
use crate::support::router_answer::Answer;
use provenance_core::protocol::failure::{ErasedFailure, InvalidInputReason, OperationFailure};
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
        Fixed::Failure(json!({"kind": "no_projection", "extra": 1}), 409),
        Fixed::Failure(json!({"kind": "no_projection"}), 1000),
        Fixed::Failure(json!({"kind": "no_projection"}), 200),
        Fixed::Failure(json!({"kind": "no_projection"}), 503),
        Fixed::Failure(invalid, 500),
    ];
    for answer in undeclared {
        let given = through(&pair.memory, &answer).await;
        assert_eq!(kind(&given), (500, json!("internal")), "{answer:?}");
        assert_eq!(given.json()["error"], json!({"kind": "internal"}));
    }
}

#[tokio::test]
async fn an_answer_that_the_operation_gives_passes_unchanged() {
    let _alone = crate::one_at_a_time().await;
    let pair = Pair::new(None).await;
    let success = real_success(&pair.memory).await;
    let given = through(&pair.memory, &Fixed::Success(success.clone())).await;
    assert_eq!(given.status, 200, "{}", given.json());
    assert_eq!(given.json()["data"]["items"], success["result"]["items"]);

    let declared = through(
        &pair.memory,
        &Fixed::Failure(json!({"kind": "no_projection"}), 409),
    )
    .await;
    assert_eq!(kind(&declared), (409, json!("no_projection")));
}

/// A failure of a common kind that a port gives passes with the status of
/// its kind, as the host gives it.
#[tokio::test]
async fn a_common_failure_from_the_port_passes_with_its_status() {
    let _alone = crate::one_at_a_time().await;
    let pair = Pair::new(None).await;
    let refusals = [
        OperationFailure::InvalidInput {
            field: Some("request.limit".into()),
            reason: InvalidInputReason::InvalidValue,
        },
        OperationFailure::AccessDenied,
        OperationFailure::HostBusy,
    ];
    for failure in refusals {
        let body = common(&failure);
        let answer = Fixed::Failure(body.clone(), failure.status_code());
        let given = through(&pair.memory, &answer).await;
        assert_eq!(given.status, failure.status_code(), "{answer:?}");
        assert_eq!(given.json()["error"], body, "{answer:?}");
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
