use provenance_core::protocol::failure::{ErasedFailure, OperationFailure};
use serde_json::json;

mod authorize_order;
mod ideation;
mod scope_mismatch;

#[derive(Debug, serde::Serialize, thiserror::Error)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
enum DeclaredFailure {
    #[error("fixture refusal")]
    AccessDenied,
}
impl From<DeclaredFailure> for OperationFailure {
    fn from(_: DeclaredFailure) -> Self {
        Self::UnknownTarget
    }
}
struct FailureFixture;
impl super::Operation for FailureFixture {
    type Request = provenance_core::protocol::CheckStatementRequest;
    type Success = provenance_ste100::Report;
    type Failure = DeclaredFailure;
    const NAME: &'static str = "failure-fixture";
    fn needs(_: &Self::Request) -> super::ExecutionNeeds {
        &[]
    }
    fn run(
        _: super::PreparedContext,
        _: Self::Request,
    ) -> super::OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async { Err(DeclaredFailure::AccessDenied) })
    }
}

/// The failure that fixture `O` frames for one call on a host with no repositories.
async fn framed_failure<O: super::Operation>() -> ErasedFailure {
    let host = super::Direct(std::sync::Arc::new(super::context::NoRepositories));
    let call = json!({"request":{"statement":"x"}});
    crate::test_support::call::<O, _>(call, host)
        .await
        .unwrap_err()
}

#[tokio::test]
async fn handler_failure_cannot_change_its_declared_wire_variant() {
    let failure = framed_failure::<FailureFixture>().await;
    assert_eq!(failure.error, json!({"kind":"access_denied"}));
}

#[derive(Debug, serde::Serialize, thiserror::Error)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ExtendedFailure {
    #[error("fixture refusal with a field the wire family does not support")]
    AccessDenied { detail: bool },
}
struct ExtendedFailureFixture;
impl super::Operation for ExtendedFailureFixture {
    type Request = provenance_core::protocol::CheckStatementRequest;
    type Success = provenance_ste100::Report;
    type Failure = ExtendedFailure;
    const NAME: &'static str = "extended-failure-fixture";
    fn needs(_: &Self::Request) -> super::ExecutionNeeds {
        &[]
    }
    fn run(
        _: super::PreparedContext,
        _: Self::Request,
    ) -> super::OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async { Err(ExtendedFailure::AccessDenied { detail: true }) })
    }
}
#[tokio::test]
async fn declared_handler_fields_cannot_disappear_during_erasure() {
    let failure = framed_failure::<ExtendedFailureFixture>().await;
    assert_eq!(
        failure.error,
        json!({"kind":"access_denied", "detail":true})
    );
}
