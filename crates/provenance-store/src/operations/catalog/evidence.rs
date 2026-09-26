//! Evidence reads preserve the native result and conditional live dependencies.
use super::records::query;
use super::{shapes::scoped_read_operation, ExecutionNeed, ExecutionNeeds};
use provenance_core::protocol::{self, QueryResponse};

query!(
    Impact,
    "impact",
    ImpactQuery,
    ImpactResult,
    impact,
    impact_answer,
    source_needs
);
query!(
    ResolveSymbol,
    "resolve-symbol",
    ResolveSymbolQuery,
    ResolveSymbolResult,
    resolve_symbol,
    resolve_symbol_answer,
    source_needs
);
query!(
    Evidence,
    "evidence",
    EvidenceQuery,
    EvidenceResult,
    evidence,
    evidence_answer,
    evidence_needs
);
query!(
    Stale,
    "stale",
    StaleQuery,
    StaleResult,
    stale,
    stale_answer,
    git_needs
);

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
        scoped_read_operation!(
            pub $name,
            $wire,
            protocol::repository::VerificationListRequest,
            Vec<provenance_core::$result>,
            &[409],
            &[
                ExecutionNeed::GraphStorage,
                ExecutionNeed::$need,
            ],
            |store, scope, request| {
                crate::operations::$handler(
                    Some(store.layout.root().to_path_buf()),
                    scope,
                    request.rule.as_ref(),
                )
            }
        );
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
