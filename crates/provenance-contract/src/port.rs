//! A call that a port answers in place of the host: the dispatch decodes and checks the call as
//! for an operation that the host runs, and checks the answer of the port against the operation.

use crate::dispatch::decode_call;
use provenance_core::protocol::failure::{ErasedFailure as FailureEnvelope, OperationFailure};
use provenance_core::protocol::read_failure::ReadFailure;
use provenance_core::protocol::ResponseMeta;
use provenance_store::operations::catalog::{
    ExecutionNeeds, GetReviewedResource, Operation, ReadDocument, RequestedContext,
    ReviewDiscussionMessages, ReviewDiscussions,
};
use provenance_store::operations::family_operations::{
    PageQuestions, PageRequirements, PageResolutions, PageRules, PageSources, PageTopics,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use std::future::Future;
use std::sync::LazyLock;

/// The context that a checked call selects, and the needs of its request.
type Checked = (Option<RequestedContext>, ExecutionNeeds);

/// The checks of one read operation for a call that a port answers.
struct Checks {
    name: &'static str,
    check: fn(Value) -> Result<Checked, Box<FailureEnvelope>>,
    /// The success schema of the operation, closed: an object of the answer has no member that
    /// the schema does not name.
    success: jsonschema::Validator,
    /// Each status that the operation gives.
    statuses: Vec<u16>,
}

impl Checks {
    fn of<O: Operation>() -> Self {
        let definition = crate::schema::raw_definition::<O>();
        let mut success = definition.success_schema;
        close(&mut success);
        Self {
            name: O::NAME,
            check: check::<O>,
            success: jsonschema::options()
                .with_draft(jsonschema::Draft::Draft202012)
                .build(&success)
                .expect("the closed success schema of a read operation compiles"),
            statuses: definition.http_statuses,
        }
    }
}

/// The checks of each operation in `READ_OPERATIONS`, in its order. They are made once, at the
/// first call through a port.
static CHECKS: LazyLock<[Checks; 10]> = LazyLock::new(|| {
    [
        Checks::of::<PageSources>(),
        Checks::of::<PageRequirements>(),
        Checks::of::<PageResolutions>(),
        Checks::of::<PageRules>(),
        Checks::of::<PageTopics>(),
        Checks::of::<PageQuestions>(),
        Checks::of::<GetReviewedResource>(),
        Checks::of::<ReadDocument>(),
        Checks::of::<ReviewDiscussions>(),
        Checks::of::<ReviewDiscussionMessages>(),
    ]
});

/// Runs one call of `operation`, one of `READ_OPERATIONS`, through a port.
///
/// The call is decoded and its request is checked as for an operation that the host runs, so a
/// call that does not decode, or a request that the operation refuses, gets the same refusal.
/// Then `run` gets the operation name, the selected context, the needs of the request and the
/// call. It authorizes the context, waits for its work slot, prepares the repository and asks
/// the port; its own refusal is a common failure of the operation.
///
/// The answer of the port must be one that the operation gives:
/// - a success that the success schema of the operation accepts, where no object has a member
///   that the schema does not name;
/// - or a common failure or a read failure, with no member that the failure does not name, no
///   response meta, and the one status of its kind, which the operation gives.
///
/// Any other answer becomes the `internal` failure, and the host log keeps what the port gave.
/// The check of a success walks the answer once for each schema node that applies to a value,
/// so its cost grows with the size of the answer and not with the number of operations.
pub async fn invoke_port<R, A>(
    operation: &str,
    call: Value,
    run: R,
) -> Result<Value, Box<FailureEnvelope>>
where
    R: FnOnce(&'static str, Option<RequestedContext>, ExecutionNeeds, Value) -> A,
    A: Future<Output = Result<Result<Value, FailureEnvelope>, OperationFailure>>,
{
    let checks = CHECKS
        .iter()
        .find(|checks| checks.name == operation)
        .ok_or_else(|| {
            Box::new(FailureEnvelope::new(
                None,
                OperationFailure::UnknownOperation,
            ))
        })?;
    let (selected, needs) = (checks.check)(call.clone())?;
    let answer = run(checks.name, selected, needs, call)
        .await
        .map_err(|failure| Box::new(FailureEnvelope::new(Some(checks.name), failure)))?;
    checked(checks, answer)
}

fn check<O: Operation>(call: Value) -> Result<Checked, Box<FailureEnvelope>> {
    let (selected, request) = decode_call::<O>(call).map_err(Box::new)?;
    O::validate_external(&request)
        .map_err(|failure| Box::new(FailureEnvelope::new(Some(O::NAME), failure)))?;
    Ok((selected, O::needs(&request)))
}

/// The answer of the port when the operation gives it, else the `internal` failure.
fn checked(
    checks: &Checks,
    answer: Result<Value, FailureEnvelope>,
) -> Result<Value, Box<FailureEnvelope>> {
    let given = match &answer {
        Ok(success) if checks.success.is_valid(success) => None,
        Err(failure) if declared(checks, failure) => None,
        Ok(success) => Some(format!("a success that its schema refuses: {success}")),
        Err(failure) => Some(format!(
            "a failure that it does not give, with status {}: {}",
            failure.status_code(),
            failure.error
        )),
    };
    let Some(given) = given else {
        return answer.map_err(Box::new);
    };
    Err(Box::new(
        FailureEnvelope::new(Some(checks.name), OperationFailure::Internal).with_cause(Some(
            format!(
                "the read operations port answered `{}` with {given}",
                checks.name
            ),
        )),
    ))
}

/// Whether the operation gives `failure`: its body is a common failure or a read failure as the
/// dispatch writes it, it has no response meta, and its status is the one of its kind.
fn declared(checks: &Checks, failure: &FailureEnvelope) -> bool {
    let body = &failure.error;
    let status = exact::<OperationFailure>(body)
        .map(|common| common.status_code())
        .or_else(|| exact::<ReadFailure>(body).map(|read| read.status_code()));
    failure.meta == ResponseMeta::default()
        && status == Some(failure.status_code())
        && checks.statuses.contains(&failure.status_code())
}

/// `body` as a `T`, when `T` writes it back as the same JSON: no member is unknown or left out.
fn exact<T: DeserializeOwned + Serialize>(body: &Value) -> Option<T> {
    let value = T::deserialize(body).ok()?;
    (serde_json::to_value(&value).ok()? == *body).then_some(value)
}

/// Closes each object schema in `schema` that names its properties or joins
/// other schemas, so an object may have no member that the schema does not
/// name. `unevaluatedProperties` counts the members that a passing `allOf`,
/// `anyOf` or `oneOf` branch of the same object names. A branch itself stays
/// open: it can state a part of the object only, as the `anyOf` of a record
/// that ties its archive commit to its status does.
fn close(schema: &mut Value) {
    close_schema(schema, false);
}

fn close_schema(schema: &mut Value, branch: bool) {
    let Value::Object(schema) = schema else {
        return;
    };
    let names = ["properties", "allOf", "anyOf", "oneOf"]
        .iter()
        .any(|keyword| schema.contains_key(*keyword));
    let open = !schema.contains_key("additionalProperties")
        && !schema.contains_key("unevaluatedProperties");
    if names && open && !branch {
        schema.insert("unevaluatedProperties".into(), Value::Bool(false));
    }
    for (keyword, value) in schema.iter_mut() {
        match (keyword.as_str(), value) {
            (
                "properties" | "patternProperties" | "$defs" | "definitions",
                Value::Object(named),
            ) => {
                named
                    .values_mut()
                    .for_each(|value| close_schema(value, false));
            }
            ("allOf" | "anyOf" | "oneOf", Value::Array(branches)) => {
                branches
                    .iter_mut()
                    .for_each(|value| close_schema(value, true));
            }
            ("prefixItems", Value::Array(each)) => {
                each.iter_mut().for_each(|value| close_schema(value, false));
            }
            (
                "items" | "additionalProperties" | "not" | "if" | "then" | "else" | "contains",
                value,
            ) => close_schema(value, false),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CHECKS;
    use provenance_store::operations::read_operations::READ_OPERATIONS;

    /// Implementation aid: the table names the operations of the published list, in its order,
    /// and each closed success schema compiles.
    #[test]
    fn the_checks_cover_the_published_list() {
        let names: Vec<_> = CHECKS.iter().map(|checks| checks.name).collect();
        assert_eq!(names, READ_OPERATIONS);
    }
}
