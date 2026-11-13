use super::invoke;
use provenance_core::SDK_PROTOCOL_VERSION;
use provenance_macros::verifies;
use provenance_store::cache::ProjectionFamily;
use serde_json::json;

#[tokio::test]
#[verifies("rule_ste_sdk_statement_report", examples)]
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
#[verifies("rule_ste_sdk_statement_request_schema", examples)]
async fn statement_call_is_closed_and_refuses_invalid_payloads() {
    for (value, field, reason) in [
        (json!({}), "request", "required"),
        (json!({"request":{}}), "request.statement", "required"),
        (
            json!({"request":{"statement":null}}),
            "request.statement",
            "invalid_value",
        ),
        (
            json!({"request":{"statement":9}}),
            "request.statement",
            "invalid_value",
        ),
        (
            json!({"request":{"statement":"text", "base_revision":"x"}}),
            "request.base_revision",
            "unknown_field",
        ),
        (
            json!({"request":{"statement":"text"},"context":{"repository":"/tmp"}}),
            "context",
            "unknown_field",
        ),
    ] {
        let error = invoke("check-statement", SDK_PROTOCOL_VERSION, value)
            .await
            .unwrap_err();
        assert_eq!(error.error["kind"], "invalid_input");
        assert_eq!(error.error["reason"], reason);
        assert_eq!(error.error["field"], field);
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

#[test]
fn registered_operation_dispatch_names_are_unique() {
    let names = crate::dispatch::registered_operation_names_for_test();
    let unique = names
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        names.len(),
        unique.len(),
        "duplicate operation dispatch name"
    );
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

#[test]
fn catalog_registers_each_family_operation() {
    let registered = crate::dispatch::registered_operation_names_for_test();
    for family in ProjectionFamily::ALL {
        for operation in family.catalog_operation_names() {
            assert!(
                registered.contains(operation),
                "{} operation {operation} is absent from the catalog",
                family.family_name()
            );
        }
    }
}

#[tokio::test]
async fn target_discussion_writes_dispatch_by_name() {
    assert!(crate::contains("write-target-discussion"));
    assert!(crate::mutates("write-target-discussion"));
    // The entry decodes the call, so an empty call is refused as input and not as an
    // unknown operation.
    let error = invoke("write-target-discussion", SDK_PROTOCOL_VERSION, json!({}))
        .await
        .unwrap_err();
    assert_eq!(error.error["kind"], "invalid_input");
}

/// A Topic has no field that an update can clear, so the Topic update refuses a clear list.
/// The refusal comes before the host: a host with no repositories gives `unavailable_needs`
/// to each call that decodes.
#[tokio::test]
#[verifies("rule_update_clear_list_only_for_clearable_fields", examples)]
#[verifies("rule_unknown_request_field_names_accepted_fields", examples)]
async fn topic_update_call_refuses_a_clear_list() {
    let call = |request: serde_json::Value| json!({"context":{"repository":"selected","scope":"default"},"request":request});
    let refused = invoke(
        "update-topic",
        SDK_PROTOCOL_VERSION,
        call(json!({"scope_id":"default","id":"topic_one","title":"Renamed","clear_fields":[]})),
    )
    .await
    .unwrap_err();
    assert_eq!(
        refused.error,
        json!({
            "kind":"invalid_input", "field":"request.clear_fields", "reason":"unknown_field",
            "message":"update-topic does not take clear_fields; accepted fields: `scope_id`, \
                       `id`, `expected_etag`, `title`, `status`, `links`, `archived_in_commit`"
        })
    );
    let reached = invoke(
        "update-topic",
        SDK_PROTOCOL_VERSION,
        call(json!({"scope_id":"default","id":"topic_one","title":"Renamed"})),
    )
    .await
    .unwrap_err();
    assert_eq!(reached.error, json!({"kind":"unavailable_needs"}));
}

/// The info operation has no input field. Its call takes a null request and refuses each
/// other request form.
#[tokio::test]
#[verifies("rule_operation_without_input_takes_null_request", examples)]
async fn info_call_takes_only_a_null_request() {
    let info = |call: serde_json::Value| async move {
        invoke("info", SDK_PROTOCOL_VERSION, call)
            .await
            .unwrap_err()
            .error
    };
    let context = json!({"repository":"selected"});
    assert_eq!(
        info(json!({"context":context,"request":null})).await,
        json!({"kind":"unavailable_needs"})
    );
    assert_eq!(
        info(json!({"context":context,"request":{}})).await,
        json!({"kind":"invalid_input","field":"request","reason":"invalid_value"})
    );
    assert_eq!(
        info(json!({"context":context})).await,
        json!({"kind":"invalid_input","field":"request","reason":"required"})
    );
}
