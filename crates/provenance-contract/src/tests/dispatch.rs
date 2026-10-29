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
            "invalid_value",
        ),
        (
            json!({"request":{"statement":"text"},"context":{"repository":"/tmp"}}),
            "context",
            "invalid_value",
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
