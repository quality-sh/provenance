//! Registered graph reads reuse the native reader and frame their own identity.
use super::{
    failures::ReadError, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
};
use provenance_core::protocol::{
    self,
    failure::OperationFailure,
    repository::{InfoRequest, RepositoryInfo},
    QueryResponse,
};

macro_rules! query {
    ($name:ident, $wire:literal, $request:ident, $result:ident, $handler:ident, $needs:expr) => {
        pub struct $name;
        impl Operation for $name {
            type Request = protocol::$request;
            type Success = QueryResponse<protocol::$result>;
            type Failure = ReadError;
            fn failure_status(error: &ReadError) -> u16 {
                error.status()
            }
            const NAME: &'static str = $wire;
            const CONTEXT: super::ContextKind = super::ContextKind::Scoped;
            const FAILURE_STATUSES: &'static [u16] = &[409, 500];
            fn needs(request: &Self::Request) -> ExecutionNeeds {
                ($needs)(request)
            }
            fn validate_external(request: &Self::Request) -> Result<(), OperationFailure> {
                request
                    .validate()
                    .map_err(provenance_core::protocol::QueryValidation::into_failure)
            }
            fn run(
                context: PreparedContext,
                request: Self::Request,
            ) -> OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let context = context.graph()?;
                    let mut answer = crate::operations::queries::$handler(
                        Some(context.root),
                        &context.scope,
                        context.policy,
                        request,
                    )
                    .await?;
                    if context.external
                        && answer.stamp.policy == protocol::StampPolicy::CatchUpFailed
                    {
                        answer.freshness_error =
                            Some("catch-up failed; answer uses the stored projection".to_owned());
                    }
                    let mut response = QueryResponse::new(Self::NAME, answer);
                    if context.external
                        && response.stamp.policy == protocol::StampPolicy::CatchUpFailed
                    {
                        response.freshness_cause =
                            Some(protocol::read_failure::FreshnessCause::CatchUpFailed);
                    }
                    Ok(response)
                })
            }
        }
    };
}
query!(Get, "get", GetQuery, GetResult, get, graph_needs);
query!(
    Search,
    "search",
    SearchQuery,
    SearchResult,
    search,
    graph_needs
);
query!(
    Neighbors,
    "neighbors",
    NeighborsQuery,
    NeighborsResult,
    neighbors,
    graph_needs
);
query!(Trace, "trace", TraceQuery, TraceResult, trace, graph_needs);

pub struct Info;
impl Operation for Info {
    type Request = InfoRequest;
    type Success = RepositoryInfo;
    type Failure = ReadError;
    fn failure_status(error: &ReadError) -> u16 {
        error.status()
    }
    const NAME: &'static str = "info";
    const CONTEXT: super::ContextKind = super::ContextKind::Repository;
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[ExecutionNeed::GraphStorage]
    }
    fn run(
        context: PreparedContext,
        _: Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async move {
            let context = context.repository()?;
            let info = crate::operations::engine_info(Some(context.root))?;
            Ok(RepositoryInfo {
                engine_version: info.engine_version,
                protocol_version: info.protocol_version,
                state_schema_version: info.state_schema_version,
                repository: context.requested_target,
            })
        })
    }
}

const fn graph_needs<R>(_: &R) -> ExecutionNeeds {
    &[
        ExecutionNeed::GraphStorage,
        ExecutionNeed::ProjectionMaintenance,
    ]
}
pub(super) use query;
