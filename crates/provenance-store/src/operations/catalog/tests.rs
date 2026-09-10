use super::invoke;
use provenance_core::protocol::failure::OperationFailure;
use provenance_core::SDK_PROTOCOL_VERSION;
use serde_json::json;

mod ideation;

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
        assert_eq!(error.error["kind"], "invalid_input");
        assert_eq!(error.operation.as_deref(), Some("check-statement"));
    }
}

#[tokio::test]
async fn unknown_operations_never_claim_an_execution_identity() {
    let error = invoke("attacker-name", SDK_PROTOCOL_VERSION, json!({}))
        .await
        .unwrap_err();
    assert_eq!(error.error, json!({"kind":"unknown_operation"}));
    assert_eq!(error.operation, None);
}

#[tokio::test]
async fn dispatch_refuses_incompatible_versions_before_decoding() {
    let error = invoke("check-statement", SDK_PROTOCOL_VERSION + 1, json!(null))
        .await
        .unwrap_err();
    assert_eq!(
        error.error,
        json!({"kind":"protocol_mismatch","requested":SDK_PROTOCOL_VERSION+1,"supported":SDK_PROTOCOL_VERSION})
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
    assert_eq!(failure.error, json!({"kind":"access_denied"}));
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
async fn declared_handler_fields_cannot_disappear_during_erasure() {
    let failure = super::invoke::invoke_erased::<ExtendedFailureFixture>(
        json!({"request":{"statement":"x"}}),
    )
    .await
    .unwrap_err();
    assert_eq!(
        failure.error,
        json!({"kind":"access_denied", "detail":true})
    );
}

#[cfg(feature = "schema")]
#[test]
fn baseline_operations_have_one_registered_contract_each() {
    let names: Vec<_> = super::definitions()
        .into_iter()
        .map(|entry| entry.name)
        .collect();
    assert_eq!(
        names,
        [
            "check-statement",
            "create-contribution",
            "upsert-contribution",
            "create-synthesis-packet",
            "upsert-synthesis-packet",
            "set-requirement-refines",
            "clear-requirement-refines",
            "add-requirement-depends-on",
            "clear-requirement-depends-on",
            "add-requirement-supersedes",
            "clear-requirement-supersedes",
            "set-requirement-spawned-by",
            "clear-requirement-spawned-by",
            "add-rule-requirement",
            "clear-rule-requirement",
            "add-rule-resolution",
            "clear-rule-resolution",
            "add-resolution-requirement",
            "clear-resolution-requirement",
            "add-resolution-supersedes",
            "clear-resolution-supersedes",
            "add-source-supersedes",
            "clear-source-supersedes",
            "set-question-contradicts",
            "clear-question-contradicts",
            "clear-source-reference",
            "claim-topic",
            "release-topic",
            "close-topic",
            "claim-question",
            "release-question",
            "answer-question",
            "update-source",
            "update-resolution",
            "update-requirement",
            "update-rule",
            "update-domain",
            "update-boundary",
            "update-topic",
            "update-question",
            "create-domain",
            "create-boundary",
            "create-topic",
            "create-question",
            "create-source",
            "create-requirement",
            "create-rule",
            "create-resolution",
            "add-source-reference",
            "plan",
            "apply",
            "begin-verification",
            "complete-verification",
            "info",
            "get",
            "search",
            "neighbors",
            "trace",
            "impact",
            "resolve-symbol",
            "evidence",
            "stale",
            "verification-runs",
            "verification-bindings",
            "list-threads",
            "list-messages",
            "post-thread-message",
            "list-proposals",
            "list-dispositions",
            "list-assertions",
            "create-proposal",
            "create-assertion",
            "create-disposition"
        ]
    );
}

#[cfg(feature = "schema")]
#[test]
fn repository_info_has_a_literal_version_and_repository_only_context() {
    let definition = super::definitions()
        .into_iter()
        .find(|definition| definition.name == "info")
        .unwrap();
    assert_eq!(
        definition.success_schema["properties"]["protocol_version"]["const"],
        SDK_PROTOCOL_VERSION
    );
    let validator = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&definition.request_schema)
        .unwrap();
    assert!(validator.is_valid(&json!({"context":{"repository":"first"},"request":{}})));
    assert!(!validator
        .is_valid(&json!({"context":{"repository":"first","scope":"default"},"request":{}})));
}

#[cfg(feature = "schema")]
#[test]
fn data_free_failure_schema_does_not_advertise_read_refusals() {
    let definitions = super::definitions();
    let statement = definitions
        .iter()
        .find(|entry| entry.name == "check-statement")
        .unwrap();
    assert!(!statement
        .failure_schema
        .to_string()
        .contains("no_projection"));
    assert!(!statement.http_statuses.contains(&409));
    let get = definitions
        .iter()
        .find(|entry| entry.name == "get")
        .unwrap();
    assert!(get.failure_schema.to_string().contains("no_projection"));
    assert!(get.http_statuses.contains(&409));
}
