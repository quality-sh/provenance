use provenance_core::protocol::failure::OperationFailure;
use provenance_store::operations::catalog::{
    CheckStatement, ExecutionNeeds, Get, Info, Operation, OperationFuture, PreparedContext,
};
use serde_json::json;

mod review_identity;
mod review_kinds;
mod schema_precision;

#[test]
fn mcp_schema_resolves_definitions_at_the_document_root() {
    let definition = crate::definitions()
        .iter()
        .find(|entry| entry.name == "check-statement")
        .unwrap();
    let schema = definition.mcp_input_schema();
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap();
    assert!(validator.is_valid(&json!({"data":{"statement":"Stop; wait."}})));
}

#[test]
fn mcp_query_schema_validates_only_the_selected_request_shape() {
    let definition: &crate::Definition = crate::definitions()
        .iter()
        .find(|entry| entry.name == "list-rules")
        .unwrap();
    let schema: serde_json::Value = definition.mcp_input_schema();
    let validator: jsonschema::Validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&schema)
        .unwrap();

    for value in [
        json!({}),
        json!({"query":"search"}),
        json!({"query":"search", "text":"bounded"}),
        json!({"query":"stale", "base":"main"}),
        json!({"query":"resolve-symbol", "file":"src/lib.rs"}),
    ] {
        assert!(validator.is_valid(&value), "valid query shape: {value}");
    }
    for value in [
        json!({"query":"stale"}),
        json!({"query":"resolve-symbol"}),
        json!({"query":"search", "text":"bounded", "base":"main"}),
        json!({"text":"bounded"}),
    ] {
        assert!(!validator.is_valid(&value), "invalid query shape: {value}");
    }
}
#[derive(Debug, serde::Serialize, thiserror::Error, schemars::JsonSchema)]
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
impl Operation for FailureFixture {
    type Request = provenance_core::protocol::CheckStatementRequest;
    type Success = provenance_ste100::Report;
    type Failure = DeclaredFailure;
    const NAME: &'static str = "failure-fixture";
    fn needs(_: &Self::Request) -> ExecutionNeeds {
        &[]
    }
    fn run(_: PreparedContext, _: Self::Request) -> OperationFuture<Self::Success, Self::Failure> {
        Box::pin(async { Err(DeclaredFailure::AccessDenied) })
    }
}

#[test]
fn handler_family_participates_in_the_generated_failure_contract() {
    let definition = crate::schema::raw_definition::<FailureFixture>();
    assert!(definition.failure_schema["$defs"]
        .get("DeclaredFailure")
        .is_some());
}

#[test]
fn resource_operations_have_unique_registered_contracts() {
    let names: Vec<_> = crate::definitions()
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

#[test]
fn query_registrations_keep_typed_scalar_parameters() {
    let definition = crate::definitions()
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
        crate::parse_parameter_value(base, "0123456789abcdef").unwrap(),
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
    let member = crate::definitions()
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

#[test]
fn relation_filters_are_typed_array_query_parameters() {
    let definition = crate::definitions()
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
            crate::parse_parameter_value(relations, "supersedes,depends_on").unwrap(),
            json!(["supersedes", "depends_on"])
        );
        assert_eq!(
            crate::serialize_parameter_value(relations, &json!(["supersedes", "depends_on"]))
                .unwrap(),
            "supersedes,depends_on"
        );
    }
}

#[test]
fn proposal_fact_members_declare_object_success_payloads() {
    for name in ["get-proposal-assertion", "get-proposal-disposition"] {
        let definition = crate::definitions()
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

#[test]
fn every_public_collection_declares_bounded_paging() {
    for definition in crate::definitions().iter().filter(|definition| {
        definition.method == crate::HttpMethod::Get
            && definition.registration.response.kind == crate::ResponseKind::Items
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

#[test]
fn public_member_reads_do_not_use_collection_response_adapters() {
    for definition in crate::definitions().iter().filter(|definition| {
        definition.method == crate::HttpMethod::Get
            && definition.registration.response.kind == crate::ResponseKind::Resource
    }) {
        assert!(
            matches!(
                definition.registration.response.adapter,
                crate::ResponseAdapter::Direct | crate::ResponseAdapter::Result
            ),
            "{} adapts one member from a collection response",
            definition.name
        );
    }
}

#[test]
fn repository_info_has_a_numeric_version_and_closed_request() {
    let definition = crate::schema::raw_definition::<Info>();
    assert_eq!(
        definition.success_schema["properties"]["protocol_version"]["type"],
        "integer"
    );
    let validator = jsonschema::options()
        .with_draft(jsonschema::Draft::Draft202012)
        .build(&definition.request_schema)
        .unwrap();
    assert!(validator.is_valid(&json!({})));
    assert!(!validator.is_valid(&json!({"scope":"default"})));
}

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
    let mut schema = Some(crate::schema::request_envelope(
        crate::schema::type_schema::<Request>(schemars::generate::Contract::Deserialize),
    ));
    crate::schema::hide_bound_request_field(&mut schema, "id");
    let schema = schema.unwrap();

    assert!(schema["properties"]["data"]["properties"]
        .get("id")
        .is_none());
    assert!(schema["$defs"]["Nested"]["properties"].get("id").is_some());
}

#[test]
fn data_free_failure_schema_does_not_advertise_read_refusals() {
    let statement = crate::schema::raw_definition::<CheckStatement>();
    assert!(!statement
        .failure_schema
        .to_string()
        .contains("no_projection"));
    assert!(!statement.http_statuses.contains(&409));
    assert!(statement.http_statuses.contains(&405));
    let get = crate::schema::raw_definition::<Get>();
    assert!(get.failure_schema.to_string().contains("no_projection"));
    assert!(get.http_statuses.contains(&409));
    assert!(get.http_statuses.contains(&405));
}

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
    for definition in crate::definitions().iter().filter(|definition| {
        definition.name.ends_with("get-discussion-message")
            || definition.name.ends_with("get-legacy-message")
    }) {
        let success_schema = definition.success_schema();
        let validator = jsonschema::options()
            .with_draft(jsonschema::Draft::Draft202012)
            .build(&success_schema)
            .unwrap();
        assert!(
            validator.is_valid(&message),
            "{} rejects a single Message: {:?}",
            definition.name,
            validator.iter_errors(&message).collect::<Vec<_>>()
        );
    }
}
