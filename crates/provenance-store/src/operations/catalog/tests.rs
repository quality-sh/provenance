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
        assert_eq!(error.error["kind"], "invalid_input");
    }
}

#[tokio::test]
async fn unknown_operations_never_claim_an_execution_identity() {
    let error = invoke("attacker-name", SDK_PROTOCOL_VERSION, json!({}))
        .await
        .unwrap_err();
    assert_eq!(error.error, json!({"kind":"unknown_operation"}));
    assert_eq!(serde_json::to_value(error.meta).unwrap(), json!({}));
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
    let definition = super::definitions()
        .iter()
        .find(|entry| entry.name == "check-statement")
        .unwrap();
    let schema = definition.mcp_input_schema();
    let validator = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema)
        .unwrap();
    assert!(validator.is_valid(&json!({"data":{"statement":"Stop; wait."}})));
}

#[cfg(feature = "schema")]
#[test]
fn mcp_query_schema_validates_only_the_selected_request_shape() {
    let definition: &super::Definition = super::definitions()
        .iter()
        .find(|entry| entry.name == "list-rules")
        .unwrap();
    let schema: serde_json::Value = definition.mcp_input_schema();
    let validator: jsonschema::JSONSchema = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&schema)
        .unwrap();

    for value in [
        json!({}),
        json!({"query":"search", "text":"bounded"}),
        json!({"query":"stale", "base":"main"}),
        json!({"query":"resolve-symbol", "file":"src/lib.rs"}),
    ] {
        assert!(validator.is_valid(&value), "valid query shape: {value}");
    }
    for value in [
        json!({"query":"search"}),
        json!({"query":"stale"}),
        json!({"query":"resolve-symbol"}),
        json!({"query":"search", "text":"bounded", "base":"main"}),
        json!({"text":"bounded"}),
    ] {
        assert!(!validator.is_valid(&value), "invalid query shape: {value}");
    }
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
    let definition = super::schema::raw_definition::<FailureFixture>();
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
fn resource_operations_have_unique_registered_contracts() {
    let names: Vec<_> = super::definitions()
        .iter()
        .map(|entry| entry.name)
        .collect();
    let unique = names
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(names.len(), unique.len());
    for required in [
        "check-statement",
        "list-requirements",
        "update-requirement",
        "questions-create-discussion",
        "create-proposal-assertion",
        "complete-verification",
    ] {
        assert!(unique.contains(required), "{required}");
    }
    for retired in ["get", "upsert-contribution", "post-thread-message"] {
        assert!(!unique.contains(retired), "{retired}");
    }
}

#[cfg(feature = "schema")]
#[test]
fn query_registrations_keep_typed_scalar_parameters() {
    let definition = super::definitions()
        .iter()
        .find(|definition| definition.name == "list-rules")
        .unwrap();
    let stale = definition
        .registration
        .queries
        .iter()
        .find(|query| query.name == "stale")
        .unwrap();
    let base = stale
        .parameters
        .iter()
        .find(|parameter| parameter.name == "base")
        .unwrap();
    assert_eq!(
        super::parse_parameter_value(base, "0123456789abcdef").unwrap(),
        json!("0123456789abcdef"),
        "schema: {}",
        base.schema
    );
    for parameter in stale
        .parameters
        .iter()
        .filter(|parameter| matches!(parameter.name, "head" | "line" | "symbol"))
    {
        assert!(
            parameter.schema.get("default").is_none(),
            "non-null query parameter {} has an invalid default: {}",
            parameter.name,
            parameter.schema
        );
    }
    let member = super::definitions()
        .iter()
        .find(|definition| definition.name == "get-rule")
        .unwrap();
    let trace = member
        .registration
        .queries
        .iter()
        .find(|query| query.name == "trace")
        .unwrap();
    assert!(trace
        .parameters
        .iter()
        .any(|parameter| parameter.name == "limit"));
}

#[cfg(feature = "schema")]
#[test]
fn relation_filters_are_typed_array_query_parameters() {
    let definition = super::definitions()
        .iter()
        .find(|definition| definition.name == "get-source")
        .unwrap();
    for name in ["neighbors", "trace"] {
        let route = definition
            .registration
            .queries
            .iter()
            .find(|query| query.name == name)
            .unwrap();
        let relations = route
            .parameters
            .iter()
            .find(|parameter| parameter.name == "relations")
            .unwrap_or_else(|| panic!("{name} omitted relations"));
        assert_eq!(relations.schema["type"], "array");
        assert_eq!(
            super::parse_parameter_value(relations, "supersedes,depends_on").unwrap(),
            json!(["supersedes", "depends_on"])
        );
        assert_eq!(
            super::serialize_parameter_value(relations, &json!(["supersedes", "depends_on"]))
                .unwrap(),
            "supersedes,depends_on"
        );
    }
}

