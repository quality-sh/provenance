//! Evidence reads preserve the native result and conditional live dependencies.
use super::records::query;
use super::{
    failures::ReadError, ExecutionNeed, ExecutionNeeds, Operation, OperationFuture, PreparedContext,
};
use provenance_core::protocol::{self, failure::OperationFailure, QueryResponse};

query!(
    Impact,
    "impact",
    ImpactQuery,
    ImpactResult,
    impact,
    source_needs
);
query!(
    ResolveSymbol,
    "resolve-symbol",
    ResolveSymbolQuery,
    ResolveSymbolResult,
    resolve_symbol,
    source_needs
);
query!(
    Evidence,
    "evidence",
    EvidenceQuery,
    EvidenceResult,
    evidence,
    evidence_needs
);
query!(Stale, "stale", StaleQuery, StaleResult, stale, git_needs);

const fn source_needs<R>(_: &R) -> ExecutionNeeds {
    &[
        ExecutionNeed::GraphStorage,
        ExecutionNeed::ProjectionMaintenance,
        ExecutionNeed::RepositoryFiles,
    ]
}
const fn git_needs<R>(_: &R) -> ExecutionNeeds {
    &[
        ExecutionNeed::GraphStorage,
        ExecutionNeed::ProjectionMaintenance,
        ExecutionNeed::Git,
    ]
}
const fn evidence_needs(request: &protocol::EvidenceQuery) -> ExecutionNeeds {
    if request.base.is_some() {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
            ExecutionNeed::RunStorage,
            ExecutionNeed::Git,
        ]
    } else {
        &[
            ExecutionNeed::GraphStorage,
            ExecutionNeed::ProjectionMaintenance,
            ExecutionNeed::RunStorage,
        ]
    }
}
macro_rules! list {
    ($name:ident, $wire:literal, $result:ident, $handler:ident, $need:ident) => {
        pub struct $name;
        impl Operation for $name {
            type Request = protocol::repository::VerificationListRequest;
            type Success = Vec<provenance_core::$result>;
            type Failure = ReadError;
            const NAME: &'static str = $wire;
            const CONTEXT: super::ContextKind = super::ContextKind::Scope;
            fn needs(_: &Self::Request) -> ExecutionNeeds {
                &[ExecutionNeed::GraphStorage, ExecutionNeed::$need]
            }
            fn failure_status(error: &ReadError) -> u16 {
                error.status()
            }
            fn run(
                context: PreparedContext,
                request: Self::Request,
            ) -> OperationFuture<Self::Success, Self::Failure> {
                Box::pin(async move {
                    let context = context.scope()?;
                    Ok(crate::operations::$handler(
                        Some(context.root),
                        &context.scope,
                        request.rule.as_ref(),
                    )?)
                })
            }
        }
    };
}
list!(
    VerificationRuns,
    "verification-runs",
    VerificationRun,
    verification_runs,
    RunStorage
);
list!(
    VerificationBindings,
    "verification-bindings",
    VerificationBinding,
    verification_bindings,
    GraphStorage
);
