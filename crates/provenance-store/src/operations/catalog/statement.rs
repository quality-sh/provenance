use super::{ExecutionNeeds, Operation, OperationFuture, PreparedContext};
use provenance_core::protocol::CheckStatementRequest;
use provenance_macros::rule;
use provenance_ste100::Report;
use std::{convert::Infallible, fmt};

/// A statement check has no failure, so this type has no value.
///
/// It is a struct and not an enum with no variants, because the `empty_enums`
/// lint refuses that enum. Its impls give the serialized form, the errors and
/// the schema of such an enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatementFailure(pub Infallible);

impl fmt::Display for StatementFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

impl std::error::Error for StatementFailure {}

impl serde::Serialize for StatementFailure {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // The type has no value, so this function never runs. The drop gives
        // it a statement that coverage can record. With no such statement, the
        // coverage report shows the function as covered.
        drop(serializer);
        match self.0 {}
    }
}

impl<'de> serde::Deserialize<'de> for StatementFailure {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        provenance_core::protocol::empty::deserialize_enum(deserializer, "StatementFailure")
            .map(Self)
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for StatementFailure {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "StatementFailure".into()
    }

    fn schema_id() -> std::borrow::Cow<'static, str> {
        concat!(module_path!(), "::StatementFailure").into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::Schema::from(false)
    }
}

pub struct CheckStatement;

impl Operation for CheckStatement {
    type Request = CheckStatementRequest;
    type Success = Report;
    type Failure = StatementFailure;
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
    ) -> OperationFuture<Report, StatementFailure> {
        Box::pin(async move {
            Ok(crate::statement_analysis::check_statement(
                &request.statement,
                None,
            ))
        })
    }
}
