use provenance_http_client::{types::GetFailureOutput, PROTOCOL_VERSION};
use serde_json::json;

#[test]
fn generated_read_failure_rejects_unknown_kinds() {
    let value =
        json!({"protocol_version":PROTOCOL_VERSION,"operation":"get","error":{"kind":"invented"}});
    assert!(serde_json::from_value::<GetFailureOutput>(value).is_err());
}

#[test]
fn generated_read_failure_retains_the_stale_facts() {
    let value = json!({"protocol_version":PROTOCOL_VERSION,"operation":"get","error":{
        "kind":"stale","serial":17,"digest":"stored","instance_id":"fixture",
        "moved":[{"unit":"rules","stored":"before","live":"after"}]
    }});
    let decoded: GetFailureOutput = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
}
