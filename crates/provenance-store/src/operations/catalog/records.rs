//! Registered graph reads reuse the native reader and frame their own identity.
use super::{
    failures::ReadError, shapes::handled, ExecutionNeed, ExecutionNeeds, Operation,
    OperationFuture, PreparedContext,
};
use provenance_core::protocol::{
    self, failure::OperationError, repository::RepositoryInfo, QueryResponse,
};

macro_rules! query {
    (
        $name:ident,
        $wire:literal,
        $request:ident,
        $result:ident,
        $handler:ident,
        $answer:ident,
        $needs:expr
    ) => {
        $crate::operations::catalog::shapes::graph_read_operation!(
            pub $name,
            $wire,
            protocol::$request,
            QueryResponse<protocol::$result>,
            &[409, 500],
            $needs,
            validate = |request: &protocol::$request| {
                request
                    .validate()
                    .map_err(provenance_core::protocol::QueryValidation::into_failure)
            },
            |context, request| async move {
                let mut answer = if context.external {
                    crate::operations::queries::$answer(
                        &context.repository,
                        context.policy,
                        request,
                    )
                    .await?
                } else {
                    crate::operations::queries::$handler(
                        &context.repository,
                        context.policy,
                        request,
                    )
                    .await?
                };
                if context.external && answer.stamp.policy == protocol::StampPolicy::CatchUpFailed {
                    answer.freshness_error =
                        Some("catch-up failed; answer uses the stored projection".to_owned());
                }
                let mut response = QueryResponse::new($wire, answer);
                if context.external && response.stamp.policy == protocol::StampPolicy::CatchUpFailed
                {
                    response.freshness_cause =
                        Some(protocol::read_failure::FreshnessCause::CatchUpFailed);
                }
                Ok(response)
            }
        );
    };
}
query!(
    ReadDocument,
    "read-document",
    ReadDocumentQuery,
    ReadDocumentResult,
    read_document,
    read_document_answer,
    graph_needs
);
query!(
    Get,
    "get",
    GetQuery,
    GetResult,
    get,
    get_answer,
    graph_needs
);
query!(
    ResolveRecord,
    "resolve-record",
    ResolveRecordQuery,
    ResolveRecordResult,
    resolve_record,
    resolve_record_answer,
    graph_needs
);
query!(
    Search,
    "search",
    SearchQuery,
    SearchResult,
    search,
    search_answer,
    graph_needs
);
query!(
    Neighbors,
    "neighbors",
    NeighborsQuery,
    NeighborsResult,
    neighbors,
    neighbors_answer,
    graph_needs
);
query!(
    Trace,
    "trace",
    TraceQuery,
    TraceResult,
    trace,
    trace_answer,
    graph_needs
);

/// Gives the engine version and the protocol version of one repository. The operation takes
/// no input, so its request is the unit type.
pub struct Info;
impl Operation for Info {
    type Request = ();
    type Success = RepositoryInfo;
    type Failure = OperationError<ReadError>;
    const NAME: &'static str = "info";
    const CONTEXT: super::ContextKind = super::ContextKind::Repository;
    const FAILURE_STATUSES: &'static [u16] = &[409];
    fn needs((): &Self::Request) -> ExecutionNeeds {
        &[ExecutionNeed::GraphStorage]
    }
    fn run(
        context: PreparedContext,
        (): Self::Request,
    ) -> OperationFuture<Self::Success, Self::Failure> {
        handled(async move {
            let context = context.repository()?;
            let info = crate::operations::engine_info(&context.repository);
            Ok(RepositoryInfo {
                engine_version: info.engine_version,
                protocol_version: info.protocol_version,
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
#[cfg(feature = "host")]
pub(super) use query;
