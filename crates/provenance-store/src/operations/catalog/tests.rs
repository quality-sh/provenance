use super::invoke;
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::SDK_PROTOCOL_VERSION;
use serde_json::json;

#[tokio::test]
async fn statements_return_the_exact_data_free_analyzer_report() {
    for statement in ["Install the cover.", "Arrêt; wait.", ""] {
        let expected =
            serde_json::to_value(provenance_ste100::check_descriptive(statement)).unwrap();
        let actual = invoke(
            "check-statement",
            SDK_PROTOCOL_VERSION,
            json!({"request":{"statement":statement}}),
        )
        .await
        .unwrap();
        assert_eq!(actual, expected);
    }
}

#[tokio::test]
async fn statement_call_is_closed_and_refuses_invalid_payloads() {
    for value in [
        json!({}),
        json!({"request":{}}),
        json!({"request":{"statement":null}}),
        json!({"request":{"statement":9}}),
        json!({"request":{"statement":"text", "base_revision":"x"}}),
        json!({"request":{"statement":"text"},"context":{"repository":"/tmp"}}),
    ] {
        let error = invoke("check-statement", SDK_PROTOCOL_VERSION, value)
            .await
            .unwrap_err();
        assert!(matches!(error.error, OperationFailure::InvalidInput { .. }));
        assert_eq!(error.operation.as_deref(), Some("check-statement"));
    }
}

#[tokio::test]
async fn unknown_operations_never_claim_an_execution_identity() {
    let error = invoke("attacker-name", SDK_PROTOCOL_VERSION, json!({}))
        .await
        .unwrap_err();
    assert_eq!(error.error, OperationFailure::UnknownOperation);
    assert_eq!(error.operation, None);
}

#[tokio::test]
async fn dispatch_refuses_incompatible_versions_before_decoding() {
    let error = invoke("check-statement", SDK_PROTOCOL_VERSION + 1, json!(null))
        .await
        .unwrap_err();
    assert_eq!(
        error.error,
        OperationFailure::ProtocolMismatch {
            requested: SDK_PROTOCOL_VERSION + 1,
            supported: SDK_PROTOCOL_VERSION
        }
    );
}

#[cfg(feature = "schema")]
#[test]
fn mcp_schema_resolves_definitions_at_the_document_root() {
    let definition = super::definitions().remove(0);
    let schema = definition.mcp_input_schema();
    let validator = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema)
        .unwrap();
    assert!(validator.is_valid(&json!({"protocol_version":SDK_PROTOCOL_VERSION,"call":{"request":{"statement":"Stop; wait."}}})));
}

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

#[tokio::test]
async fn handler_failure_cannot_change_its_declared_wire_variant() {
    let failure =
        super::invoke::invoke_erased::<FailureFixture>(json!({"request":{"statement":"x"}}))
            .await
            .unwrap_err();
    assert_eq!(failure.error, OperationFailure::AccessDenied);
}

#[cfg(feature = "schema")]
#[test]
fn handler_family_participates_in_the_generated_failure_contract() {
    let definition = super::schema::definition::<FailureFixture>();
    assert!(definition.failure_schema["$defs"]
        .get("DeclaredFailure")
        .is_some());
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
async fn unsupported_handler_fields_cannot_disappear_during_erasure() {
    let failure = super::invoke::invoke_erased::<ExtendedFailureFixture>(
        json!({"request":{"statement":"x"}}),
    )
    .await
    .unwrap_err();
    assert_eq!(failure.error, OperationFailure::Internal);
}
