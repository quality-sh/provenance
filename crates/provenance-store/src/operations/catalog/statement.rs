use super::{ExecutionNeeds, Operation, OperationFuture, PreparedContext};
use provenance_core::protocol::CheckStatementRequest;
use provenance_macros::rule;
use provenance_ste100::Report;

#[derive(Debug, serde::Serialize, thiserror::Error)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum StatementFailure {}

pub struct CheckStatement;

impl Operation for CheckStatement {
    type Request = CheckStatementRequest;
    type Success = Report;
    type Failure = StatementFailure;
    const NAME: &'static str = "check-statement";

    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[]
    }

    /// Returns the authoritative descriptive-text report without translating it.
    #[rule("rule_ste_sdk_statement_report")]
    fn run(
        _: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Report, StatementFailure> {
        Box::pin(async move { Ok(provenance_ste100::check_descriptive(&request.statement)) })
    }
}
