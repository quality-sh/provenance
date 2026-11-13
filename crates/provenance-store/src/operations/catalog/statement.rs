use super::{ExecutionNeeds, Operation, OperationFuture, PreparedContext};
use provenance_core::protocol::{failure::OperationFailure, CheckStatementRequest};
use provenance_macros::rule;
use provenance_ste100::Report;

pub struct CheckStatement;

impl Operation for CheckStatement {
    type Request = CheckStatementRequest;
    type Success = Report;
    /// The analysis gives a report for each statement, so the handler has no failure.
    type Failure = OperationFailure;
    const NAME: &'static str = "check-statement";

    /// Keeps statement checks independent of repository state.
    #[rule("rule_ste_sdk_statement_repository_independence")]
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[]
    }

    /// Returns the authoritative descriptive-text report without translating it.
    #[rule("rule_ste_sdk_statement_report")]
    fn run(
        _: PreparedContext,
        request: Self::Request,
    ) -> OperationFuture<Report, OperationFailure> {
        Box::pin(async move {
            Ok(crate::statement_analysis::check_statement(
                &request.statement,
                None,
            ))
        })
    }
}
