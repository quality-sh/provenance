//! A call that a port answers in place of the host: the dispatch decodes and checks the call as
//! for an operation that the host runs, and checks the answer of the port against the operation.

use crate::dispatch::{decode_call, port_checks};
use provenance_core::protocol::failure::{ErasedFailure as FailureEnvelope, OperationFailure};
use provenance_store::operations::catalog::{ExecutionNeeds, Operation, RequestedContext};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

/// The context that a checked call selects, and the needs of its request.
type Checked = (Option<RequestedContext>, ExecutionNeeds);

/// The checks of one operation for a call that a port answers.
#[derive(Clone, Copy)]
pub(crate) struct Checks {
    name: &'static str,
    check: fn(Value) -> Result<Checked, FailureEnvelope>,
    answer: fn(Result<Value, FailureEnvelope>) -> Result<Value, FailureEnvelope>,
}

impl Checks {
    pub(crate) const fn of<O: Operation>() -> Self {
        Self {
            name: O::NAME,
            check: check::<O>,
            answer: answer::<O>,
        }
    }
}

/// Runs one call of `operation` through a port.
///
/// The call is decoded and its request is checked as for an operation that the host runs, so a
/// call that does not decode, or a request that the operation refuses, gets the same refusal.
/// Then `run` gets the operation name, the selected context, the needs of the request and the
/// call. It authorizes the context, waits for its work slot, prepares the repository and asks
/// the port; its own refusal is a common failure of the operation.
///
/// The answer of the port must be one that the operation declares: a success that its success
/// schema accepts, or a failure that its failure schema accepts, with a status that the
/// operation gives for it. Any other answer becomes the `internal` failure, and the host log
/// keeps what the port gave.
pub async fn invoke_port<R, A>(
    operation: &str,
    call: Value,
    run: R,
) -> Result<Value, FailureEnvelope>
where
    R: FnOnce(&'static str, Option<RequestedContext>, ExecutionNeeds, Value) -> A,
    A: Future<Output = Result<Result<Value, FailureEnvelope>, OperationFailure>>,
{
    let checks = port_checks(operation)
        .ok_or_else(|| FailureEnvelope::new(None, OperationFailure::UnknownOperation))?;
    let (selected, needs) = (checks.check)(call.clone())?;
    let answer = run(checks.name, selected, needs, call)
        .await
        .map_err(|failure| FailureEnvelope::new(Some(checks.name), failure))?;
    (checks.answer)(answer)
}

fn check<O: Operation>(call: Value) -> Result<Checked, FailureEnvelope> {
    let (selected, request) = decode_call::<O>(call)?;
    O::validate_external(&request)
        .map_err(|failure| FailureEnvelope::new(Some(O::NAME), failure))?;
    Ok((selected, O::needs(&request)))
}

/// The answer of the port when the operation declares it, else the `internal` failure.
fn answer<O: Operation>(answer: Result<Value, FailureEnvelope>) -> Result<Value, FailureEnvelope> {
    let schemas = schemas::<O>();
    let given = match &answer {
        Ok(success) if accepts(schemas.success.as_ref(), success) => None,
        Err(failure) if declared(&schemas, failure) => None,
        Ok(success) => Some(format!("a success that its schema refuses: {success}")),
        Err(failure) => Some(format!(
            "a failure that it does not declare, with status {}: {}",
            failure.status_code(),
            failure.error
        )),
    };
    let Some(given) = given else {
        return answer;
    };
    Err(
        FailureEnvelope::new(Some(O::NAME), OperationFailure::Internal).with_cause(Some(format!(
            "the read operations port answered `{}` with {given}",
            O::NAME
        ))),
    )
}

/// Whether the operation gives `failure`: its failure schema accepts the body, and the status is
/// one that the operation gives. A common failure has the status of its kind.
fn declared(schemas: &Schemas, failure: &FailureEnvelope) -> bool {
    let status = failure.status_code();
    let given = serde_json::from_value::<OperationFailure>(failure.error.clone()).map_or_else(
        |_| schemas.statuses.contains(&status),
        |common| common.status_code() == status,
    );
    given
        && serde_json::to_value(failure).is_ok_and(|body| accepts(schemas.failure.as_ref(), &body))
}

fn accepts(validator: Option<&jsonschema::Validator>, value: &Value) -> bool {
    validator.is_some_and(|validator| validator.is_valid(value))
}

/// The published success and failure schemas of one operation, and the statuses it gives.
struct Schemas {
    success: Option<jsonschema::Validator>,
    failure: Option<jsonschema::Validator>,
    statuses: Vec<u16>,
}

fn schemas<O: Operation>() -> Arc<Schemas> {
    static SCHEMAS: OnceLock<Mutex<BTreeMap<&'static str, Arc<Schemas>>>> = OnceLock::new();
    let mut known = SCHEMAS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    Arc::clone(known.entry(O::NAME).or_insert_with(|| {
        let definition = crate::schema::raw_definition::<O>();
        Arc::new(Schemas {
            success: validator(&definition.success_schema),
            failure: validator(&definition.failure_schema),
            statuses: definition.http_statuses,
        })
    }))
}

fn validator(schema: &Value) -> Option<jsonschema::Validator> {
    jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .offline()
        .build(schema)
        .ok()
}