#[cfg(feature = "schema")]
#[test]
fn proposal_fact_members_declare_object_success_payloads() {
    for name in ["get-proposal-assertion", "get-proposal-disposition"] {
        let definition = super::definitions()
            .iter()
            .find(|definition| definition.name == name)
            .unwrap();
        let schema = definition.success_schema();
        let data = &schema["properties"]["data"];
        let data = data
            .get("$ref")
            .and_then(serde_json::Value::as_str)
            .and_then(|reference| reference.strip_prefix("#/$defs/"))
            .and_then(|definition| schema["$defs"].get(definition))
            .unwrap_or(data);
        assert_eq!(data["type"], "object", "{name}: {schema}");
    }
}

#[cfg(feature = "schema")]
#[test]
fn every_public_collection_declares_bounded_paging() {
    for definition in super::definitions().iter().filter(|definition| {
        definition.method == super::HttpMethod::Get
            && definition.registration.response.kind == super::ResponseKind::Items
    }) {
        let parameters = definition.parameters();
        for name in ["limit", "cursor"] {
            assert!(
                parameters
                    .iter()
                    .any(|parameter| parameter.location == "query" && parameter.name == name),
                "{} omitted {name}",
                definition.name
            );
        }
        assert!(
            definition.registration.controls.pagination,
            "{} does not move page controls into response metadata",
            definition.name
        );
    }
}

#[cfg(feature = "schema")]
#[test]
fn public_member_reads_do_not_use_collection_response_adapters() {
    for definition in super::definitions().iter().filter(|definition| {
        definition.method == super::HttpMethod::Get
            && definition.registration.response.kind == super::ResponseKind::Resource
    }) {
        assert!(
            matches!(
                definition.registration.response.adapter,
                super::ResponseAdapter::Direct | super::ResponseAdapter::Result
            ),
            "{} adapts one member from a collection response",
            definition.name
        );
    }
}

#[cfg(feature = "schema")]
#[test]
fn repository_info_has_a_numeric_version_and_closed_request() {
    let definition = super::schema::raw_definition::<super::Info>();
    assert_eq!(
        definition.success_schema["properties"]["protocol_version"]["type"],
        "integer"
    );
    let validator = jsonschema::JSONSchema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .compile(&definition.request_schema)
        .unwrap();
    assert!(validator.is_valid(&json!({})));
    assert!(!validator.is_valid(&json!({"scope":"default"})));
}

#[cfg(feature = "schema")]
#[test]
fn request_field_stripping_does_not_change_nested_definitions() {
    #[allow(dead_code)]
    #[derive(schemars::JsonSchema)]
    struct Nested {
        id: String,
    }
    #[allow(dead_code)]
    #[derive(schemars::JsonSchema)]
    struct Request {
        id: String,
        nested: Nested,
    }
    let mut schema = Some(super::schema::request_envelope(
        super::schema::type_schema::<Request>(schemars::generate::Contract::Deserialize),
    ));
    super::schema::hide_bound_request_field(&mut schema, "id");
    let schema = schema.unwrap();

    assert!(schema["properties"]["data"]["properties"]
        .get("id")
        .is_none());
    assert!(schema["$defs"]["Nested"]["properties"].get("id").is_some());
}

#[cfg(feature = "schema")]
#[test]
fn data_free_failure_schema_does_not_advertise_read_refusals() {
    let statement = super::schema::raw_definition::<super::CheckStatement>();
    assert!(!statement
        .failure_schema
        .to_string()
        .contains("no_projection"));
    assert!(!statement.http_statuses.contains(&409));
    assert!(statement.http_statuses.contains(&405));
    let get = super::schema::raw_definition::<super::Get>();
    assert!(get.failure_schema.to_string().contains("no_projection"));
    assert!(get.http_statuses.contains(&409));
    assert!(get.http_statuses.contains(&405));
}

#[cfg(feature = "schema")]
#[test]
fn every_discussion_message_member_schema_accepts_one_message() {
    let message = json!({
        "data": {
            "schema_version": 2,
            "scope_id": "default",
            "id": "message_a",
            "thread_id": "thread_a",
            "role": "user",
            "body": "One message.",
            "created_at": 1
        },
        "meta": {}
    });
    for definition in super::definitions().iter().filter(|definition| {
        definition.name.ends_with("get-discussion-message")
            || definition.name.ends_with("get-legacy-message")
    }) {
        let success_schema = definition.success_schema();
        let validator = jsonschema::JSONSchema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .compile(&success_schema)
            .unwrap();
        assert!(
            validator.is_valid(&message),
            "{} rejects a single Message: {:?}",
            definition.name,
            validator
                .validate(&message)
                .unwrap_err()
                .collect::<Vec<_>>()
        );
    }
}
